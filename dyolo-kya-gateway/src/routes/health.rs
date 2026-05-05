use std::sync::Arc;

use axum::{Json, extract::State, response::IntoResponse};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
struct HealthResponse {
    status:         &'static str,
    signing_pk_hex: String,
    version:        &'static str,
}

pub async fn handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(HealthResponse {
        status:         "ok",
        signing_pk_hex: state.gateway_pk_hex.clone(),
        version:        env!("CARGO_PKG_VERSION"),
    })
}
