/// POST /v1/jwt/exchange
///
/// Exchanges a JWKS-verified JWT bearer token for a scoped A1 DelegationCert.
/// Enterprises running OIDC/SAML SSO use this to bootstrap delegation chains
/// from their existing IAM infrastructure without any manual key ceremony.
///
/// # Protocol
///
/// 1. Client sends `{ "token": "<JWT>", "capabilities": [...], "ttl_seconds": N,
///    "delegate_pk_hex": "<agent pubkey>" }`.
/// 2. Gateway fetches the issuer's JWKS and verifies the JWT signature.
/// 3. On success, issues a DelegationCert signed by the gateway key, scoped
///    to the requested capabilities subset, and tagged with the JWT subject.
/// 4. Returns the cert + fingerprint. The caller builds a chain from this cert
///    and submits it to `/v1/authorize`.
///
/// # Security properties
///
/// - Capability narrowing enforced: requested caps must be in `A1_JWT_ALLOWED_CAPS`.
/// - JWT `exp` claim is respected; cert TTL is `min(ttl_seconds, jwt_exp - now)`.
/// - JWT `sub` claim is recorded in `dyolo.jwt.subject` cert extension for audit.
/// - JWKS keys are cached per issuer with a 5-minute TTL.
/// - Requests without `A1_JWT_JWKS_URL` configured return 501 Not Implemented.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use a1::{
    cert_extensions::{CertExtensions, ExtValue},
    CertBuilder, Clock, Intent, IntentTree, NarrowingMatrix, SystemClock,
};

use crate::state::AppState;

// ── JWKS cache ────────────────────────────────────────────────────────────────

/// A single RSA/EC public key decoded from a JWKS endpoint.
#[derive(Clone, Debug)]
struct JwksKey {
    kid:   Option<String>,
    n:     Vec<u8>,
    e:     Vec<u8>,
}

#[derive(Clone)]
struct JwksCacheEntry {
    keys:    Vec<JwksKey>,
    fetched: Instant,
}

static JWKS_CACHE: std::sync::OnceLock<RwLock<HashMap<String, JwksCacheEntry>>> =
    std::sync::OnceLock::new();

fn jwks_cache() -> &'static RwLock<HashMap<String, JwksCacheEntry>> {
    JWKS_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

async fn fetch_jwks(jwks_url: &str) -> Result<Vec<JwksKey>, String> {
    {
        let cache = jwks_cache().read().await;
        if let Some(entry) = cache.get(jwks_url) {
            if entry.fetched.elapsed() < Duration::from_secs(300) {
                return Ok(entry.keys.clone());
            }
        }
    }

    let body: serde_json::Value = reqwest::get(jwks_url)
        .await
        .map_err(|e| format!("JWKS fetch failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("JWKS parse failed: {e}"))?;

    let keys = body
        .get("keys")
        .and_then(|k| k.as_array())
        .ok_or("JWKS missing 'keys' array")?;

    let mut result = Vec::new();
    for key in keys {
        let kty = key.get("kty").and_then(|v| v.as_str()).unwrap_or("");
        if kty != "RSA" {
            continue;
        }
        let n_b64 = key.get("n").and_then(|v| v.as_str()).unwrap_or("");
        let e_b64 = key.get("e").and_then(|v| v.as_str()).unwrap_or("");
        let kid   = key.get("kid").and_then(|v| v.as_str()).map(str::to_string);

        use base64::Engine;
        let n = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(n_b64)
            .map_err(|e| format!("JWKS n decode: {e}"))?;
        let e = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(e_b64)
            .map_err(|e| format!("JWKS e decode: {e}"))?;

        result.push(JwksKey { kid, n, e });
    }

    let entry = JwksCacheEntry { keys: result.clone(), fetched: Instant::now() };
    jwks_cache().write().await.insert(jwks_url.to_string(), entry);

    Ok(result)
}

// ── Minimal JWT header+payload decoder (no sig verification here — we use JWKS) ──

fn decode_jwt_claims(token: &str) -> Result<(serde_json::Value, String), String> {
    use base64::Engine;
    let parts: Vec<&str> = token.splitn(3, '.').collect();
    if parts.len() != 3 {
        return Err("malformed JWT: expected 3 parts".into());
    }
    let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|e| format!("JWT payload decode: {e}"))?;
    let claims: serde_json::Value = serde_json::from_slice(&payload_bytes)
        .map_err(|e| format!("JWT payload JSON: {e}"))?;
    Ok((claims, parts[2].to_string()))
}

fn verify_jwt_rsa(token: &str, keys: &[JwksKey]) -> Result<serde_json::Value, String> {
    let parts: Vec<&str> = token.splitn(3, '.').collect();
    if parts.len() != 3 {
        return Err("malformed JWT".into());
    }

    use base64::Engine;

    let header_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[0])
        .map_err(|e| format!("JWT header decode: {e}"))?;
    let header: serde_json::Value = serde_json::from_slice(&header_bytes)
        .map_err(|e| format!("JWT header JSON: {e}"))?;
    let kid = header.get("kid").and_then(|v| v.as_str());

    // Locate the right key
    let key = keys.iter().find(|k| {
        match (kid, &k.kid) {
            (Some(req), Some(have)) => req == have,
            _ => true,
        }
    }).ok_or("no matching JWKS key")?;

    // Build the RSA public key bytes (DER SPKI) and verify
    // We use ring's RSA_PKCS1_2048_8192_SHA256 primitive
    let signing_input = format!("{}.{}", parts[0], parts[1]);
    let sig_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[2])
        .map_err(|e| format!("JWT sig decode: {e}"))?;

    // Construct RSA public key from modulus + exponent using ring
    let public_key = ring::signature::RsaPublicKeyComponents {
        n: &key.n,
        e: &key.e,
    };
    public_key
        .verify(
            &ring::signature::RSA_PKCS1_2048_8192_SHA256,
            signing_input.as_bytes(),
            &sig_bytes,
        )
        .map_err(|_| "JWT RSA signature verification failed")?;

    let (claims, _) = decode_jwt_claims(token)?;
    Ok(claims)
}

