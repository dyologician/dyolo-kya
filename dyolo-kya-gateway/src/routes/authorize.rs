use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use dyolo_kya::{
    wire::{SignedChain, VerifiedToken},
    Intent, MerkleProof, SystemClock,
};

use crate::state::AppState;

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Serialize)]
pub struct AuthorizeResponse {
    pub authorized: bool,
    pub chain_depth: usize,
    pub chain_fingerprint: String,
    pub verified_at_unix: u64,
    pub error_code: Option<String>,
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
    headers: HeaderMap,
    Json(req): Json<AuthorizeRequest>,
) -> impl IntoResponse {
    // Optional Bearer token auth for /v1/authorize if the flag is enabled
    if std::env::var("DYOLO_REQUIRE_AUTH_ON_AUTHORIZE").as_deref() == Ok("1") {
        if let Some(secret) = &state.admin_secret {
            let auth_header = headers.get("Authorization").and_then(|h| h.to_str().ok());
            let expected = format!("Bearer {}", secret);
            if auth_header != Some(expected.as_str()) {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(ErrorBody {
                        error: "invalid or missing admin secret".into(),
                        error_code: "UNAUTHORIZED",
                    }),
                )
                    .into_response();
            }
        }
    }

    match authorize_inner(&state, req).await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
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

async fn authorize_inner(
    state: &AppState,
    req: AuthorizeRequest,
) -> Result<AuthorizeResponse, dyolo_kya::KyaError> {
    let pk_bytes = hex::decode(&req.executor_pk_hex).map_err(|_| {
        dyolo_kya::KyaError::WireFormatError("invalid executor_pk_hex".into())
    })?;
    let pk_arr: [u8; 32] = pk_bytes.try_into().map_err(|_| {
        dyolo_kya::KyaError::WireFormatError("executor_pk must be 32 bytes".into())
    })?;
    let executor_pk = ed25519_dalek::VerifyingKey::from_bytes(&pk_arr).map_err(|_| {
        dyolo_kya::KyaError::WireFormatError("invalid executor Ed25519 key".into())
    })?;

    // Fix: Propagate errors from Intent::try_new and .try_param
    let mut intent = Intent::try_new(&req.intent_name)?;
    for (k, v) in &req.intent_params {
        intent = intent.try_param(k, v)?;
    }
    
    let intent_hash = intent.hash();
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

    Ok(AuthorizeResponse {
        authorized: true,
        chain_depth: action.receipt.chain_depth,
        chain_fingerprint: action.receipt.fingerprint_hex(),
        verified_at_unix: action.receipt.verified_at_unix,
        error_code: None,
        token,
    })
}