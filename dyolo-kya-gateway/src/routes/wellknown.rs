use std::sync::Arc;

use axum::{Json, extract::State};
use serde::Serialize;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct KyaConfiguration {
    pub issuer:                  String,
    pub gateway_signing_pk_hex:  String,
    pub authorization_endpoint:  String,
    pub batch_authorize_endpoint: String,
    pub cert_issuance_endpoint:  String,
    pub cert_revoke_endpoint:    String,
    pub cert_revoke_batch_endpoint: String,
    pub token_verify_endpoint:   String,
    pub crl_endpoint:            String,
    pub kya_version:             &'static str,
    pub supported_algorithms:    &'static [&'static str],
}

pub async fn handler(State(state): State<Arc<AppState>>) -> Json<KyaConfiguration> {
    let base = std::env::var("DYOLO_PUBLIC_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    Json(KyaConfiguration {
        issuer:                    base.clone(),
        gateway_signing_pk_hex:    state.gateway_pk_hex.clone(),
        authorization_endpoint:    format!("{base}/v1/authorize"),
        batch_authorize_endpoint:  format!("{base}/v1/authorize/batch"),
        cert_issuance_endpoint:    format!("{base}/v1/cert/issue"),
        cert_revoke_endpoint:      format!("{base}/v1/cert/revoke"),
        cert_revoke_batch_endpoint: format!("{base}/v1/cert/revoke-batch"),
        token_verify_endpoint:     format!("{base}/v1/token/verify"),
        crl_endpoint:              format!("{base}/v1/cert/revoke-batch"),
        kya_version:               "2.0.0",
        supported_algorithms:      &["Ed25519"],
    })
}
