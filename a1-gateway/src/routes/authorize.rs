use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use a1::{
    cert_extensions::ExtValue,
    wire::{SignedChain, VerifiedToken},
    Intent, MerkleProof, NarrowingMatrix, SystemClock,
};

use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct AuthorizeRequest {
    pub chain: SignedChain,
    pub intent_name: String,
    #[serde(default)]
    pub intent_params: std::collections::HashMap<String, String>,
    pub executor_pk_hex: String,
    #[serde(default)]
    pub return_token: bool,
    #[serde(default)]
    pub request_id: Option<String>,
}

/// Receipt sub-object included in every authorization response.
///
/// Field names align with the SDK passport client expectations so that
/// `PassportClient`, `withDyoloPassport`, and `a1.WithPassport` can consume
/// the gateway response directly without field-name translation.
///
/// `passport_namespace`, `capability_mask_hex`, and `narrowing_commitment_hex`
/// are populated when the chain carries `dyolo.passport.*` cert extensions.
/// Non-passport chains receive the structural receipt with empty passport fields;
/// all SDK passport clients handle this gracefully.
#[derive(Debug, Serialize)]
pub struct AuthorizeReceipt {
    pub chain_depth: usize,
    pub fingerprint_hex: String,
    pub verified_at_unix: u64,
    pub passport_namespace: String,
    pub capability_mask_hex: String,
    pub narrowing_commitment_hex: String,
}

#[derive(Debug, Serialize)]
pub struct AuthorizeResponse {
    pub authorized: bool,
    pub chain_depth: usize,
    pub chain_fingerprint: String,
    pub verified_at_unix: u64,
    pub error_code: Option<String>,
    pub receipt: AuthorizeReceipt,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<VerifiedToken>,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
    error_code: &'static str,
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AuthorizeRequest>,
) -> impl IntoResponse {
    use a1::Clock;
    let request_id  = req.request_id.clone();
    let intent_name = req.intent_name.clone();
    let clock       = a1::SystemClock;
    let now         = clock.unix_now();

    match authorize_inner(&state, req).await {
        Ok(resp) => {
            // intent_hex: Blake3 of the intent name for SIEM correlation
            let intent_hex = hex::encode(
                blake3::hash(intent_name.as_bytes()).as_bytes()
            );
            crate::routes::webhook::dispatch(
                crate::routes::webhook::WebhookEvent::from_authorization(
                    true,
                    resp.chain_depth,
                    resp.chain_fingerprint.clone(),
                    intent_hex,
                    Some(resp.receipt.passport_namespace.clone()).filter(|s| !s.is_empty()),
                    None,
                    request_id,
                    None,
                    now,
                ),
                state,
            );
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(e) => {
            tracing::warn!(error = %e, "authorization failed");
            let code = e.error_code();
            let status = if e.is_transient_storage_failure() {
                StatusCode::SERVICE_UNAVAILABLE
            } else {
                StatusCode::FORBIDDEN
            };
            (
                status,
                Json(ErrorBody {
                    error: e.to_string(),
                    error_code: code,
                }),
            )
                .into_response()
        }
    }
}

/// Extract passport-specific fields from the terminal cert's extensions.
///
/// Returns `(namespace, capability_mask_hex, narrowing_commitment_hex)`.
/// All fields are empty strings when the chain carries no passport extensions.
fn extract_passport_fields(chain: &SignedChain) -> (String, String, String) {
    let last_cert = match chain.certs.last() {
        Some(c) => c,
        None => return (String::new(), String::new(), String::new()),
    };

    let namespace = match last_cert.extensions.get("dyolo.passport.namespace") {
        Some(ExtValue::Str(s)) => s.clone(),
        _ => String::new(),
    };

    let mask_hex = match last_cert.extensions.get("dyolo.passport.mask") {
        Some(ExtValue::Str(s)) => s.clone(),
        _ => String::new(),
    };

    let commitment_hex = if !mask_hex.is_empty() {
        NarrowingMatrix::from_hex(&mask_hex)
            .map(|m| hex::encode(m.commitment()))
            .unwrap_or_default()
    } else {
        String::new()
    };

    (namespace, mask_hex, commitment_hex)
}

/// Inner authorization logic shared between `/v1/authorize` and `/v1/passport/authorize`.
///
/// Exposed as `pub(crate)` so the passport route handler can delegate to the
/// same verification pipeline without duplicating logic.
pub(crate) async fn authorize_inner(
    state: &AppState,
    req: AuthorizeRequest,
) -> Result<AuthorizeResponse, a1::A1Error> {
    let pk_bytes = hex::decode(&req.executor_pk_hex)
        .map_err(|_| a1::A1Error::WireFormatError("invalid executor_pk_hex".into()))?;
    let pk_arr: [u8; 32] = pk_bytes
        .try_into()
        .map_err(|_| a1::A1Error::WireFormatError("executor_pk must be 32 bytes".into()))?;
    let executor_pk = ed25519_dalek::VerifyingKey::from_bytes(&pk_arr)
        .map_err(|_| a1::A1Error::WireFormatError("invalid executor Ed25519 key".into()))?;

    let mut intent = Intent::try_new(&req.intent_name)?;
    for (k, v) in &req.intent_params {
        intent = intent.try_param(k, v)?;
    }

    let intent_hash = intent.hash();

    let (ext_namespace, capability_mask_hex, narrowing_commitment_hex) =
        extract_passport_fields(&req.chain);

    let chain = req.chain.into_chain_with_drift(15)?;

    let action = chain
        .authorize_async(
            &executor_pk,
            &intent_hash,
            &MerkleProof::default(),
            &SystemClock,
            &*state.revocation,
            &*state.nonces,
        )
        .await?;

    let token = req
        .return_token
        .then(|| VerifiedToken::sign(&action.receipt, &state.mac_key));

    let fingerprint = action.receipt.fingerprint_hex();
    let depth = action.receipt.chain_depth;
    let verified_at = action.receipt.verified_at_unix;

    let passport_namespace = action
        .receipt
        .namespace
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(String::from)
        .unwrap_or(ext_namespace);

    let capability_mask_hex = if !capability_mask_hex.is_empty() {
        capability_mask_hex
    } else {
        "00".repeat(32)
    };

    let narrowing_commitment_hex = if !narrowing_commitment_hex.is_empty() {
        narrowing_commitment_hex
    } else {
        hex::encode(NarrowingMatrix::EMPTY.commitment())
    };

    let receipt = AuthorizeReceipt {
        chain_depth: depth,
        fingerprint_hex: fingerprint.clone(),
        verified_at_unix: verified_at,
        passport_namespace,
        capability_mask_hex,
        narrowing_commitment_hex,
    };

    Ok(AuthorizeResponse {
        authorized: true,
        chain_depth: depth,
        chain_fingerprint: fingerprint,
        verified_at_unix: verified_at,
        error_code: None,
        receipt,
        token,
    })
}
