use std::sync::Arc;

use axum::{extract::State, response::IntoResponse, Json};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
struct HealthResponse {
    status:                &'static str,
    signing_pk_hex:        String,
    version:               &'static str,
    webhook_enabled:       bool,
    jwt_exchange_enabled:  bool,
    multi_tenant_enabled:  bool,
    storage_backend:       &'static str,
}

pub async fn handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let webhook_enabled      = state.webhook_url.is_some();
    let jwt_exchange_enabled = std::env::var("A1_JWT_JWKS_URL").is_ok();
    let multi_tenant_enabled = std::env::var("A1_MULTI_TENANT")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    let storage_backend = if std::env::var("A1_REDIS_URL").is_ok() {
        "redis"
    } else if std::env::var("A1_PG_URL").is_ok() {
        "postgres"
    } else {
        "memory"
    };

    Json(HealthResponse {
        status: "ok",
        signing_pk_hex: state.gateway_pk_hex.clone(),
        version: env!("CARGO_PKG_VERSION"),
        webhook_enabled,
        jwt_exchange_enabled,
        multi_tenant_enabled,
        storage_backend,
    })
}
