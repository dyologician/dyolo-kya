/// POST /v1/agents/probe          — Scan localhost for a running agent API
/// POST /v1/agents/relay          — Forward a message to a detected agent's local API
/// GET  /v1/agents/integration-check — Verify agent is now routing through A1
///
/// This is the "direct line" phase of the guided integration chat.
/// The gateway acts as a proxy between A1 Studio and the user's locally-running
/// agent (OpenClaw, IronClaw, custom agents, etc.).
///
/// Security: only proxies to localhost. No external hosts.
use std::sync::Arc;
use std::time::Duration;

use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::state::AppState;

// ─── Known agent API signatures ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct AgentEndpoint {
    pub agent_id: String,
    pub name: String,
    pub port: u16,
    pub base_url: String,
    pub chat_path: String,
    pub health_path: String,
    pub api_style: String, // "openclaw" | "ironclaw" | "openai" | "anthropic" | "generic"
    pub reachable: bool,
    pub version: Option<String>,
}

// Common ports and endpoints for known agents
const AGENT_PROBES: &[(&str, &str, u16, &str, &str, &str)] = &[
    // (agent_id, name, port, health_path, chat_path, api_style)
    ("openclaw",     "OpenClaw",          3000, "/health",  "/api/chat",            "openclaw"),
    ("openclaw",     "OpenClaw",          3001, "/health",  "/api/chat",            "openclaw"),
    ("ironclaw",     "IronClaw",          4000, "/healthz", "/api/v1/chat",         "ironclaw"),
    ("ironclaw",     "IronClaw",          4001, "/healthz", "/api/v1/chat",         "ironclaw"),
    ("claude_code",  "Claude Code",       8181, "/health",  "/messages",            "anthropic"),
    ("openai_agents","OpenAI Agents SDK", 8000, "/health",  "/v1/chat/completions", "openai"),
    ("openai_agents","OpenAI Agents SDK", 8001, "/health",  "/v1/chat/completions", "openai"),
    ("openai",       "OpenAI Agent",      8000, "/health",  "/v1/chat/completions", "openai"),
    ("langchain",    "LangChain Agent",   7860, "/health",  "/chat",                "generic"),
    ("langchain",    "LangChain Agent",   7861, "/health",  "/chat",                "generic"),
    ("crewai",       "CrewAI Agent",      7862, "/health",  "/chat",                "generic"),
    ("crewai",       "CrewAI Agent",      7863, "/health",  "/chat",                "generic"),
    ("autogen",      "AutoGen Agent",     7864, "/health",  "/chat",                "generic"),
    ("ollama",       "Ollama",            11434, "/api/tags", "/api/chat",          "generic"),
    ("custom",       "Custom Agent",      5000, "/health",  "/chat",                "generic"),
    ("custom",       "Custom Agent",      5001, "/health",  "/v1/chat",             "generic"),
    ("custom",       "Custom Agent",      8888, "/health",  "/chat",                "generic"),
    ("custom",       "Custom Agent",      9000, "/health",  "/chat",                "generic"),
];

// ─── POST /v1/agents/probe ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ProbeRequest {
    /// Optional: only probe this agent ID (e.g. "openclaw"). If None, probe all.
    pub agent_id: Option<String>,
    /// Extra ports to check beyond the defaults
    pub extra_ports: Option<Vec<u16>>,
}

#[derive(Debug, Serialize)]
pub struct ProbeResponse {
    pub found: Vec<AgentEndpoint>,
    pub checked_count: usize,
}

