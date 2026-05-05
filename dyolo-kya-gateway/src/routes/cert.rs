use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use dyolo_kya::{CertBuilder, CertExtensions, ExtValue, Intent, IntentTree};

use crate::state::AppState;

static IDEMPOTENCY_CACHE: OnceLock<Mutex<HashMap<String, (u64, IssueCertResponse)>>> = OnceLock::new();
static BATCH_IDEMPOTENCY_CACHE: OnceLock<Mutex<HashMap<String, (u64, IssueBatchResponse)>>> = OnceLock::new();

// ── Issue ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct IssueCertRequest {
    pub delegate_pk_hex: String,
    pub intents:         Vec<IntentSpec>,
    #[serde(default = "default_ttl")]
    pub ttl_seconds:     u64,
    #[serde(default = "default_max_depth")]
    pub max_depth:       u8,
    #[serde(default)]
    pub extensions:      std::collections::HashMap<String, serde_json::Value>,
}

fn default_ttl()       -> u64 { 3600 }
fn default_max_depth() -> u8  { 16 }

#[derive(Debug, Deserialize)]
pub struct IntentSpec {
    pub name:   String,
    #[serde(default)]
    pub params: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IssueCertResponse {
    pub cert:            dyolo_kya::DelegationCert,
    pub fingerprint_hex: String,
    pub scope_root_hex:  String,
}

pub async fn issue_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<IssueCertRequest>,
) -> impl IntoResponse {
    let idempotency_key = headers.get("Idempotency-Key").and_then(|h| h.to_str().ok().map(String::from));

    if let Some(key) = &idempotency_key {
        let cache = IDEMPOTENCY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        let mut map = cache.lock().unwrap();
        let now = unix_now();
        
        // Basic eviction to prevent unbounded growth
        if map.len() > 1000 {
            map.retain(|_, (exp, _)| *exp > now);
        }
        
        if let Some((exp, resp)) = map.get(key) {
            if *exp > now {
                return (StatusCode::OK, Json(resp.clone())).into_response();
            }
        }
    }

    match issue_inner(&state, req) {
        Ok(resp) => {
            if let Some(key) = idempotency_key {
                let cache = IDEMPOTENCY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
                let mut map = cache.lock().unwrap();
                map.insert(key, (unix_now() + 300, resp.clone())); // 5-minute TTL
            }
            (StatusCode::CREATED, Json(resp)).into_response()
        },
        Err(e)   => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        ).into_response(),
    }
}

fn issue_inner(state: &AppState, req: IssueCertRequest) -> anyhow::Result<IssueCertResponse> {
    let delegate_pk = pk_from_hex(&req.delegate_pk_hex)?;

    // Fix: Unroll map to correctly propagate intent construction errors using `?`
    let mut hashes = Vec::with_capacity(req.intents.len());
    for s in &req.intents {
        let mut intent = Intent::new(&s.name)?;
        for (k, v) in &s.params {
            intent = intent.try_param(k, v)?;
        }
        hashes.push(intent.hash());
    }

    let tree = IntentTree::build(hashes)?;
    let scope_root = tree.root();

    let now    = unix_now();
    let expiry = now + req.ttl_seconds;

    let mut ext = CertExtensions::new();
    for (k, v) in req.extensions {
        ext = ext.set_checked(k, ExtValue::from(v))
            .map_err(|e| anyhow::anyhow!("{e}"))?;
    }

    let cert = CertBuilder::new(delegate_pk, scope_root, now, expiry)
        .max_depth(req.max_depth)
        .extensions(ext)
        .sign(&state.signing_identity);

    Ok(IssueCertResponse {
        fingerprint_hex: cert.fingerprint_hex(),
        scope_root_hex:  hex::encode(scope_root),
        cert,
    })
}

// ── Issue Batch ───────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct IssueBatchRequest {
    pub requests: Vec<IssueCertRequest>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IssueBatchResponse {
    pub bundle: dyolo_kya::CertBundle,
    pub issued: Vec<IssueCertResponse>,
    pub total:  usize,
}

