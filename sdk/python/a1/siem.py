"""
a1.siem — Structured audit exporters for enterprise SIEM integration.

Every authorization event produced by a1 can be forwarded to your
existing SIEM infrastructure. This module provides plug-and-play exporters
for the four most common enterprise observability stacks:

- ``NdjsonFileExporter``      — line-delimited JSON file (universal, works with any SIEM)
- ``DatadogLogExporter``      — Datadog Logs via HTTP intake
- ``SplunkHecExporter``       — Splunk HTTP Event Collector
- ``OpenTelemetryExporter``   — OTLP/HTTP log exporter (compatible with any OTel backend)
- ``CompositeExporter``       — fan-out to multiple exporters simultaneously

All exporters implement the ``AuditExporter`` protocol. Pass one to any
gateway route handler or use them directly in your Python application.

Usage
-----
    from a1.siem import DatadogLogExporter, CompositeExporter, SplunkHecExporter

    dd = DatadogLogExporter(api_key=os.environ["DD_API_KEY"], service="trading-agents")
    splunk = SplunkHecExporter(url="https://splunk.corp.example.com:8088", token=os.environ["SPLUNK_HEC_TOKEN"])
    exporter = CompositeExporter([dd, splunk])

    # In your authorization handler:
    exporter.export(auth_event_dict)
"""

from __future__ import annotations

import json
import os
import threading
import time
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional

__all__ = [
    "AuditExporter",
    "AuditRecord",
    "NdjsonFileExporter",
    "DatadogLogExporter",
    "SplunkHecExporter",
    "OpenTelemetryExporter",
    "CompositeExporter",
    "BufferedExporter",
]


@dataclass
class AuditRecord:
    """
    Normalized audit record emitted after every authorization attempt.

    Fields are a superset of the Rust ``AuditEvent`` wire format so that
    Python-side enrichment (user agent, request ID, geo, etc.) can be
    added without modifying the Rust layer.
    """
    event_id: str
    timestamp_unix: int
    outcome: str
    principal_pk_hex: str
    executor_pk_hex: str
    chain_depth: int
    intent_hex: str
    passport_namespace: Optional[str] = None
    chain_fingerprint: Optional[str] = None
    error_message: Optional[str] = None
    policy_name: Optional[str] = None
    capability_mask_hex: Optional[str] = None
    request_id: Optional[str] = None
    user_agent: Optional[str] = None
    source_ip: Optional[str] = None
    extra: Dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        d = {
            "event_id": self.event_id,
            "timestamp_unix": self.timestamp_unix,
            "timestamp_iso": _unix_to_iso(self.timestamp_unix),
            "outcome": self.outcome,
            "principal_pk_hex": self.principal_pk_hex,
            "executor_pk_hex": self.executor_pk_hex,
            "chain_depth": self.chain_depth,
            "intent_hex": self.intent_hex,
        }
        for key in (
            "passport_namespace", "chain_fingerprint", "error_message",
            "policy_name", "capability_mask_hex", "request_id",
            "user_agent", "source_ip",
        ):
            val = getattr(self, key)
            if val is not None:
                d[key] = val
        d.update(self.extra)
        return d

    def to_ndjson(self) -> str:
        return json.dumps(self.to_dict(), separators=(",", ":"))

    @classmethod
    def from_dict(cls, d: Dict[str, Any]) -> "AuditRecord":
        known = {
            "event_id", "timestamp_unix", "outcome", "principal_pk_hex",
            "executor_pk_hex", "chain_depth", "intent_hex", "passport_namespace",
            "chain_fingerprint", "error_message", "policy_name",
            "capability_mask_hex", "request_id", "user_agent", "source_ip",
        }
        extra = {k: v for k, v in d.items() if k not in known and k != "timestamp_iso"}
        return cls(
            event_id=d.get("event_id", ""),
            timestamp_unix=d.get("timestamp_unix", 0),
            outcome=d.get("outcome", "UNKNOWN"),
            principal_pk_hex=d.get("principal_pk_hex", ""),
            executor_pk_hex=d.get("executor_pk_hex", ""),
            chain_depth=d.get("chain_depth", 0),
            intent_hex=d.get("intent_hex", ""),
            passport_namespace=d.get("passport_namespace"),
            chain_fingerprint=d.get("chain_fingerprint"),
            error_message=d.get("error_message"),
            policy_name=d.get("policy_name"),
            capability_mask_hex=d.get("capability_mask_hex"),
            request_id=d.get("request_id"),
            user_agent=d.get("user_agent"),
            source_ip=d.get("source_ip"),
            extra=extra,
        )