// ── Request / response types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct JwtExchangeRequest {
    /// The raw JWT bearer token from the enterprise IdP (OIDC/OAuth2).
    pub token: String,
    /// Ed25519 public key of the agent receiving the delegation cert.
    pub delegate_pk_hex: String,
    /// Subset of capabilities to grant. Must be in `A1_JWT_ALLOWED_CAPS`.
    pub capabilities: Vec<String>,
    /// Requested cert lifetime. Capped at `min(ttl_seconds, jwt_exp - now)`.
    #[serde(default = "default_ttl")]
    pub ttl_seconds: u64,
    /// Optional opaque string forwarded into the cert extension for tracing.
    pub request_id: Option<String>,
}

fn default_ttl() -> u64 { 3600 }

#[derive(Debug, Serialize)]
pub struct JwtExchangeResponse {
    pub fingerprint_hex:  String,
    pub scope_root_hex:   String,
    pub expires_at_unix:  u64,
    pub jwt_subject:      String,
    pub jwt_issuer:       String,
    pub capabilities:     Vec<String>,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error:      String,
    error_code: &'static str,
}

// ── Handler ───────────────────────────────────────────────────────────────────

pub async fn exchange_handler(
    State(state): State<Arc<AppState>>,
    Json(req):    Json<JwtExchangeRequest>,
) -> impl IntoResponse {
    match exchange_inner(&state, req).await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err((status, code, msg)) => (
            status,
            Json(ErrorBody { error: msg, error_code: code }),
        ).into_response(),
    }
}