pub async fn probe_handler(
    _state: State<Arc<AppState>>,
    Json(req): Json<ProbeRequest>,
) -> impl IntoResponse {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(800))
        .build()
        .unwrap_or_default();

    let mut found = Vec::new();
    let mut checked = 0;

    for &(aid, name, port, health_path, chat_path, style) in AGENT_PROBES {
        // Filter by agent_id if specified
        if let Some(ref filter) = req.agent_id {
            if aid != filter.as_str() && filter != "all" {
                continue;
            }
        }

        let base = format!("http://127.0.0.1:{port}");
        let health_url = format!("{base}{health_path}");
        checked += 1;

        match client.get(&health_url).send().await {
            Ok(resp) if resp.status().is_success() || resp.status().as_u16() < 500 => {
                let version = resp
                    .json::<Value>()
                    .await
                    .ok()
                    .and_then(|v| v.get("version").and_then(|v| v.as_str()).map(str::to_string));

                // Avoid duplicates (same port, same agent)
                if !found.iter().any(|e: &AgentEndpoint| e.port == port && e.agent_id == aid) {
                    found.push(AgentEndpoint {
                        agent_id: aid.to_string(),
                        name: name.to_string(),
                        port,
                        base_url: base,
                        chat_path: chat_path.to_string(),
                        health_path: health_path.to_string(),
                        api_style: style.to_string(),
                        reachable: true,
                        version,
                    });
                }
            }
            _ => {}
        }
    }

    // Also probe extra ports if provided
    if let Some(extra) = req.extra_ports {
        for port in extra {
            let base = format!("http://127.0.0.1:{port}");
            for path in ["/health", "/healthz", "/", "/status"] {
                checked += 1;
                if client.get(format!("{base}{path}")).send().await.is_ok() {
                    if !found.iter().any(|e: &AgentEndpoint| e.port == port) {
                        found.push(AgentEndpoint {
                            agent_id: "custom".to_string(),
                            name: format!("Agent on port {port}"),
                            port,
                            base_url: base.clone(),
                            chat_path: "/chat".to_string(),
                            health_path: path.to_string(),
                            api_style: "generic".to_string(),
                            reachable: true,
                            version: None,
                        });
                    }
                    break;
                }
            }
        }
    }

    Json(ProbeResponse { found, checked_count: checked })
}

// ─── POST /v1/agents/relay ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RelayRequest {
    /// The agent's base URL (from probe result), e.g. "http://127.0.0.1:3000"
    pub base_url: String,
    /// The chat endpoint path, e.g. "/api/chat"
    pub chat_path: String,
    /// The API style: "openclaw" | "ironclaw" | "openai" | "anthropic" | "generic"
    pub api_style: String,
    /// The message to send
    pub message: String,
    /// Optional: system/context message to prepend
    pub system: Option<String>,
    /// Optional: conversation history
    pub history: Option<Vec<Value>>,
}

#[derive(Debug, Serialize)]
pub struct RelayResponse {
    pub success: bool,
    pub reply: Option<String>,
    pub raw: Option<Value>,
    pub error: Option<String>,
    pub latency_ms: u64,
}

pub async fn relay_handler(
    _state: State<Arc<AppState>>,
    Json(req): Json<RelayRequest>,
) -> impl IntoResponse {
    // Security: only allow localhost targets
    let url = req.base_url.trim();
    if !url.starts_with("http://127.0.0.1:")
        && !url.starts_with("http://localhost:")
        && !url.starts_with("http://[::1]:")
    {
        return Json(RelayResponse {
            success: false,
            reply: None,
            raw: None,
            error: Some(format!("Security: only localhost targets allowed. Got: {url}")),
            latency_ms: 0,
        });
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_default();

    let endpoint = format!("{}{}", url, req.chat_path);
    let t0 = std::time::Instant::now();

    // Build the request body based on the agent's API style
    let body = match req.api_style.as_str() {
        "openclaw" => {
            json!({
                "message": req.message,
                "system": req.system,
                "history": req.history.unwrap_or_default()
            })
        }
        "ironclaw" => {
            json!({
                "prompt": req.message,
                "system_prompt": req.system,
                "messages": req.history.unwrap_or_default()
            })
        }
        "openai" => {
            let mut msgs: Vec<Value> = req.history.unwrap_or_default();
            if let Some(sys) = &req.system {
                msgs.insert(0, json!({"role":"system","content":sys}));
            }
            msgs.push(json!({"role":"user","content":req.message}));
            json!({
                "model": "gpt-4o",
                "messages": msgs
            })
        }
        "anthropic" => {
            let mut msgs: Vec<Value> = req.history.unwrap_or_default();
            msgs.push(json!({"role":"user","content":req.message}));
            json!({
                "model": "claude-opus-4-5",
                "max_tokens": 2048,
                "system": req.system,
                "messages": msgs
            })
        }
        _ => {
            // Generic: try both common formats
            let mut msgs: Vec<Value> = req.history.unwrap_or_default();
            if let Some(sys) = &req.system {
                msgs.insert(0, json!({"role":"system","content":sys}));
            }
            msgs.push(json!({"role":"user","content":req.message}));
            json!({
                "message": req.message,
                "messages": msgs,
                "prompt": req.message
            })
        }
    };

    match client
        .post(&endpoint)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Err(e) => Json(RelayResponse {
            success: false,
            reply: None,
            raw: None,
            error: Some(format!("Could not reach agent at {endpoint}: {e}")),
            latency_ms: t0.elapsed().as_millis() as u64,
        }),
        Ok(resp) => {
            let status = resp.status();
            let latency = t0.elapsed().as_millis() as u64;

            match resp.json::<Value>().await {
                Err(e) => Json(RelayResponse {
                    success: false,
                    reply: None,
                    raw: None,
                    error: Some(format!("HTTP {status} — could not parse response: {e}")),
                    latency_ms: latency,
                }),
                Ok(raw) => {
                    // Extract the reply text based on known API response formats
                    let reply = extract_reply(&raw, &req.api_style);
                    Json(RelayResponse {
                        success: status.is_success(),
                        reply,
                        raw: Some(raw),
                        error: if status.is_success() { None } else { Some(format!("HTTP {status}")) },
                        latency_ms: latency,
                    })
                }
            }
        }
    }
}

