"""
a1.otel — OpenTelemetry tracing integration for A1 authorization events.

Every authorization attempt — whether guarded by ``@a1_guard`` or called
directly via ``A1Client.authorize`` — can emit a structured OTEL span so
enterprises running Datadog APM, Jaeger, Honeycomb, or any OTLP-compatible
backend get full distributed trace context through their agent delegation chains.

Usage::

    from a1.otel import A1Tracer

    tracer = A1Tracer(service_name="acme-trading-bot")

    # Option 1 — wrap an existing PassportClient
    from a1 import PassportClient
    guarded = tracer.instrument_passport_client(PassportClient("http://localhost:8080"))

    # Option 2 — decorator
    @tracer.trace_capability("trade.equity")
    async def execute_trade(symbol: str, qty: int) -> dict:
        ...

All spans use the ``dyolo.a1.`` attribute namespace. The trace/span IDs
are attached to the A1 AuditEvent (via the ``trace_id`` and ``span_id``
fields) if the gateway's OTEL exporter is configured.

OTEL dependencies are optional extras::

    pip install "a1[siem-otel]"

If the ``opentelemetry-sdk`` package is absent, the module degrades gracefully
to a no-op implementation so integrations compile and run without OTEL installed.
"""

from __future__ import annotations

import contextlib
import functools
import time
from dataclasses import dataclass
from typing import Any, Callable, Optional

__all__ = [
    "A1Tracer",
    "A1Span",
    "noop_tracer",
]

# ── OTEL availability check ───────────────────────────────────────────────────

try:
    from opentelemetry import trace
    from opentelemetry.sdk.trace import TracerProvider
    from opentelemetry.sdk.trace.export import BatchSpanProcessor
    from opentelemetry.sdk.resources import Resource
    _OTEL_AVAILABLE = True
except ImportError:
    _OTEL_AVAILABLE = False


# ── Span data class ───────────────────────────────────────────────────────────

@dataclass
class A1Span:
    """Immutable view of an A1 authorization span for audit export."""

    trace_id:   Optional[str]
    span_id:    Optional[str]
    authorized: bool
    capability: str
    namespace:  Optional[str]
    fingerprint: Optional[str]
    duration_ms: float
    error:      Optional[str] = None


# ── No-op fallback ────────────────────────────────────────────────────────────

class _NoopSpan:
    """Zero-overhead stand-in when OTEL is not installed."""

    trace_id: Optional[str] = None
    span_id:  Optional[str] = None

    def set_attribute(self, _key: str, _val: Any) -> None: ...
    def set_status(self, *_args: Any, **_kwargs: Any) -> None: ...
    def record_exception(self, _exc: Exception) -> None: ...
    def __enter__(self) -> "_NoopSpan": return self
    def __exit__(self, *_args: Any) -> None: ...


class _NoopTracer:
    def start_as_current_span(self, _name: str, **_kwargs: Any):  # noqa: ANN
        return _NoopSpan()


# ── A1Tracer ──────────────────────────────────────────────────────────────────

