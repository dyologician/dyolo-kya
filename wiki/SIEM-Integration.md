# SIEM Integration

A1 emits a structured audit event for every authorization attempt — authorized, denied, or policy-violated. Events can be forwarded to any SIEM in real time.

---

## What gets logged

Every event includes:

| Field | Description |
|---|---|
| `timestamp` | ISO 8601 UTC |
| `outcome` | `AUTHORIZED` / `DENIED` / `POLICY_VIOLATION` / `STORAGE_ERROR` |
| `namespace` | Passport namespace (tenant isolation scope) |
| `chain_fingerprint` | Blake3 over all certs — uniquely identifies this delegation chain |
| `capability_mask` | 256-bit hex mask authorized at this call |
| `intent_hash` | Blake3 of the authorized intent |
| `chain_depth` | Number of delegation hops |
| `agent_pk_hex` | Executing agent's public key |
| `error_code` | Error code on DENIED events (e.g. `PASSPORT_NARROWING_VIOLATION`) |
| `error_message` | Human-readable reason |

---

## Python SDK exporters

```bash
pip install "a1identity[siem-datadog]"    # Datadog
pip install "a1identity[siem-splunk]"     # Splunk
pip install "a1identity[siem-otel]"       # OpenTelemetry
# NdjsonFileExporter included in base package
```

### Datadog

```python
from a1.siem import DatadogLogExporter

exporter = DatadogLogExporter(
    api_key=os.environ["DD_API_KEY"],
    service="trading-agents",
    host="https://http-intake.logs.datadoghq.com",  # or EU endpoint
)

exporter.export(auth_event)
```

Events appear in Datadog Logs under `source:a1`. Set up a monitor on `outcome:DENIED` for instant alerting.

---

### Splunk HEC

```python
from a1.siem import SplunkHecExporter

exporter = SplunkHecExporter(
    url="https://splunk.corp.example.com:8088",
    token=os.environ["SPLUNK_HEC_TOKEN"],
    index="a1-audit",
    sourcetype="a1:authorization",
)

exporter.export(auth_event)
```

---

### OpenTelemetry

```python
from a1.siem import OpenTelemetryExporter

exporter = OpenTelemetryExporter(
    endpoint="https://otel.corp.example.com:4318/v1/logs",
    service_name="trading-agents",
)

exporter.export(auth_event)
```

Compatible with any OTLP/HTTP backend: Grafana Loki, Honeycomb, Dynatrace, New Relic, AWS CloudWatch, etc.

---

### NDJSON file (universal)

```python
from a1.siem import NdjsonFileExporter

exporter = NdjsonFileExporter(path="/var/log/a1-audit.jsonl")
exporter.export(auth_event)
```

One JSON object per line. Feed to any log shipper: Filebeat, Fluentd, Vector, Logstash, Fluent Bit.

---

### Fan-out (multiple destinations)

```python
from a1.siem import CompositeExporter, DatadogLogExporter, SplunkHecExporter, NdjsonFileExporter

exporter = CompositeExporter([
    DatadogLogExporter(api_key=os.environ["DD_API_KEY"]),
    SplunkHecExporter(url="https://splunk.corp.example.com:8088", token=os.environ["SPLUNK_TOKEN"]),
    NdjsonFileExporter(path="/var/log/a1-audit.jsonl"),  # local backup
])

exporter.export(auth_event)
```

---

### Buffered exporter (high-throughput)

```python
from a1.siem import BufferedExporter, DatadogLogExporter

exporter = BufferedExporter(
    inner=DatadogLogExporter(api_key=os.environ["DD_API_KEY"]),
    max_batch=100,
    flush_interval_seconds=5,
)

# Export is non-blocking — events are buffered and flushed in background
exporter.export(auth_event)
```

---

## Rust gateway (AuditSink)

In the Rust gateway, audit events are emitted through the `AuditSink` trait. The in-process SIEM exporter (`SiemHttpAuditSink`) forwards events to any HTTP endpoint using the NDJSON format.

Set `A1_AUDIT_ENDPOINT` and `A1_AUDIT_TOKEN` environment variables to enable it:

```bash
A1_AUDIT_ENDPOINT=https://http-intake.logs.datadoghq.com/api/v2/logs
A1_AUDIT_TOKEN=<your-datadog-api-key>
```

---

## Alerting recommendations

| Alert | Condition | Severity |
|---|---|---|
| Unauthorized capability | `outcome=DENIED AND error_code=PASSPORT_NARROWING_VIOLATION` | High |
| Replay attack | `outcome=DENIED AND error_code=NONCE_REPLAY` | Critical |
| Revoked cert used | `outcome=DENIED AND error_code=CERT_REVOKED` | Critical |
| Expired cert | `outcome=DENIED AND error_code=CERT_EXPIRED` | Medium |
| Storage error | `outcome=STORAGE_ERROR` | High |
| High denial rate | `outcome=DENIED count > threshold per minute` | High |

---

## OpenTelemetry tracing (Python)

Beyond log export, the `A1Tracer` wraps passport client calls with OTEL spans:

```python
from a1.otel import A1Tracer

tracer = A1Tracer(service_name="trading-service")

with tracer.trace_capability("trade.equity") as span:
    result = await client.authorize(...)
    span.set_attribute("a1.chain_depth", result.chain_depth)
    span.set_attribute("a1.namespace", result.namespace)
```

Spans appear in your OTEL backend (Jaeger, Zipkin, Honeycomb, etc.) with full distributed trace context.

---

*Source: `src/audit.rs`, `sdk/python/a1/siem.py`, `sdk/python/a1/otel.py` · [Back to wiki home](Home)*