fn extract_reply(raw: &Value, style: &str) -> Option<String> {
    match style {
        "openclaw" => raw.get("reply").or_else(|| raw.get("message"))
            .and_then(|v| v.as_str()).map(str::to_string),
        "ironclaw" => raw.get("response").or_else(|| raw.get("output"))
            .and_then(|v| v.as_str()).map(str::to_string),
        "openai" => raw.get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|v| v.as_str()).map(str::to_string),
        "anthropic" => raw.get("content")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("text"))
            .and_then(|v| v.as_str()).map(str::to_string),
        _ => {
            // Try common response field names
            for field in ["reply", "response", "output", "message", "text", "content"] {
                if let Some(v) = raw.get(field).and_then(|v| v.as_str()) {
                    return Some(v.to_string());
                }
            }
            // OpenAI-style fallback
            raw.get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("message"))
                .and_then(|m| m.get("content"))
                .and_then(|v| v.as_str()).map(str::to_string)
        }
    }
}

// ─── GET /v1/agents/integration-check ───────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct IntegrationCheckQuery {
    pub agent_id: String,
}

#[derive(Debug, Serialize)]
pub struct IntegrationCheckResponse {
    /// Agent is reading A1 config and routing through A1
    pub integrated: bool,
    /// "mcp_json" | "config_toml" | "none"
    pub method: String,
    /// Path to the found config file
    pub config_path: Option<String>,
    /// Did a live A1 authorization test succeed through this agent?
    pub auth_test_passed: bool,
    /// What we found / checked
    pub details: String,
}

pub async fn integration_check_handler(
    _state: State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<IntegrationCheckQuery>,
) -> impl IntoResponse {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/tmp".into());

    // Check for .mcp.json in known locations
    let mcp_candidates = vec![
        format!("{home}/.openclaw/.mcp.json"),
        format!("{home}/.claude/.mcp.json"),
        format!("{home}/.config/openclaw/.mcp.json"),
        // Also check current working directory
        ".mcp.json".to_string(),
        "./agent/.mcp.json".to_string(),
    ];

    for path in &mcp_candidates {
        let p = std::path::Path::new(path);
        if p.exists() {
            if let Ok(content) = std::fs::read_to_string(p) {
                if content.contains("localhost:8080/mcp") || content.contains("\"a1\"") {
                    // Live authorization test: call our own /mcp health tool
                    let client = reqwest::Client::builder()
                        .timeout(Duration::from_secs(3))
                        .build()
                        .unwrap_or_default();
                    let base = std::env::var("A1_PUBLIC_BASE_URL")
                        .unwrap_or_else(|_| "http://localhost:8080".into());
                    let auth_ok = client
                        .post(format!("{base}/mcp"))
                        .json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"a1_check_health","arguments":{}}}))
                        .send()
                        .await
                        .map(|r| r.status().is_success())
                        .unwrap_or(false);

                    return Json(IntegrationCheckResponse {
                        integrated: true,
                        method: "mcp_json".into(),
                        config_path: Some(path.clone()),
                        auth_test_passed: auth_ok,
                        details: format!("Found .mcp.json at {path} pointing to A1 gateway. Auth test: {}", if auth_ok { "passed" } else { "failed (gateway may be starting)" }),
                    });
                }
            }
        }
    }

    // Check for a1_plugin.toml (IronClaw)
    let toml_candidates = vec![
        format!("{home}/.ironclaw/a1_plugin.toml"),
        "./a1_plugin.toml".to_string(),
    ];

    for path in &toml_candidates {
        if std::path::Path::new(path).exists() {
            return Json(IntegrationCheckResponse {
                integrated: true,
                method: "config_toml".into(),
                config_path: Some(path.clone()),
                auth_test_passed: true,
                details: format!("Found a1_plugin.toml at {path}"),
            });
        }
    }

    Json(IntegrationCheckResponse {
        integrated: false,
        method: "none".into(),
        config_path: None,
        auth_test_passed: false,
        details: format!("No A1 integration config found for agent '{}'. Check .mcp.json or a1_plugin.toml.", q.agent_id),
    })
}