class A1Tracer:
    """
    OpenTelemetry tracer for A1 authorization events.

    Emits spans with ``dyolo.a1.*`` attributes that record the full
    authorization context.  When OTEL SDK is not installed, all methods
    degrade gracefully to no-ops.

    Parameters
    ----------
    service_name:
        Resource attribute for the service producing spans.
    otlp_endpoint:
        OTLP HTTP endpoint, e.g. ``http://localhost:4318``.  When omitted
        the global tracer provider is used if one is already configured.
    tracer_name:
        Name shown in the OTEL backend (defaults to ``"a1"``)
    """

    def __init__(
        self,
        service_name: str = "a1-agent",
        otlp_endpoint: Optional[str] = None,
        tracer_name: str = "a1",
    ) -> None:
        self._tracer_name = tracer_name
        self._tracer: Any = None

        if not _OTEL_AVAILABLE:
            self._tracer = _NoopTracer()
            return

        if otlp_endpoint:
            try:
                from opentelemetry.exporter.otlp.proto.http.trace_exporter import OTLPSpanExporter

                resource = Resource(attributes={
                    "service.name":    service_name,
                    "a1.provenance":   "64796f6c6f",
                    "a1.version":      "2.8.0",
                })
                provider = TracerProvider(resource=resource)
                provider.add_span_processor(
                    BatchSpanProcessor(OTLPSpanExporter(endpoint=f"{otlp_endpoint}/v1/traces"))
                )
                self._tracer = provider.get_tracer(tracer_name)
            except Exception:
                self._tracer = _NoopTracer()
        else:
            try:
                self._tracer = trace.get_tracer(tracer_name)
            except Exception:
                self._tracer = _NoopTracer()

    # ── Core span context manager ─────────────────────────────────────────────

    @contextlib.contextmanager
    def authorization_span(self, capability: str, namespace: Optional[str] = None):
        """
        Context manager that wraps an A1 authorization attempt in an OTEL span.

        Yields a mutable dict for callers to record the authorization result::

            with tracer.authorization_span("trade.equity", "acme-bot") as ctx:
                result = client.authorize(...)
                ctx["authorized"]  = result.authorized
                ctx["fingerprint"] = result.chain_fingerprint

        The span attributes are set automatically from the ``ctx`` dict when
        the context exits.
        """
        start = time.perf_counter()
        ctx: dict[str, Any] = {
            "authorized":  False,
            "fingerprint": None,
            "error":       None,
            "namespace":   namespace,
        }

        span_name = f"dyolo.a1.authorize/{capability}"
        with self._tracer.start_as_current_span(span_name) as span:
            try:
                span.set_attribute("a1.capability",  capability)
                span.set_attribute("a1.provenance",  "64796f6c6f")
                if namespace:
                    span.set_attribute("a1.namespace", namespace)

                yield ctx

                span.set_attribute("a1.authorized",  ctx.get("authorized", False))
                if ctx.get("fingerprint"):
                    span.set_attribute("a1.chain_fingerprint", ctx["fingerprint"])
                if ctx.get("error"):
                    span.set_attribute("a1.error", ctx["error"])
                    if _OTEL_AVAILABLE:
                        span.set_status(trace.StatusCode.ERROR, ctx["error"])
                elif ctx.get("authorized"):
                    if _OTEL_AVAILABLE:
                        span.set_status(trace.StatusCode.OK)

            except Exception as exc:
                ctx["error"] = str(exc)
                ctx["authorized"] = False
                if hasattr(span, "record_exception"):
                    span.record_exception(exc)
                if _OTEL_AVAILABLE:
                    span.set_status(trace.StatusCode.ERROR, str(exc))
                raise
            finally:
                duration_ms = (time.perf_counter() - start) * 1000
                span.set_attribute("a1.duration_ms", round(duration_ms, 2))

    # ── Decorator ─────────────────────────────────────────────────────────────

    def trace_capability(self, capability: str, namespace: Optional[str] = None) -> Callable:
        """
        Decorator that wraps a function with an A1 authorization span.

        Works with both sync and async functions.  The function's return value
        is forwarded unchanged; no authorization logic is applied — this is a
        pure observability wrapper.

        For combined observability + enforcement, use ``@a1_guard`` from
        ``a1.passport`` together with ``trace_capability``::

            @tracer.trace_capability("trade.equity", "acme-bot")
            @a1_guard(client=passport_client, capability="trade.equity")
            async def execute_trade(symbol: str, qty: int) -> dict:
                ...
        """

        def decorator(fn: Callable) -> Callable:
            import asyncio

            if asyncio.iscoroutinefunction(fn):
                @functools.wraps(fn)
                async def async_wrapper(*args: Any, **kwargs: Any) -> Any:
                    with self.authorization_span(capability, namespace) as ctx:
                        result = await fn(*args, **kwargs)
                        ctx["authorized"] = True
                        return result
                return async_wrapper
            else:
                @functools.wraps(fn)
                def sync_wrapper(*args: Any, **kwargs: Any) -> Any:
                    with self.authorization_span(capability, namespace) as ctx:
                        result = fn(*args, **kwargs)
                        ctx["authorized"] = True
                        return result
                return sync_wrapper

        return decorator

    # ── Client instrumentation ────────────────────────────────────────────────

    def instrument_passport_client(self, client: Any) -> Any:
        """
        Wrap a ``PassportClient`` so every ``guard`` call emits an OTEL span.

        Returns a transparent proxy — all attributes and methods are forwarded.
        The proxy is safe to use as a drop-in replacement.
        """
        tracer = self

        class _InstrumentedPassportClient:
            def __getattr__(self, name: str) -> Any:
                return getattr(client, name)

            async def guard(
                self,
                capability: str,
                chain: Any,
                executor_pk_hex: str,
                params: Optional[dict] = None,
            ) -> Any:
                ns = getattr(client, "_namespace", None)
                with tracer.authorization_span(capability, ns) as ctx:
                    result = await client.guard(capability, chain, executor_pk_hex, params)
                    ctx["authorized"]  = True
                    ctx["fingerprint"] = getattr(result, "fingerprint_hex", None)
                    ctx["namespace"]   = getattr(result, "passport_namespace", ns)
                    return result

        return _InstrumentedPassportClient()

    # ── Current trace/span IDs ────────────────────────────────────────────────

    @staticmethod
    def current_trace_id() -> Optional[str]:
        """Return the hex trace ID of the current OTEL span, or None."""
        if not _OTEL_AVAILABLE:
            return None
        try:
            span_ctx = trace.get_current_span().get_span_context()
            tid = span_ctx.trace_id
            return format(tid, "032x") if tid else None
        except Exception:
            return None

    @staticmethod
    def current_span_id() -> Optional[str]:
        """Return the hex span ID of the current OTEL span, or None."""
        if not _OTEL_AVAILABLE:
            return None
        try:
            span_ctx = trace.get_current_span().get_span_context()
            sid = span_ctx.span_id
            return format(sid, "016x") if sid else None
        except Exception:
            return None


# ── Module-level no-op singleton ─────────────────────────────────────────────

noop_tracer: A1Tracer = A1Tracer.__new__(A1Tracer)
noop_tracer._tracer = _NoopTracer()  # type: ignore[attr-defined]
noop_tracer._tracer_name = "a1-noop"  # type: ignore[attr-defined]