pub async fn issue_batch_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<IssueBatchRequest>,
) -> impl IntoResponse {
    let idempotency_key = headers.get("Idempotency-Key").and_then(|h| h.to_str().ok().map(String::from));

    if let Some(key) = &idempotency_key {
        let cache = BATCH_IDEMPOTENCY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        let mut map = cache.lock().unwrap();
        let now = unix_now();
        
        if map.len() > 1000 {
            map.retain(|_, (exp, _)| *exp > now);
        }
        
        if let Some((exp, resp)) = map.get(key) {
            if *exp > now {
                return (StatusCode::OK, Json(resp.clone())).into_response();
            }
        }
    }

    if req.requests.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "requests array must not be empty" })),
        ).into_response();
    }

    let mut issued: Vec<IssueCertResponse>       = Vec::with_capacity(req.requests.len());
    let mut certs:  Vec<dyolo_kya::DelegationCert> = Vec::with_capacity(req.requests.len());

    for single in req.requests {
        match issue_inner(&state, single) {
            Ok(resp) => {
                certs.push(resp.cert.clone());
                issued.push(resp);
            }
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": e.to_string() })),
                ).into_response();
            }
        }
    }

    // Fix: Instantiating the CertBundle explicitly bypasses the missing clock argument bug on `from_certs`
    let bundle = dyolo_kya::CertBundle { certs, issued_at: unix_now() };
    let total  = issued.len();
    let response = IssueBatchResponse { bundle, issued, total };

    if let Some(key) = idempotency_key {
        let cache = BATCH_IDEMPOTENCY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        let mut map = cache.lock().unwrap();
        map.insert(key, (unix_now() + 300, response.clone()));
    }

    (StatusCode::CREATED, Json(response)).into_response()
}

// ── Revoke ────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RevokeCertRequest {
    pub fingerprint_hex: String,
}

pub async fn revoke_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RevokeCertRequest>,
) -> impl IntoResponse {
    let fp = match parse_fp(&req.fingerprint_hex) {
        Ok(f) => f,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e }))).into_response(),
    };
    match state.revocation.revoke(&fp).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "revoked": req.fingerprint_hex }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

// ── Inspect ───────────────────────────────────────────────────────────────────

pub async fn inspect_handler(
    State(state): State<Arc<AppState>>,
    Path(fingerprint): Path<String>,
) -> impl IntoResponse {
    let fp = match parse_fp(&fingerprint) {
        Ok(f) => f,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e }))).into_response(),
    };
    let revoked = state.revocation.is_revoked(&fp).await.unwrap_or(false);
    (StatusCode::OK, Json(serde_json::json!({ "fingerprint": fingerprint, "revoked": revoked }))).into_response()
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn pk_from_hex(h: &str) -> anyhow::Result<ed25519_dalek::VerifyingKey> {
    let bytes = hex::decode(h)?;
    let arr: [u8; 32] = bytes.try_into().map_err(|_| anyhow::anyhow!("pk must be 32 bytes"))?;
    ed25519_dalek::VerifyingKey::from_bytes(&arr).map_err(Into::into)
}

fn parse_fp(h: &str) -> Result<[u8; 32], String> {
    hex::decode(h)
        .map_err(|_| "invalid hex".to_string())?
        .try_into()
        .map_err(|_| "fingerprint must be 32 bytes".to_string())
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock before epoch")
        .as_secs()
}

// ── Revoke Batch (CRL) ────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct RevokeBatchRequest {
    pub fingerprints: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct RevokeBatchResponse {
    pub revoked_count: usize,
    pub failed:        Vec<String>,
}

pub async fn revoke_batch_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RevokeBatchRequest>,
) -> impl IntoResponse {
    let mut fingerprints: Vec<[u8; 32]> = Vec::with_capacity(req.fingerprints.len());
    let mut failed: Vec<String> = Vec::new();

    for fp_hex in &req.fingerprints {
        match hex::decode(fp_hex).ok().and_then(|b| b.try_into().ok()) {
            Some(bytes) => fingerprints.push(bytes),
            None => failed.push(fp_hex.clone()),
        }
    }

    match state.revocation.revoke_batch(&fingerprints).await {
        Ok(()) => (
            StatusCode::OK,
            Json(RevokeBatchResponse {
                revoked_count: fingerprints.len(),
                failed,
            }),
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ).into_response(),
    }
}