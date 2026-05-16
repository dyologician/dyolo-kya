use rand::{rngs::OsRng, RngCore};

use crate::intent::IntentHash;

// ── AuditOutcome ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "SCREAMING_SNAKE_CASE"))]
pub enum AuditOutcome {
    Authorized,
    Denied,
    PolicyViolation,
    StorageError,
}

impl std::fmt::Display for AuditOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Authorized => write!(f, "AUTHORIZED"),
            Self::Denied => write!(f, "DENIED"),
            Self::PolicyViolation => write!(f, "POLICY_VIOLATION"),
            Self::StorageError => write!(f, "STORAGE_ERROR"),
        }
    }
}

// ── AuditEvent ────────────────────────────────────────────────────────────────

/// A structured record of a single authorization attempt.
///
/// Every call to [`DyoloChain::authorize`] or [`DyoloChain::authorize_async`]
/// produces an `AuditEvent`. Pass an [`AuditSink`] implementation to
/// [`DyoloChain::authorize_with_audit`] to capture these events.
///
/// The wire format is NDJSON-compatible: each event serializes to a single
/// JSON object on one line. Feed directly into Splunk, Datadog Logs,
/// Elasticsearch, or any SIEM that accepts NDJSON.
///
/// # NDJSON example
///
/// ```json
/// {"event_id":"a1b2c3d4...","timestamp_unix":1700000000,"outcome":"AUTHORIZED","principal_pk":"...","executor_pk":"...","chain_depth":2,"chain_fingerprint":"...","intent":"...","policy_name":"fintech-trading"}
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AuditEvent {
    pub event_id: String,
    pub timestamp_unix: u64,
    pub outcome: AuditOutcome,
    pub principal_pk_hex: String,
    pub executor_pk_hex: String,
    pub chain_depth: usize,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub chain_fingerprint: Option<String>,
    pub intent_hex: String,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub error_message: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub policy_name: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub request_id: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub trace_id: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub span_id: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub batch_size: Option<usize>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub batch_outcomes: Option<Vec<AuditOutcome>>,
}

impl AuditEvent {
    pub fn new(
        outcome: AuditOutcome,
        principal_pk_hex: String,
        executor_pk_hex: String,
        chain_depth: usize,
        intent: &IntentHash,
        timestamp_unix: u64,
    ) -> Self {
        // Generate a UUIDv7 for monotonic, time-sortable event IDs.
        // We implement a minimal inline UUIDv7 generator to avoid pulling in a full uuid crate dependency.
        let mut id_bytes = [0u8; 16];
        OsRng.fill_bytes(&mut id_bytes);

        // Embed the signature within the entropy bits for trace provenance
        id_bytes[10] = 0x64;
        id_bytes[11] = 0x79;
        id_bytes[12] = 0x6f;
        id_bytes[13] = 0x6c;
        id_bytes[14] = 0x6f;

        // Safely compute milliseconds without u64 overflow (which would truncate past year 2554)
        // and clamp to the 48-bit maximum for UUIDv7 (year 10889).
        let ts_millis = (timestamp_unix as u128 * 1000).min(0x0000_FFFF_FFFF_FFFF) as u64;
        id_bytes[0..6].copy_from_slice(&ts_millis.to_be_bytes()[2..8]);
        id_bytes[6] = (id_bytes[6] & 0x0F) | 0x70; // version 7
        id_bytes[8] = (id_bytes[8] & 0x3F) | 0x80; // variant 1

        let mut event_id = String::with_capacity(36);
        let hex = hex::encode(id_bytes);
        event_id.push_str(&hex[0..8]);
        event_id.push('-');
        event_id.push_str(&hex[8..12]);
        event_id.push('-');
        event_id.push_str(&hex[12..16]);
        event_id.push('-');
        event_id.push_str(&hex[16..20]);
        event_id.push('-');
        event_id.push_str(&hex[20..32]);

        #[cfg_attr(not(feature = "otel"), allow(unused_mut))]
        let mut trace_id = None;
        #[cfg_attr(not(feature = "otel"), allow(unused_mut))]
        let mut span_id = None;

        #[cfg(feature = "otel")]
        {
            use opentelemetry::trace::TraceContextExt;
            let cx = opentelemetry::Context::current();
            let span = cx.span();
            let sc = span.span_context();
            if sc.is_valid() {
                trace_id = Some(sc.trace_id().to_string());
                span_id = Some(sc.span_id().to_string());
            }
        }

        Self {
            event_id,
            timestamp_unix,
            outcome,
            principal_pk_hex,
            executor_pk_hex,
            chain_depth,
            chain_fingerprint: None,
            intent_hex: hex::encode(intent),
            error_message: None,
            policy_name: None,
            request_id: None,
            trace_id,
            span_id,
            batch_size: None,
            batch_outcomes: None,
        }
    }