class AuditExporter(ABC):
    """
    Protocol for a1 audit exporters.

    Implement ``export`` to forward authorization events to your SIEM.
    All built-in exporters are safe to share across threads.
    """

    @abstractmethod
    def export(self, record: AuditRecord) -> None:
        """Forward a single audit record synchronously."""

    def export_dict(self, d: Dict[str, Any]) -> None:
        """Convenience wrapper — parse a raw event dict and export it."""
        self.export(AuditRecord.from_dict(d))

    def close(self) -> None:
        """Flush pending events and release any held resources."""


class NdjsonFileExporter(AuditExporter):
    """
    Append-only NDJSON file exporter.

    Every authorization event is written as one JSON line to the target
    file. Thread-safe via an internal write lock.

    Compatible with any SIEM that supports file-based ingestion (Splunk
    Universal Forwarder, Filebeat, Fluentd, Vector, etc.).

    Parameters
    ----------
    path:
        Destination file path. Created if it does not exist.
    flush_every:
        Flush the file handle after this many writes (default: 1).
        Increase for high-throughput deployments.
    """

    def __init__(self, path: str, *, flush_every: int = 1) -> None:
        self._path = path
        self._flush_every = flush_every
        self._lock = threading.Lock()
        self._count = 0
        self._fh = open(path, "a", encoding="utf-8")

    def export(self, record: AuditRecord) -> None:
        with self._lock:
            self._fh.write(record.to_ndjson() + "\n")
            self._count += 1
            if self._count % self._flush_every == 0:
                self._fh.flush()

    def close(self) -> None:
        with self._lock:
            self._fh.flush()
            self._fh.close()


class DatadogLogExporter(AuditExporter):
    """
    Datadog Logs HTTP intake exporter.

    Sends each authorization event to the Datadog Logs API as a structured
    log entry. Set ``service``, ``source``, and ``tags`` to match your
    existing Datadog taxonomy.

    Requires: ``pip install httpx``

    Parameters
    ----------
    api_key:
        Datadog API key. Falls back to ``DD_API_KEY`` environment variable.
    site:
        Datadog site (default: ``"datadoghq.com"``). Use ``"datadoghq.eu"``
        for EU customers.
    service:
        Service name shown in Datadog (e.g. ``"trading-agents"``).
    source:
        Log source tag (default: ``"a1"``).
    tags:
        Additional tags to attach, e.g. ``["env:production", "team:platform"]``.
    timeout:
        HTTP timeout in seconds (default: 5).
    """

    _ENDPOINT = "https://http-intake.logs.{site}/api/v2/logs"

    def __init__(
        self,
        *,
        api_key: Optional[str] = None,
        site: str = "datadoghq.com",
        service: str = "a1",
        source: str = "a1",
        tags: Optional[List[str]] = None,
        timeout: float = 5.0,
    ) -> None:
        self._api_key = api_key or os.environ.get("DD_API_KEY", "")
        if not self._api_key:
            raise ValueError("Datadog API key required: pass api_key or set DD_API_KEY")
        self._url = self._ENDPOINT.format(site=site)
        self._service = service
        self._source = source
        self._tags = ",".join(tags or [])
        self._timeout = timeout

    def export(self, record: AuditRecord) -> None:
        try:
            import httpx
        except ImportError as exc:
            raise ImportError("httpx is required: pip install httpx") from exc

        payload = {
            "ddsource": self._source,
            "ddtags": self._tags,
            "hostname": os.uname().nodename if hasattr(os, "uname") else "unknown",
            "service": self._service,
            "message": record.to_ndjson(),
            **record.to_dict(),
        }

        try:
            httpx.post(
                self._url,
                json=[payload],
                headers={
                    "DD-API-KEY": self._api_key,
                    "Content-Type": "application/json",
                },
                timeout=self._timeout,
            ).raise_for_status()
        except Exception as exc:
            import warnings
            warnings.warn(f"[a1/DatadogLogExporter] Failed to export event: {exc}", stacklevel=2)


