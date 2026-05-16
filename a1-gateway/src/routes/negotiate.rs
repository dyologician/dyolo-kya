use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};

use a1::{
    cert::CertBuilder,
    intent::Intent,
    negotiate::{CapabilityRequest, DelegationOffer, NegotiationResult},
    Clock, SystemClock,
};

use crate::state::AppState;

/// POST /v1/negotiate
///
/// Agent-to-agent delegation negotiation. An agent sends a signed
/// `CapabilityRequest` to this endpoint; the gateway issues a scoped
/// `DelegationCert` from its signing identity and returns a `DelegationOffer`.
///
/// # Request freshness
///
/// Requests older than 300 seconds are rejected to prevent replay.
///
/// # Capability scoping
///
/// The issued cert is scoped to the `intent_name` in the request. The
/// requesting agent can use the returned cert to build a delegation chain
/// against this gateway's principal key.
///
/// # Configuring allowed capabilities
///
/// Set `A1_NEGOTIATE_ALLOW_ALL=1` to allow any capability request (dev/staging).
/// In production, the gateway issues certs only for capabilities listed in
/// `A1_NEGOTIATE_CAPABILITIES` (comma-separated). If the env var is unset and
/// `A1_NEGOTIATE_ALLOW_ALL` is not set, this endpoint returns 403.
pub async fn handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CapabilityRequest>,
) -> impl IntoResponse {
    match negotiate_inner(&state, req).await {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(e) => {
            let status = StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::FORBIDDEN);
            (
                status,
                Json(serde_json::json!({
                    "error": e.to_string(),
                    "error_code": e.error_code(),
                })),
            )
                .into_response()
        }
    }
}

async fn negotiate_inner(
    state: &AppState,
    req: CapabilityRequest,
) -> Result<NegotiationResult, a1::A1Error> {
    let now = SystemClock.unix_now();

    req.verify_signature()?;
    req.verify_freshness(now, 300)?;

    check_capability_policy(&req.requested_capabilities)?;

    let delegate_pk_bytes = hex::decode(&req.requester_pk_hex)
        .map_err(|_| a1::A1Error::WireFormatError("invalid requester_pk_hex".into()))?;
    let pk_arr: [u8; 32] = delegate_pk_bytes
        .try_into()
        .map_err(|_| a1::A1Error::WireFormatError("requester_pk_hex must be 32 bytes".into()))?;
    let delegate_pk = ed25519_dalek::VerifyingKey::from_bytes(&pk_arr)
        .map_err(|_| a1::A1Error::WireFormatError("invalid requester Ed25519 key".into()))?;

    let mut intent = Intent::new(&req.intent_name)?;
    let intent_hash = intent.hash();

    let expiry = now + req.ttl_secs.min(86400);
    let cert = CertBuilder::new(delegate_pk, intent_hash, now, expiry).sign(&state.signing_identity);

    let fingerprint_hex = cert.fingerprint_hex();
    let offer = DelegationOffer::build(&state.signing_identity, &req, cert.clone(), now, 120)?;

    Ok(NegotiationResult {
        fingerprint_hex,
        offerer_did: offer.offerer_did.clone(),
        requester_did: req.requester_did.clone(),
        cert,
        offer,
    })
}

fn check_capability_policy(requested: &[String]) -> Result<(), a1::A1Error> {
    if std::env::var("A1_NEGOTIATE_ALLOW_ALL").as_deref() == Ok("1") {
        return Ok(());
    }

    let allowed_raw = std::env::var("A1_NEGOTIATE_CAPABILITIES").unwrap_or_default();
    if allowed_raw.is_empty() {
        return Err(a1::A1Error::PolicyViolation(
            "negotiation not configured on this gateway — set A1_NEGOTIATE_CAPABILITIES".into(),
        ));
    }

    let allowed: std::collections::HashSet<&str> =
        allowed_raw.split(',').map(str::trim).collect();

    for cap in requested {
        if !allowed.contains(cap.as_str()) {
            return Err(a1::A1Error::PolicyViolation(format!(
                "capability '{}' is not offered by this gateway",
                cap
            )));
        }
    }
    Ok(())
}
