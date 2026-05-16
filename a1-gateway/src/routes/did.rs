use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use a1::{
    did::{AgentDid, DidDocument, VerifiableCredential},
    NarrowingMatrix,
};

use crate::state::AppState;

// ── DID Resolution ────────────────────────────────────────────────────────────

/// GET /v1/did/{pk_hex}
///
/// Resolve a `did:a1:` DID Document from an Ed25519 public key.
/// The key is the hex-encoded 32-byte verifying key.
pub async fn resolve_handler(
    State(state): State<Arc<AppState>>,
    Path(pk_hex): Path<String>,
) -> impl IntoResponse {
    let pk_bytes = match hex::decode(&pk_hex) {
        Ok(b) => b,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "pk_hex must be valid hex" })),
            )
                .into_response();
        }
    };
    let arr: [u8; 32] = match pk_bytes.try_into() {
        Ok(a) => a,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "pk_hex must be 32 bytes" })),
            )
                .into_response();
        }
    };
    let vk = match ed25519_dalek::VerifyingKey::from_bytes(&arr) {
        Ok(k) => k,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "invalid Ed25519 public key" })),
            )
                .into_response();
        }
    };

    let doc = DidDocument::for_identity(&vk);
    (StatusCode::OK, Json(doc)).into_response()
}

/// GET /v1/did/gateway
///
/// Return the DID Document for the gateway's own signing identity.
pub async fn gateway_did_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let doc = DidDocument::for_identity(&state.signing_identity.verifying_key());
    (StatusCode::OK, Json(doc)).into_response()
}

// ── VC Issuance ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct IssueVcRequest {
    /// Hex-encoded Ed25519 public key of the credential subject (the agent).
    pub subject_pk_hex: String,
    /// Human-readable passport namespace.
    pub passport_namespace: String,
    /// Capabilities to include in the VC.
    pub capabilities: Vec<String>,
    /// VC lifetime in seconds (default: 86400).
    #[serde(default = "default_vc_ttl")]
    pub ttl_seconds: u64,
    /// Chain fingerprint to bind this VC to (hex, optional).
    #[serde(default)]
    pub chain_fingerprint_hex: Option<String>,
}

fn default_vc_ttl() -> u64 {
    86400
}

#[derive(Debug, Serialize)]
pub struct IssueVcResponse {
    pub credential: VerifiableCredential,
    pub subject_did: String,
    pub issuer_did: String,
}

/// POST /v1/vc/issue
///
/// Issue a W3C Verifiable Credential asserting that a subject agent holds
/// specific capabilities. Signed by the gateway's signing identity.
///
/// Requires `Authorization: Bearer <A1_ADMIN_SECRET>`.
pub async fn issue_vc_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<IssueVcRequest>,
) -> impl IntoResponse {
    match issue_vc_inner(&state, req) {
        Ok(resp) => (StatusCode::CREATED, Json(resp)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

fn issue_vc_inner(state: &AppState, req: IssueVcRequest) -> anyhow::Result<IssueVcResponse> {
    let pk_bytes = hex::decode(&req.subject_pk_hex)?;
    let arr: [u8; 32] = pk_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("subject_pk_hex must be 32 bytes"))?;
    let vk = ed25519_dalek::VerifyingKey::from_bytes(&arr)?;
    let subject_did = AgentDid::from_key(&vk);
    let issuer_did = AgentDid::from_key(&state.signing_identity.verifying_key());

    let chain_fp: [u8; 32] = match &req.chain_fingerprint_hex {
        Some(hex_str) => {
            let bytes = hex::decode(hex_str)?;
            bytes.try_into().map_err(|_| anyhow::anyhow!("chain fingerprint must be 32 bytes"))?
        }
        None => [0u8; 32],
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let caps: Vec<&str> = req.capabilities.iter().map(String::as_str).collect();

    let vc = VerifiableCredential::issue_capability(
        &state.signing_identity,
        &subject_did,
        &req.passport_namespace,
        &caps,
        now,
        now + req.ttl_seconds,
        &chain_fp,
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;

    Ok(IssueVcResponse {
        subject_did: subject_did.to_string(),
        issuer_did: issuer_did.to_string(),
        credential: vc,
    })
}

// ── VC Verification ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct VerifyVcRequest {
    pub credential: VerifiableCredential,
}

#[derive(Debug, Serialize)]
pub struct VerifyVcResponse {
    pub valid: bool,
    pub issuer_did: String,
    pub subject_did: String,
    pub passport_namespace: String,
    pub capabilities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// POST /v1/vc/verify
///
/// Verify a W3C Verifiable Credential's Ed25519 signature. Returns the
/// decoded subject claims on success.
pub async fn verify_vc_handler(
    Json(req): Json<VerifyVcRequest>,
) -> impl IntoResponse {
    match req.credential.verify() {
        Ok(()) => {
            let subject = &req.credential.credential_subject;
            (
                StatusCode::OK,
                Json(VerifyVcResponse {
                    valid: true,
                    issuer_did: req.credential.issuer.clone(),
                    subject_did: subject.id.clone(),
                    passport_namespace: subject.passport_namespace.clone(),
                    capabilities: subject.capabilities.clone(),
                    error: None,
                }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::OK,
            Json(VerifyVcResponse {
                valid: false,
                issuer_did: req.credential.issuer.clone(),
                subject_did: req.credential.credential_subject.id.clone(),
                passport_namespace: req.credential.credential_subject.passport_namespace.clone(),
                capabilities: Vec::new(),
                error: Some(e.to_string()),
            }),
        )
            .into_response(),
    }
}
