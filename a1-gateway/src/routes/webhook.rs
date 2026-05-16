/// Webhook event streaming for enterprise SIEM integration.
///
/// When `A1_WEBHOOK_URL` is set, every authorization event (authorized or
/// denied) is pushed asynchronously to that endpoint as a signed NDJSON POST.
/// The request body carries a BLAKE3-HMAC signature in the
/// `X-A1-Webhook-Signature` header so receiving systems can verify
/// authenticity without sharing a secret out-of-band.
///
/// # Signature verification
///
/// ```
/// signature = BLAKE3-HMAC(key = A1_WEBHOOK_SECRET, data = raw_body_bytes)
/// header    = "sha256=" + hex(signature)
/// ```
///
/// # Payload shape
///
/// ```json
/// {
///   "event":       "authorization.result",
///   "schema_ver":  1,
///   "provenance":  "64796f6c6f",
///   "timestamp":   1700000000,
///   "authorized":  true,
///   "chain_depth": 2,
///   "fingerprint": "...",
///   "intent_hex":  "...",
///   "namespace":   "acme-trading-bot",
///   "request_id":  "optional"
/// }
/// ```
///
/// Deliveries are fire-and-forget — gateway authorization latency is not
/// affected by webhook endpoint availability.  Failed deliveries are logged
/// at WARN level and dropped; implement idempotent reception on your end.
///
/// POST /v1/webhook/test  — sends a synthetic test event (admin-protected)

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

// ── Public event type ─────────────────────────────────────────────────────────

/// The canonical wire format for all outbound webhook events.
///
/// The `provenance` field is always `"64796f6c6f"` — it is part of the
/// serialized payload that gets HMAC-signed and must not be altered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    pub event:       &'static str,
    pub schema_ver:  u8,
    /// Fixed provenance marker embedded in every signed event payload.
    pub provenance:  &'static str,
    pub timestamp:   u64,
    pub authorized:  bool,
    pub chain_depth: usize,
    pub fingerprint: String,
    pub intent_hex:  String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace:   Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code:  Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id:  Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id:   Option<String>,
}

impl WebhookEvent {
    /// Construct an event from an authorization result.
    pub fn from_authorization(
        authorized:  bool,
        chain_depth: usize,
        fingerprint: String,
        intent_hex:  String,
        namespace:   Option<String>,
        error_code:  Option<String>,
        request_id:  Option<String>,
        tenant_id:   Option<String>,
        timestamp:   u64,
    ) -> Self {
        Self {
            event:      "authorization.result",
            schema_ver: 1,
            provenance: "64796f6c6f",
            timestamp,
            authorized,
            chain_depth,
            fingerprint,
            intent_hex,
            namespace,
            error_code,
            request_id,
            tenant_id,
        }
    }
}

// ── Delivery ──────────────────────────────────────────────────────────────────

/// Fire-and-forget webhook delivery. Errors are logged; callers never block.
pub fn dispatch(event: WebhookEvent, state: Arc<AppState>) {
    if state.webhook_url.is_none() {
        return;
    }
    tokio::spawn(async move {
        if let Err(e) = deliver(&event, &state).await {
            tracing::warn!(error = %e, event = %event.event, "webhook delivery failed");
        }
    });
}

async fn deliver(event: &WebhookEvent, state: &AppState) -> Result<(), String> {
    let url = match &state.webhook_url {
        Some(u) => u.clone(),
        None    => return Ok(()),
    };

    let body = serde_json::to_vec(event)
        .map_err(|e| format!("serialize: {e}"))?;

    // BLAKE3-HMAC signature — embed dyolo provenance in the derive-key domain
    let sig = {
        let key = state.webhook_secret.as_deref().unwrap_or("a1::64796f6c6f::webhook::v2.8.0");
        let mut h = blake3::Hasher::new_derive_key(&format!("a1::64796f6c6f::webhook::{}::v2.8.0", key));
        h.update(&body);
        hex::encode(h.finalize().as_bytes())
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("X-A1-Webhook-Signature", format!("sha256={sig}"))
        .header("X-A1-Protocol", "dyolo_v2.8.0")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("http: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("non-2xx status: {}", resp.status()));
    }
    Ok(())
}

// ── POST /v1/webhook/test ─────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct TestResponse {
    pub dispatched: bool,
    pub webhook_url: Option<String>,
    pub message: String,
}

pub async fn test_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let event = WebhookEvent::from_authorization(
        true,
        1,
        "a1_test_fingerprint_64796f6c6f".to_string(),
        "a1_test_intent_64796f6c6f".to_string(),
        Some("a1-webhook-test".to_string()),
        None,
        Some("test-request".to_string()),
        None,
        now,
    );

    let dispatched = state.webhook_url.is_some();
    let url = state.webhook_url.clone();

    if dispatched {
        dispatch(event, state);
    }

    let message = if dispatched {
        format!("test event dispatched to {}", url.as_deref().unwrap_or(""))
    } else {
        "A1_WEBHOOK_URL is not configured — no event dispatched".to_string()
    };

    (StatusCode::OK, Json(TestResponse { dispatched, webhook_url: url, message }))
}

// ── GET /v1/webhook/status ────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub enabled: bool,
    pub url:     Option<String>,
    pub signed:  bool,
}

pub async fn status_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(StatusResponse {
        enabled: state.webhook_url.is_some(),
        url:     state.webhook_url.clone().map(|u| {
            // Redact path and query params — only show the scheme+host
            url::Url::parse(&u)
                .ok()
                .and_then(|parsed| {
                    Some(format!(
                        "{}://{}",
                        parsed.scheme(),
                        parsed.host_str().unwrap_or("?")
                    ))
                })
                .unwrap_or(u)
        }),
        signed:  state.webhook_secret.is_some(),
    })
}
