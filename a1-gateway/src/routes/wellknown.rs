use std::sync::Arc;

use axum::{extract::State, Json};
use serde::Serialize;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct A1Configuration {
    pub issuer:                       String,
    pub gateway_signing_pk_hex:       String,
    pub gateway_did:                  String,
    pub authorization_endpoint:       String,
    pub batch_authorize_endpoint:     String,
    pub passport_authorize_endpoint:  String,
    pub cert_issuance_endpoint:       String,
    pub cert_revoke_endpoint:         String,
    pub cert_revoke_batch_endpoint:   String,
    pub token_verify_endpoint:        String,
    pub crl_endpoint:                 String,
    pub did_resolve_endpoint:         String,
    pub did_gateway_endpoint:         String,
    pub vc_issue_endpoint:            String,
    pub vc_verify_endpoint:           String,
    pub anchor_endpoint:              String,
    pub negotiate_endpoint:           String,
    pub jwt_exchange_endpoint:        String,
    pub webhook_status_endpoint:      String,
    pub tenant_info_endpoint:         String,
    pub a1_version:                   &'static str,
    pub protocol_enforcer:            &'static str,
    pub supported_algorithms:         &'static [&'static str],
    pub supported_features:           &'static [&'static str],
    pub supported_networks:           &'static [&'static str],
    pub jwt_exchange_enabled:         bool,
    pub webhook_enabled:              bool,
    pub multi_tenant_enabled:         bool,
}

pub async fn handler(State(state): State<Arc<AppState>>) -> Json<A1Configuration> {
    let base = std::env::var("A1_PUBLIC_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());

    let gateway_did = format!("did:a1:{}", state.gateway_pk_hex);

    let jwt_exchange_enabled  = std::env::var("A1_JWT_JWKS_URL").is_ok();
    let webhook_enabled       = state.webhook_url.is_some();
    let multi_tenant_enabled  = std::env::var("A1_MULTI_TENANT")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    Json(A1Configuration {
        issuer:                      base.clone(),
        gateway_signing_pk_hex:      state.gateway_pk_hex.clone(),
        gateway_did,
        authorization_endpoint:      format!("{base}/v1/authorize"),
        batch_authorize_endpoint:    format!("{base}/v1/authorize/batch"),
        passport_authorize_endpoint: format!("{base}/v1/passport/authorize"),
        cert_issuance_endpoint:      format!("{base}/v1/cert/issue"),
        cert_revoke_endpoint:        format!("{base}/v1/cert/revoke"),
        cert_revoke_batch_endpoint:  format!("{base}/v1/cert/revoke-batch"),
        token_verify_endpoint:       format!("{base}/v1/token/verify"),
        crl_endpoint:                format!("{base}/v1/cert/revoke-batch"),
        did_resolve_endpoint:        format!("{base}/v1/did/{{pk_hex}}"),
        did_gateway_endpoint:        format!("{base}/v1/did/gateway"),
        vc_issue_endpoint:           format!("{base}/v1/vc/issue"),
        vc_verify_endpoint:          format!("{base}/v1/vc/verify"),
        anchor_endpoint:             format!("{base}/v1/anchor"),
        negotiate_endpoint:          format!("{base}/v1/negotiate"),
        jwt_exchange_endpoint:       format!("{base}/v1/jwt/exchange"),
        webhook_status_endpoint:     format!("{base}/v1/webhook/status"),
        tenant_info_endpoint:        format!("{base}/v1/tenant/info"),
        a1_version:                  env!("CARGO_PKG_VERSION"),
        protocol_enforcer:           "dyolo_v2.8.0",
        supported_algorithms: &[
            "Ed25519",
            "HybridMlDsa44Ed25519",
            "HybridMlDsa65Ed25519",
        ],
        supported_features: &[
            "delegation",
            "passports",
            "did",
            "vc",
            "zk-commitment",
            "zk-trace-proof",
            "on-chain-anchor",
            "agent-negotiation",
            "post-quantum-ready",
            "mcp-server",
            "jwt-exchange",
            "webhook-siem",
            "multi-tenant",
        ],
        supported_networks: &[
            "ethereum",
            "ethereum-sepolia",
            "polygon",
            "base",
            "arbitrum",
            "solana",
        ],
        jwt_exchange_enabled,
        webhook_enabled,
        multi_tenant_enabled,
    })
}