class SplunkHecExporter(AuditExporter):
    """
    Splunk HTTP Event Collector (HEC) exporter.

    Sends each authorization event to Splunk HEC as a structured event.

    Requires: ``pip install httpx``

    Parameters
    ----------
    url:
        Splunk HEC endpoint, e.g. ``"https://splunk.corp.example.com:8088"``.
    token:
        HEC authentication token. Falls back to ``SPLUNK_HEC_TOKEN``.
    index:
        Splunk index to write to (optional).
    source:
        Splunk source field (default: ``"a1"``).
    sourcetype:
        Splunk sourcetype field (default: ``"_json"``).
    verify_ssl:
        Verify TLS certificates (default: ``True``). Disable only in dev.
    timeout:
        HTTP timeout in seconds (default: 5).
    """

    def __init__(
        self,
        *,
        url: str,
        token: Optional[str] = None,
        index: Optional[str] = None,
        source: str = "a1",
        sourcetype: str = "_json",
        verify_ssl: bool = True,
        timeout: float = 5.0,
    ) -> None:
        self._url = url.rstrip("/") + "/services/collector/event"
        self._token = token or os.environ.get("SPLUNK_HEC_TOKEN", "")
        if not self._token:
            raise ValueError("Splunk HEC token required: pass token or set SPLUNK_HEC_TOKEN")
        self._index = index
        self._source = source
        self._sourcetype = sourcetype
        self._verify_ssl = verify_ssl
        self._timeout = timeout

    def export(self, record: AuditRecord) -> None:
        try:
            import httpx
        except ImportError as exc:
            raise ImportError("httpx is required: pip install httpx") from exc

        payload: Dict[str, Any] = {
            "time": record.timestamp_unix,
            "source": self._source,
            "sourcetype": self._sourcetype,
            "event": record.to_dict(),
        }
        if self._index:
            payload["index"] = self._index

        try:
            httpx.post(
                self._url,
                json=payload,
                headers={"Authorization": f"Splunk {self._token}"},
                verify=self._verify_ssl,
                timeout=self._timeout,
            ).raise_for_status()
        except Exception as exc:
            import warnings
            warnings.warn(f"[a1/SplunkHecExporter] Failed to export event: {exc}", stacklevel=2)


class OpenTelemetryExporter(AuditExporter):
    """
    OpenTelemetry OTLP/HTTP log exporter.

    Emits each authorization event as an OTel ``LogRecord`` with all
    ``AuditRecord`` fields mapped to log body attributes. Compatible with
    any OTel collector: Grafana Alloy, Elastic, New Relic, Honeycomb, etc.

    Requires: ``pip install opentelemetry-sdk opentelemetry-exporter-otlp-proto-http``

    Parameters
    ----------
    endpoint:
        OTLP HTTP logs endpoint, e.g. ``"http://otel-collector:4318"``.
    service_name:
        OTel service name (default: ``"a1"``).
    headers:
        Extra HTTP headers, e.g. for authentication tokens.
    timeout:
        Export timeout in seconds (default: 5).
    """

    def __init__(
        self,
        *,
        endpoint: str = "http://localhost:4318",
        service_name: str = "a1",
        headers: Optional[Dict[str, str]] = None,
        timeout: float = 5.0,
    ) -> None:
        self._endpoint = endpoint
        self._service_name = service_name
        self._headers = headers or {}
        self._timeout = timeout
        self._provider: Any = None

    def _ensure_provider(self) -> Any:
        if self._provider is not None:
            return self._provider
        try:
            from opentelemetry.sdk._logs import LoggerProvider
            from opentelemetry.sdk._logs.export import BatchLogRecordProcessor
            from opentelemetry.sdk.resources import Resource
            from opentelemetry.exporter.otlp.proto.http._log_exporter import OTLPLogExporter
        except ImportError as exc:
            raise ImportError(
                "OpenTelemetry SDK required: "
                "pip install opentelemetry-sdk opentelemetry-exporter-otlp-proto-http"
            ) from exc

        resource = Resource.create({"service.name": self._service_name})
        exporter = OTLPLogExporter(
            endpoint=self._endpoint.rstrip("/") + "/v1/logs",
            headers=self._headers,
            timeout=int(self._timeout),
        )
        provider = LoggerProvider(resource=resource)
        provider.add_log_record_processor(BatchLogRecordProcessor(exporter))
        self._provider = provider
        return provider

    def export(self, record: AuditRecord) -> None:
        try:
            from opentelemetry._logs import SeverityNumber
            from opentelemetry.sdk._logs import LogRecord
        except ImportError as exc:
            raise ImportError(
                "OpenTelemetry SDK required: "
                "pip install opentelemetry-sdk opentelemetry-exporter-otlp-proto-http"
            ) from exc

        provider = self._ensure_provider()
        logger = provider.get_logger("a1")

        severity = (
            SeverityNumber.INFO
            if record.outcome == "AUTHORIZED"
            else SeverityNumber.WARN
        )

        body = record.to_dict()
        log_record = LogRecord(
            timestamp=record.timestamp_unix * 1_000_000_000,
            severity_number=severity,
            severity_text=record.outcome,
            body=record.to_ndjson(),
            attributes=body,
        )
        logger.emit(log_record)

    def close(self) -> None:
        if self._provider is not None:
            try:
                self._provider.shutdown()
            except Exception:
                pass