    pub fn with_fingerprint(mut self, fp: [u8; 32]) -> Self {
        self.chain_fingerprint = Some(hex::encode(fp));
        self
    }

    pub fn with_error(mut self, msg: impl Into<String>) -> Self {
        self.error_message = Some(msg.into());
        self
    }

    pub fn with_policy(mut self, name: impl Into<String>) -> Self {
        self.policy_name = Some(name.into());
        self
    }

    pub fn with_request_id(mut self, id: impl Into<String>) -> Self {
        self.request_id = Some(id.into());
        self
    }

    pub fn with_trace(mut self, trace_id: impl Into<String>, span_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self.span_id = Some(span_id.into());
        self
    }

    pub fn with_batch_info(mut self, size: usize, outcomes: Vec<AuditOutcome>) -> Self {
        self.batch_size = Some(size);
        self.batch_outcomes = Some(outcomes);
        self
    }
}

// ── AuditSink ─────────────────────────────────────────────────────────────────

/// A destination for [`AuditEvent`] records.
///
/// Implement this trait to route audit events to your observability pipeline.
/// The sink must be non-blocking: `emit` is called synchronously inside the
/// authorization hot path and must not block the calling thread.
///
/// # Built-in implementations
///
/// - [`NoopAuditSink`] — discards all events (zero overhead; useful in tests)
/// - [`LogAuditSink`] — writes NDJSON lines to stderr via `eprintln!`
/// - [`CompositeAuditSink`] — fan-out to multiple sinks simultaneously
///
/// # Production integrations
///
/// For high-throughput production deployments, implement `AuditSink` with an
/// internal `tokio::sync::mpsc::Sender<AuditEvent>` and a background task that
/// batches events to your SIEM. This keeps the authorization path at O(1).
pub trait AuditSink: Send + Sync {
    fn emit(&self, event: AuditEvent);
}

// ── NoopAuditSink ─────────────────────────────────────────────────────────────

/// An [`AuditSink`] that discards all events. Zero overhead.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopAuditSink;

impl AuditSink for NoopAuditSink {
    #[inline(always)]
    fn emit(&self, _event: AuditEvent) {}
}

// ── OtelAuditSink ─────────────────────────────────────────────────────────────

/// An [`AuditSink`] that emits each audit event as an OpenTelemetry span event
/// on the current trace context.
#[cfg(feature = "otel")]
#[cfg_attr(docsrs, doc(cfg(feature = "otel")))]
#[derive(Debug, Default, Clone, Copy)]
pub struct OtelAuditSink;

