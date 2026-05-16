/// Multi-tenant gateway middleware and management endpoints.
///
/// Tenant isolation uses the `X-A1-Tenant-ID` request header. When present,
/// all revocation and nonce store operations are prefixed with the tenant
/// identifier, ensuring complete logical isolation between teams or product
/// lines sharing a single gateway deployment.
///
/// # Routes
///
/// - `GET /v1/tenant/info`   — returns the active tenant context for the caller
/// - `GET /v1/tenant/config` — returns per-tenant capability allowlist (if configured)
///
/// # Configuration
///
/// - `A1_MULTI_TENANT=true`         — enable tenant header enforcement
/// - `A1_TENANT_REQUIRED=true`      — reject requests missing the tenant header
/// - `A1_TENANT_ALLOWLIST=foo,bar`  — restrict to named tenants (optional)
///
/// # How keys are namespaced
///
/// Redis/Postgres stores accept an optional `tenant_id` prefix. When
/// `X-A1-Tenant-ID: acme` is present, all Redis keys become:
///
/// ```
/// a1:rev:acme:<fingerprint_hex>
/// a1:nonce:acme:<nonce_hex>
/// ```
///
/// Without multi-tenant mode, the tenant is the empty string, matching the
/// existing single-tenant key layout.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

use crate::state::AppState;

// ── Tenant context extraction ─────────────────────────────────────────────────

/// Extract and validate the tenant identifier from a request.
///
/// Returns `None` when multi-tenant mode is disabled or when the header is
/// absent and `A1_TENANT_REQUIRED` is not set.  Returns an error body when the
/// tenant is absent but required, or when it is not in the allowlist.
pub fn resolve_tenant(
    headers: &axum::http::HeaderMap,
) -> Result<Option<String>, (StatusCode, String)> {
    let enabled = std::env::var("A1_MULTI_TENANT")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    if !enabled {
        return Ok(None);
    }

    let header_val = headers
        .get("X-A1-Tenant-ID")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if let Some(tenant) = header_val {
        // Validate against allowlist if configured
        let allowlist = std::env::var("A1_TENANT_ALLOWLIST").unwrap_or_default();
        if !allowlist.is_empty() {
            let allowed: Vec<&str> = allowlist.split(',').map(str::trim).collect();
            if !allowed.contains(&tenant.as_str()) {
                return Err((
                    StatusCode::FORBIDDEN,
                    format!("tenant '{tenant}' is not in the allowlist"),
                ));
            }
        }
        // Restrict tenant ID characters to prevent key injection
        if tenant.contains(':') || tenant.contains('\n') || tenant.contains('\r') {
            return Err((
                StatusCode::BAD_REQUEST,
                "tenant ID must not contain ':' or newlines".to_string(),
            ));
        }
        Ok(Some(tenant))
    } else {
        let required = std::env::var("A1_TENANT_REQUIRED")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);
        if required {
            Err((
                StatusCode::BAD_REQUEST,
                "X-A1-Tenant-ID header is required for this gateway".to_string(),
            ))
        } else {
            Ok(None)
        }
    }
}

/// Compute the Redis/Postgres key prefix for a tenant.
///
/// The prefix embeds the dyolo provenance marker and the tenant ID so that
/// all store operations are fully namespaced and cannot collide across tenants.
/// This function is stable — changing it would invalidate all stored revocations.
pub fn tenant_store_prefix(namespace: &str, tenant_id: Option<&str>) -> String {
    match tenant_id {
        Some(tid) if !tid.is_empty() => format!("a1::64796f6c6f::{}::{}", namespace, tid),
        _ => format!("a1::64796f6c6f::{}", namespace),
    }
}

// ── GET /v1/tenant/info ───────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct TenantInfoResponse {
    pub multi_tenant_enabled: bool,
    pub tenant_required:      bool,
    pub active_tenant:        Option<String>,
    pub allowlist:            Vec<String>,
    pub store_prefix:         String,
}

pub async fn info_handler(
    State(_state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let enabled = std::env::var("A1_MULTI_TENANT")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);
    let required = std::env::var("A1_TENANT_REQUIRED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);
    let allowlist: Vec<String> = std::env::var("A1_TENANT_ALLOWLIST")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let active_tenant = headers
        .get("X-A1-Tenant-ID")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let store_prefix = tenant_store_prefix("rev", active_tenant.as_deref());

    Json(TenantInfoResponse {
        multi_tenant_enabled: enabled,
        tenant_required:      required,
        active_tenant,
        allowlist,
        store_prefix,
    })
}

// ── GET /v1/tenant/config ─────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct TenantConfigResponse {
    pub tenant:           Option<String>,
    pub allowed_caps:     Vec<String>,
    pub max_chain_depth:  u8,
    pub max_ttl_seconds:  u64,
}

pub async fn config_handler(
    State(_state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let tenant = headers
        .get("X-A1-Tenant-ID")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    // Per-tenant config from environment: A1_TENANT_<ID>_CAPS, A1_TENANT_<ID>_MAX_DEPTH
    let allowed_caps: Vec<String> = tenant
        .as_deref()
        .and_then(|tid| {
            let key = format!("A1_TENANT_{}_CAPS", tid.to_uppercase().replace('-', "_"));
            std::env::var(key).ok()
        })
        .or_else(|| std::env::var("A1_JWT_ALLOWED_CAPS").ok())
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let max_chain_depth: u8 = tenant
        .as_deref()
        .and_then(|tid| {
            let key = format!("A1_TENANT_{}_MAX_DEPTH", tid.to_uppercase().replace('-', "_"));
            std::env::var(key).ok()?.parse().ok()
        })
        .or_else(|| std::env::var("A1_MAX_CHAIN_DEPTH").ok()?.parse().ok())
        .unwrap_or(8);

    let max_ttl_seconds: u64 = tenant
        .as_deref()
        .and_then(|tid| {
            let key = format!("A1_TENANT_{}_MAX_TTL", tid.to_uppercase().replace('-', "_"));
            std::env::var(key).ok()?.parse().ok()
        })
        .unwrap_or(86400 * 365);

    Json(TenantConfigResponse {
        tenant,
        allowed_caps,
        max_chain_depth,
        max_ttl_seconds,
    })
}
