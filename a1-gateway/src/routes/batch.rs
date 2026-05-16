use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use a1::{wire::SignedChain, Intent, MerkleProof, SystemClock};

use crate::state::AppState;

pub const MAX_BATCH_SIZE: usize = 256;

#[derive(Debug, Deserialize)]
pub struct BatchAuthorizeRequest {
    pub chain: SignedChain,
    pub executor_pk_hex: String,
    pub intents: Vec<IntentRequest>,
}

#[derive(Debug, Deserialize)]
pub struct IntentRequest {
    pub name: String,
    #[serde(default)]
    pub params: std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct BatchAuthorizeResponse {
    pub all_authorized: bool,
    pub authorized_count: usize,
    pub total_count: usize,
    pub results: Vec<BatchItem>,
}

#[derive(Debug, Serialize)]
pub struct BatchItem {
    pub intent_name: String,
    pub authorized: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<&'static str>,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
    error_code: &'static str,
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BatchAuthorizeRequest>,
) -> impl IntoResponse {
    match batch_inner(&state, req).await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "batch authorization pre-check failed");
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorBody {
                    error: e.to_string(),
                    error_code: e.error_code(),
                }),
            )
                .into_response()
        }
    }
}

async fn batch_inner(
    state: &AppState,
    req: BatchAuthorizeRequest,
) -> Result<BatchAuthorizeResponse, a1::A1Error> {
    if req.intents.len() > MAX_BATCH_SIZE {
        return Err(a1::A1Error::WireFormatError(format!(
            "Batch size {} exceeds maximum allowed of {}",
            req.intents.len(),
            MAX_BATCH_SIZE
        )));
    }

    let pk_bytes = hex::decode(&req.executor_pk_hex)
        .map_err(|_| a1::A1Error::WireFormatError("invalid executor_pk_hex".into()))?;
    let pk_arr: [u8; 32] = pk_bytes
        .try_into()
        .map_err(|_| a1::A1Error::WireFormatError("executor_pk must be 32 bytes".into()))?;
    let executor_pk = ed25519_dalek::VerifyingKey::from_bytes(&pk_arr)
        .map_err(|_| a1::A1Error::WireFormatError("invalid executor Ed25519 key".into()))?;

    let chain = req.chain.into_chain_with_drift(15)?;

    let mut intent_pairs: Vec<(a1::IntentHash, MerkleProof)> =
        Vec::with_capacity(req.intents.len());
    for i in &req.intents {
        // Fix: Propagate Intent instantiation errors correctly to prevent compilation failure
        let mut intent = Intent::try_new(&i.name)?;
        for (k, v) in &i.params {
            intent = intent.try_param(k, v)?;
        }
        intent_pairs.push((intent.hash(), MerkleProof::default()));
    }

    let batch = chain
        .authorize_batch_async(
            &executor_pk,
            &intent_pairs,
            &SystemClock,
            &*state.revocation,
            &*state.nonces,
        )
        .await;

    let results: Vec<BatchItem> = req
        .intents
        .iter()
        .enumerate()
        .map(|(i, intent_req)| {
            let receipt = &batch.receipts[i];
            let error = &batch.errors[i];
            BatchItem {
                intent_name: intent_req.name.clone(),
                authorized: receipt.is_some(),
                chain_fingerprint: receipt.as_ref().map(|r| hex::encode(r.chain_fingerprint)),
                error: error.as_ref().map(|e| e.to_string()),
                error_code: error.as_ref().map(|e| e.error_code()),
            }
        })
        .collect();

    let authorized_count = batch.authorized_count();
    let total_count = req.intents.len();

    Ok(BatchAuthorizeResponse {
        all_authorized: batch.all_authorized,
        authorized_count,
        total_count,
        results,
    })
}