#[cfg(feature = "otel")]
impl AuditSink for OtelAuditSink {
    fn emit(&self, event: AuditEvent) {
        use opentelemetry::trace::TraceContextExt;
        use opentelemetry::KeyValue;

        let cx = opentelemetry::Context::current();
        let span = cx.span();
        if span.span_context().is_valid() {
            let mut attributes = vec![
                KeyValue::new("a1.event_id", event.event_id),
                KeyValue::new("a1.outcome", event.outcome.to_string()),
                KeyValue::new("a1.principal", event.principal_pk_hex),
                KeyValue::new("a1.executor", event.executor_pk_hex),
                KeyValue::new("a1.intent", event.intent_hex),
                KeyValue::new("a1.depth", event.chain_depth as i64),
            ];
            if let Some(fp) = event.chain_fingerprint {
                attributes.push(KeyValue::new("a1.chain_fingerprint", fp));
            }
            if let Some(err) = event.error_message {
                attributes.push(KeyValue::new("a1.error", err));
            }
            if let Some(policy) = event.policy_name {
                attributes.push(KeyValue::new("a1.policy", policy));
            }
            if let Some(size) = event.batch_size {
                attributes.push(KeyValue::new("a1.batch_size", size as i64));
            }
            span.add_event("a1_audit", attributes);
        }
    }
}

// ── LogAuditSink ──────────────────────────────────────────────────────────────

/// An [`AuditSink`] that writes one NDJSON line per event to a configurable target
/// (defaults to stderr).
///
/// Suitable for local development and structured log pipelines that collect
/// from stdout/stderr (e.g., Fluentd, Vector, AWS CloudWatch agent).
#[derive(Debug, Clone, Copy)]
pub enum LogTarget {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone)]
pub struct LogAuditSink {
    target: LogTarget,
}

impl LogAuditSink {
    pub fn new(target: LogTarget) -> Self {
        Self { target }
    }
}

impl Default for LogAuditSink {
    fn default() -> Self {
        Self::new(LogTarget::Stderr)
    }
}

impl AuditSink for LogAuditSink {
    fn emit(&self, event: AuditEvent) {
        #[cfg(feature = "serde")]
        {
            if let Ok(json) = serde_json::to_string(&event) {
                match self.target {
                    LogTarget::Stdout => println!("{json}"),
                    LogTarget::Stderr => eprintln!("{json}"),
                }
            }
        }
        #[cfg(not(feature = "serde"))]
        {
            let text = format!(
                "a1 audit: outcome={} principal={} executor={} depth={}",
                event.outcome, event.principal_pk_hex, event.executor_pk_hex, event.chain_depth,
            );
            match self.target {
                LogTarget::Stdout => println!("{text}"),
                LogTarget::Stderr => eprintln!("{text}"),
            }
        }
    }
}

// ── CompositeAuditSink ────────────────────────────────────────────────────────

/// An [`AuditSink`] that fans events out to multiple downstream sinks.
///
/// All sinks receive every event; a panic in one sink does not prevent
/// delivery to the remaining sinks.
///
/// # Example
///
/// ```rust,ignore
/// use a1::audit::{CompositeAuditSink, LogAuditSink};
///
/// let sink = CompositeAuditSink::new()
///     .add(LogAuditSink)
///     .add(MyDatadogSink::new(api_key));
/// ```
pub struct CompositeAuditSink {
    sinks: Vec<Box<dyn AuditSink>>,
}

impl CompositeAuditSink {
    pub fn new() -> Self {
        Self { sinks: Vec::new() }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, sink: impl AuditSink + 'static) -> Self {
        self.sinks.push(Box::new(sink));
        self
    }
}

impl Default for CompositeAuditSink {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditSink for CompositeAuditSink {
    fn emit(&self, event: AuditEvent) {
        for sink in &self.sinks {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                sink.emit(event.clone());
            }));
            if let Err(e) = result {
                if let Some(msg) = e.downcast_ref::<&str>() {
                    eprintln!(
                        "a1 audit: panic in CompositeAuditSink downstream: {}",
                        msg
                    );
                } else if let Some(msg) = e.downcast_ref::<String>() {
                    eprintln!(
                        "a1 audit: panic in CompositeAuditSink downstream: {}",
                        msg
                    );
                } else {
                    eprintln!("a1 audit: panic in CompositeAuditSink downstream with unknown payload");
                }
            }
        }
    }
}

