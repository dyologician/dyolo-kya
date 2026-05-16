/// POST /v1/ai/chat   — Proxy a Claude API call through the gateway.
///
/// When `A1_AI_KEY` is set in the gateway environment, the Studio can send
/// AI Integration Assistant messages here instead of directly to Anthropic.
/// Non-technical users get the full agentic loop with zero accounts required.
///
/// GET  /v1/ai/status  — Reports whether the proxy is configured.

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::state::AppState;

// ─── Status ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct AiStatusResponse {
    pub available: bool,
    pub model: String,
    pub note: String,
}

pub async fn status_handler(_state: State<Arc<AppState>>) -> impl IntoResponse {
    let key = std::env::var("A1_AI_KEY").unwrap_or_default();
    let available = key.starts_with("sk-ant-");
    Json(AiStatusResponse {
        available,
        model: if available { "claude-sonnet-4-20250514".into() } else { String::new() },
        note: if available {
            "Gateway AI proxy is active. No user API key required.".into()
        } else {
            "Set A1_AI_KEY in the gateway environment to enable proxy mode.".into()
        },
    })
}

// ─── Chat proxy ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AiChatRequest {
    pub messages: serde_json::Value,
    pub system:   Option<String>,
    pub tools:    Option<serde_json::Value>,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

fn default_max_tokens() -> u32 { 4096 }

pub async fn chat_handler(
    _state: State<Arc<AppState>>,
    Json(req): Json<AiChatRequest>,
) -> impl IntoResponse {
    let key = match std::env::var("A1_AI_KEY") {
        Ok(k) if k.starts_with("sk-ant-") => k,
        _ => return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": {
                    "type": "proxy_not_configured",
                    "message": "A1_AI_KEY is not set on this gateway. Set it to enable AI proxy mode, or enter your own Claude API key in the Studio."
                }
            })),
        ).into_response(),
    };

    let mut body = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": req.max_tokens,
        "messages": req.messages,
    });

    if let Some(sys) = req.system {
        body["system"] = serde_json::Value::String(sys);
    }
    if let Some(tools) = req.tools {
        body["tools"] = tools;
    }

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
    {
        Ok(c)  => c,
        Err(e) => return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": { "message": format!("HTTP client error: {e}") } })),
        ).into_response(),
    };

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await;

    match resp {
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": { "message": format!("Upstream error: {e}") } })),
        ).into_response(),
        Ok(r) => {
            let status = r.status();
            match r.json::<serde_json::Value>().await {
                Ok(j)  => (StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK), Json(j)).into_response(),
                Err(e) => (StatusCode::BAD_GATEWAY, Json(serde_json::json!({ "error": { "message": format!("Parse error: {e}") } }))).into_response(),
            }
        }
    }
}