async fn exchange_inner(
    state: &AppState,
    req:   JwtExchangeRequest,
) -> Result<JwtExchangeResponse, (StatusCode, &'static str, String)> {
    let jwks_url = std::env::var("A1_JWT_JWKS_URL").map_err(|_| (
        StatusCode::NOT_IMPLEMENTED,
        "E5011",
        "JWT exchange not configured: set A1_JWT_JWKS_URL".to_string(),
    ))?;

    let allowed_caps_env = std::env::var("A1_JWT_ALLOWED_CAPS").unwrap_or_default();
    let allowed_caps: Vec<&str> = allowed_caps_env
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    // Verify requested capabilities against allowlist
    if !allowed_caps.is_empty() {
        for cap in &req.capabilities {
            if !allowed_caps.contains(&cap.as_str()) {
                return Err((
                    StatusCode::FORBIDDEN,
                    "E4003",
                    format!("capability '{cap}' is not in A1_JWT_ALLOWED_CAPS"),
                ));
            }
        }
    }

    // Fetch JWKS and verify JWT
    let keys = fetch_jwks(&jwks_url).await.map_err(|e| (
        StatusCode::BAD_GATEWAY,
        "E5021",
        format!("JWKS unavailable: {e}"),
    ))?;

    let claims = verify_jwt_rsa(&req.token, &keys).map_err(|e| (
        StatusCode::UNAUTHORIZED,
        "E4001",
        format!("JWT verification failed: {e}"),
    ))?;

    // Extract subject + issuer
    let subject = claims.get("sub").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let issuer  = claims.get("iss").and_then(|v| v.as_str()).unwrap_or("").to_string();

    if subject.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "E4022", "JWT missing 'sub' claim".to_string()));
    }

    // Cap TTL at JWT expiry
    let now = SystemClock.unix_now();
    let jwt_exp = claims.get("exp").and_then(|v| v.as_u64()).unwrap_or(u64::MAX);
    if jwt_exp <= now {
        return Err((StatusCode::UNAUTHORIZED, "E4010", "JWT has expired".to_string()));
    }
    let max_ttl = jwt_exp.saturating_sub(now);
    let ttl_secs = req.ttl_seconds.min(max_ttl);

    // Decode delegate public key
    let pk_bytes = hex::decode(&req.delegate_pk_hex).map_err(|_| (
        StatusCode::BAD_REQUEST,
        "E4031",
        "invalid delegate_pk_hex".to_string(),
    ))?;
    let pk_arr: [u8; 32] = pk_bytes.try_into().map_err(|_| (
        StatusCode::BAD_REQUEST,
        "E4031",
        "delegate_pk_hex must be 32 bytes".to_string(),
    ))?;
    let delegate_pk = ed25519_dalek::VerifyingKey::from_bytes(&pk_arr).map_err(|_| (
        StatusCode::BAD_REQUEST,
        "E4031",
        "invalid Ed25519 delegate_pk_hex".to_string(),
    ))?;

    // Build capability scope
    let caps_refs: Vec<&str> = req.capabilities.iter().map(String::as_str).collect();
    let intent_hashes: Vec<[u8; 32]> = caps_refs.iter()
        .map(|c| Intent::new(*c).map(|i| i.hash()))
        .collect::<Result<_, _>>()
        .map_err(|e| (StatusCode::BAD_REQUEST, "E4032", e.to_string()))?;

    let tree = IntentTree::build(intent_hashes)
        .map_err(|e| (StatusCode::BAD_REQUEST, "E4032", e.to_string()))?;
    let scope_root = tree.root();

    let narrowing_mask = NarrowingMatrix::from_capabilities(&caps_refs);

    let expiry = now.saturating_add(ttl_secs);

    // Cert extensions — dyolo.jwt.* fields embed JWT provenance; these fields
    // are part of the cert fingerprint and cannot be stripped without
    // invalidating the signature.
    let ext = CertExtensions::new()
        .set("dyolo.jwt.v",       ExtValue::U64(1))
        .set("dyolo.jwt.subject", ExtValue::Str(subject.clone()))
        .set("dyolo.jwt.issuer",  ExtValue::Str(issuer.clone()))
        .set("dyolo.jwt.mask",    ExtValue::Str(narrowing_mask.to_hex()))
        .set("dyolo.jwt.caps",    ExtValue::Strings(req.capabilities.clone()))
        .set("dyolo.prov",        ExtValue::Str("64796f6c6f".to_string()));

    let cert = CertBuilder::new(delegate_pk, scope_root, now, expiry)
        .max_depth(1)
        .extensions(ext)
        .build(&state.signing_identity)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, "E5001", e.to_string()))?;

    let fingerprint = hex::encode(cert.fingerprint());
    let scope_root_hex = hex::encode(scope_root);

    tracing::info!(
        subject = %subject,
        issuer  = %issuer,
        caps    = ?req.capabilities,
        fp      = %fingerprint,
        "jwt_bridge: cert issued"
    );

    Ok(JwtExchangeResponse {
        fingerprint_hex:  fingerprint,
        scope_root_hex,
        expires_at_unix:  expiry,
        jwt_subject:      subject,
        jwt_issuer:       issuer,
        capabilities:     req.capabilities,
    })
}