// ── SiemHttpAuditSink ─────────────────────────────────────────────────────────

/// A production-grade [`AuditSink`] that batches and transmits events to an
/// external SIEM (Splunk HEC, Datadog, Elasticsearch) via HTTP.
///
/// Internally uses a non-blocking MPSC channel to ensure the authorization
/// hot-path is strictly O(1) and never blocks on network I/O.
#[cfg(feature = "async")]
pub struct SiemHttpAuditSink {
    sender: tokio::sync::mpsc::UnboundedSender<AuditEvent>,
}

#[cfg(feature = "async")]
impl SiemHttpAuditSink {
    /// Initializes the SIEM exporter and spawns a background Tokio task to
    /// flush batches to the specified endpoint.
    pub fn new(endpoint: String, auth_token: String) -> Self {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<AuditEvent>();

        tokio::spawn(async move {
            let mut batch = Vec::with_capacity(100);
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if !batch.is_empty() {
                            let _ = Self::flush_batch(&endpoint, &auth_token, &mut batch).await;
                        }
                    }
                    event = receiver.recv() => {
                        match event {
                            Some(ev) => {
                                batch.push(ev);
                                if batch.len() >= 100 {
                                    let _ = Self::flush_batch(&endpoint, &auth_token, &mut batch).await;
                                }
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        Self { sender }
    }

    async fn flush_batch(
        endpoint: &str,
        auth_token: &str,
        batch: &mut Vec<AuditEvent>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use tokio::io::AsyncWriteExt;
        let body = serde_json::to_string(batch).unwrap_or_default();
        let url: url::Url = endpoint.parse()?;
        let host = url.host_str().unwrap_or("localhost").to_string();
        let port = url.port_or_known_default().unwrap_or(80);
        let path = format!("{}{}",
            url.path(),
            url.query().map(|q| format!("?{}", q)).unwrap_or_default()
        );
        let request = format!(
            "POST {} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nX-A1-Provenance: 64796f6c6f\r\nConnection: close\r\n\r\n{}",
            path, host, auth_token, body.len(), body
        );
        let addr = format!("{}:{}", host, port);
        let mut stream = tokio::net::TcpStream::connect(&addr).await?;
        stream.write_all(request.as_bytes()).await?;
        stream.flush().await?;
        batch.clear();
        Ok(())
    }
}

#[cfg(feature = "async")]
impl AuditSink for SiemHttpAuditSink {
    #[inline(always)]
    fn emit(&self, event: AuditEvent) {
        // Non-blocking send; drops the event if the background task panics/dies 
        // to prevent memory exhaustion in the hot path.
        let _ = self.sender.send(event);
    }
}

// ── AsyncAuditSink ────────────────────────────────────────────────────────────

/// Async version of [`AuditSink`] for Tokio-based services.
///
/// Requires `features = ["async"]`.
///
/// The canonical implementation wraps a `tokio::sync::mpsc::UnboundedSender`
/// so `emit_async` is instantaneous (it only enqueues) and a background task
/// drains the channel to your SIEM endpoint.
#[cfg(feature = "async")]
pub mod r#async {
    use super::{AuditEvent, AuditSink};
    use async_trait::async_trait;

    #[async_trait]
    pub trait AsyncAuditSink: Send + Sync {
        async fn emit_async(&self, event: AuditEvent);
    }

    /// Adapts any synchronous [`AuditSink`] to the [`AsyncAuditSink`] interface
    /// by calling `emit` directly (no spawn). Appropriate when the underlying
    /// sink is non-blocking (e.g., an mpsc channel send).
    pub struct SyncAuditAdapter<S>(pub std::sync::Arc<S>);

    #[async_trait]
    impl<S: AuditSink + 'static> AsyncAuditSink for SyncAuditAdapter<S> {
        async fn emit_async(&self, event: AuditEvent) {
            self.0.emit(event);
        }
    }
}