class CompositeExporter(AuditExporter):
    """
    Fan-out exporter that forwards each event to multiple exporters.

    Failures in individual exporters are logged as warnings — they do not
    propagate to the caller. This ensures that a SIEM outage never blocks
    agent authorization.

    Parameters
    ----------
    exporters:
        List of ``AuditExporter`` instances to forward events to.
    """

    def __init__(self, exporters: List[AuditExporter]) -> None:
        self._exporters = list(exporters)

    def export(self, record: AuditRecord) -> None:
        for exporter in self._exporters:
            try:
                exporter.export(record)
            except Exception as exc:
                import warnings
                warnings.warn(
                    f"[a1/CompositeExporter] {type(exporter).__name__} failed: {exc}",
                    stacklevel=2,
                )

    def close(self) -> None:
        for exporter in self._exporters:
            try:
                exporter.close()
            except Exception:
                pass


class BufferedExporter(AuditExporter):
    """
    In-memory buffered exporter with configurable batch flush.

    Accumulates events in a thread-safe queue and flushes them to a
    downstream exporter in batches. Reduces network overhead for
    high-throughput deployments.

    Parameters
    ----------
    downstream:
        The target exporter to flush batches to.
    batch_size:
        Number of records per flush batch (default: 50).
    flush_interval_secs:
        Maximum time between flushes in seconds (default: 5.0).
    """

    def __init__(
        self,
        downstream: AuditExporter,
        *,
        batch_size: int = 50,
        flush_interval_secs: float = 5.0,
    ) -> None:
        import queue
        self._downstream = downstream
        self._batch_size = batch_size
        self._flush_interval = flush_interval_secs
        self._queue: "queue.Queue[AuditRecord]" = queue.Queue()
        self._lock = threading.Lock()
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._flush_loop, daemon=True)
        self._thread.start()

    def export(self, record: AuditRecord) -> None:
        self._queue.put(record)

    def _flush_loop(self) -> None:
        import queue
        while not self._stop.is_set():
            batch: List[AuditRecord] = []
            deadline = time.monotonic() + self._flush_interval
            while len(batch) < self._batch_size and time.monotonic() < deadline:
                try:
                    batch.append(self._queue.get(timeout=0.1))
                except queue.Empty:
                    break
            for record in batch:
                self._downstream.export(record)

    def close(self) -> None:
        self._stop.set()
        self._thread.join(timeout=10.0)
        import queue as q
        remaining: List[AuditRecord] = []
        while True:
            try:
                remaining.append(self._queue.get_nowait())
            except q.Empty:
                break
        for record in remaining:
            self._downstream.export(record)
        self._downstream.close()


def _unix_to_iso(unix_ts: int) -> str:
    import datetime
    return datetime.datetime.utcfromtimestamp(unix_ts).strftime("%Y-%m-%dT%H:%M:%SZ")