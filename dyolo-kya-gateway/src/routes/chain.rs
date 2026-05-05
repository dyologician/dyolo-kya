use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};

use dyolo_kya::wire::VerifiedToken;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct VerifyTokenRequest {
    pub token: VerifiedToken,
}

#[derive(Debug, Serialize)]
pub struct VerifyTokenResponse {
    pub valid:             bool,
    pub chain_depth:       usize,
    pub chain_fingerprint: String,
    pub verified_at_unix:  u64,
}

pub async fn verify_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VerifyTokenRequest>,
) -> impl IntoResponse {
    match req.token.verify(&state.mac_key) {
        Ok(receipt) => (StatusCode::OK, Json(VerifyTokenResponse {
            valid:             true,
            chain_depth:       receipt.chain_depth,
            chain_fingerprint: receipt.fingerprint_hex(),
            verified_at_unix:  receipt.verified_at_unix,
        })).into_response(),
        Err(e) => (StatusCode::UNAUTHORIZED, Json(serde_json::json!({
            "valid": false, "error": e.to_string()
        }))).into_response(),
    }
}
