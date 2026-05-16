/// GET  /mcp        — SSE stream (MCP protocol, server-sent events)
/// POST /mcp        — JSON-RPC 2.0 request handler
/// GET  /mcp/tools  — Tool manifest (non-MCP convenience)
///
/// Implements the Model Context Protocol (MCP) so that Claude Code and any
/// other MCP-compatible agent can use A1 authorization without any code
/// changes to the agent. Zero decorators. Zero imports. One config file.
///
/// MCP spec reference: https://spec.modelcontextprotocol.io/
use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio_stream::StreamExt;

use crate::state::AppState;

// ─── JSON-RPC 2.0 envelope ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    fn ok(id: Option<Value>, result: Value) -> Self {
        Self { jsonrpc: "2.0", id, result: Some(result), error: None }
    }
    fn err(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError { code, message: message.into(), data: None }),
        }
    }
}

// ─── MCP Tool definitions ────────────────────────────────────────────────────

fn tool_list() -> Value {
    json!({
        "tools": [
            {
                "name": "a1_authorize",
                "description": "Verify that an AI agent is authorized to perform an action. Checks the delegation chain, capability scope, nonce, and revocation status. Returns a ProvableReceipt if authorized.",
                "inputSchema": {
                    "type": "object",
                    "required": ["intent_name", "executor_pk_hex"],
                    "properties": {
                        "intent_name": {
                            "type": "string",
                            "description": "The capability the agent wants to use (e.g. 'trade.equity', 'files.read', 'web.search')"
                        },
                        "executor_pk_hex": {
                            "type": "string",
                            "description": "The executing agent's Ed25519 public key as a hex string"
                        },
                        "chain": {
                            "type": "object",
                            "description": "The SignedChain delegation chain JSON. If omitted and a passport is configured, uses the root passport chain."
                        },
                        "intent_params": {
                            "type": "object",
                            "description": "Optional parameters describing the specific action (e.g. {symbol: 'AAPL', qty: 100})"
                        }
                    }
                }
            },
            {
                "name": "a1_check_health",
                "description": "Check that the A1 gateway is running and return the gateway's identity.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "a1_inspect_passport",
                "description": "Inspect a passport file to see its capabilities, namespace, expiry date, and current status.",
                "inputSchema": {
                    "type": "object",
                    "required": ["passport_path"],
                    "properties": {
                        "passport_path": {
                            "type": "string",
                            "description": "Path to the passport JSON file on disk (e.g. './passport.json')"
                        }
                    }
                }
            },
            {
                "name": "a1_list_capabilities",
                "description": "List all recognized A1 capability names with descriptions. Useful when deciding what capabilities to grant a new agent.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "a1_issue_cert",
                "description": "Issue a delegation certificate to a sub-agent. Requires admin access. The new cert is a subset of the capabilities in the provided chain.",
                "inputSchema": {
                    "type": "object",
                    "required": ["delegate_pk_hex", "capabilities", "ttl_seconds"],
                    "properties": {
                        "delegate_pk_hex": {
                            "type": "string",
                            "description": "The sub-agent's Ed25519 public key as a hex string"
                        },
                        "capabilities": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "List of capability names to grant (must be a subset of the current chain's capabilities)"
                        },
                        "ttl_seconds": {
                            "type": "integer",
                            "description": "How long the cert is valid in seconds (e.g. 3600 = 1 hour)"
                        }
                    }
                }
            },
            {
                "name": "a1_revoke",
                "description": "Revoke a certificate by its fingerprint. After revocation, the cert cannot be used for authorization. Requires admin access.",
                "inputSchema": {
                    "type": "object",
                    "required": ["fingerprint"],
                    "properties": {
                        "fingerprint": {
                            "type": "string",
                            "description": "The hex fingerprint of the certificate to revoke"
                        }
                    }
                }
            }
        ]
    })
}

fn capability_list() -> Value {
    json!([
        {"name":"files.read",       "description":"Read files and documents from disk"},
        {"name":"files.write",      "description":"Write, create, or edit files on disk"},
        {"name":"code.execute",     "description":"Execute code or run scripts"},
        {"name":"web.search",       "description":"Search the internet"},
        {"name":"email.send",       "description":"Send email messages"},
        {"name":"email.read",       "description":"Read email messages"},
        {"name":"database.read",    "description":"Query databases or spreadsheets"},
        {"name":"database.write",   "description":"Write or modify database records"},
        {"name":"trade.equity",     "description":"Execute equity buy/sell orders"},
        {"name":"portfolio.read",   "description":"Read portfolio balances and holdings"},
        {"name":"api.call",         "description":"Call external third-party APIs"},
        {"name":"agent.delegate",   "description":"Delegate tasks to other sub-agents"},
        {"name":"memory.read",      "description":"Read from agent memory store"},
        {"name":"memory.write",     "description":"Write to agent memory store"},
        {"name":"calendar.read",    "description":"Read calendar events"},
        {"name":"calendar.write",   "description":"Create or modify calendar events"},
        {"name":"payments.send",    "description":"Initiate payment transactions"},
        {"name":"compute.run",      "description":"Provision and run compute jobs"}
    ])
}

// ─── POST /mcp — JSON-RPC dispatcher ─────────────────────────────────────────

pub async fn post_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    if req.jsonrpc != "2.0" {
        return Json(JsonRpcResponse::err(req.id, -32600, "Invalid JSON-RPC version — must be \"2.0\""));
    }

    let id = req.id;

    match req.method.as_str() {
        // ── MCP lifecycle ─────────────────────────────────────────────────────
        "initialize" => {
            Json(JsonRpcResponse::ok(id, json!({
                "protocolVersion": "2024-11-05",
                "serverInfo": {
                    "name": "a1-gateway",
                    "version": env!("CARGO_PKG_VERSION"),
                    "description": "A1 — cryptographic chain-of-custody for AI agent delegation"
                },
                "capabilities": {
                    "tools": {}
                }
            })))
        }

        "notifications/initialized" => {
            // No-op acknowledgement
            Json(JsonRpcResponse::ok(id, json!({})))
        }

        // ── Tool listing ──────────────────────────────────────────────────────
        "tools/list" => {
            Json(JsonRpcResponse::ok(id, tool_list()))
        }

        // ── Tool invocation ───────────────────────────────────────────────────
        "tools/call" => {
            let params = req.params.unwrap_or(json!({}));
            let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));

            match tool_name {
                "a1_check_health" => {
                    Json(JsonRpcResponse::ok(id, json!({
                        "content": [{
                            "type": "text",
                            "text": format!(
                                "A1 gateway is running.\nVersion: {}\nGateway public key: {}\nStatus: OK",
                                env!("CARGO_PKG_VERSION"),
                                state.gateway_pk_hex
                            )
                        }]
                    })))
                }

                "a1_list_capabilities" => {
                    let caps = capability_list();
                    let list: String = caps.as_array()
                        .map(|arr| arr.iter().map(|c| {
                            let name = c["name"].as_str().unwrap_or("");
                            let desc = c["description"].as_str().unwrap_or("");
                            format!("  {name:<24} — {desc}")
                        }).collect::<Vec<_>>().join("\n"))
                        .unwrap_or_default();
                    Json(JsonRpcResponse::ok(id, json!({
                        "content": [{"type":"text","text": format!("A1 capability names:\n\n{list}")}]
                    })))
                }

                "a1_authorize" => {
                    let intent_name = args.get("intent_name").and_then(|v| v.as_str()).unwrap_or("");
                    let executor_pk = args.get("executor_pk_hex").and_then(|v| v.as_str()).unwrap_or("");

                    if intent_name.is_empty() || executor_pk.is_empty() {
                        return Json(JsonRpcResponse::err(id, -32602, "intent_name and executor_pk_hex are required"));
                    }

                    // Forward to the real authorize endpoint internally
                    let chain = args.get("chain").cloned().unwrap_or(json!(null));
                    let params_body = args.get("intent_params").cloned().unwrap_or(json!({}));

                    // Build the authorize request body and call the internal handler
                    // In a full implementation this calls authorize::authorize_inner(...)
                    // For now we proxy via HTTP to keep the MCP handler stateless
                    let client = reqwest::Client::new();
                    let base = std::env::var("A1_PUBLIC_BASE_URL")
                        .unwrap_or_else(|_| "http://localhost:8080".into());

                    let body = json!({
                        "chain": chain,
                        "intent_name": intent_name,
                        "executor_pk_hex": executor_pk,
                        "intent_params": params_body
                    });

                    match client.post(format!("{base}/v1/authorize"))
                        .header("Content-Type", "application/json")
                        .json(&body)
                        .send()
                        .await
                    {
                        Ok(resp) => {
                            let status = resp.status();
                            let json_body: Value = resp.json().await.unwrap_or(json!({}));
                            if status.is_success() {
                                let receipt_text = serde_json::to_string_pretty(&json_body)
                                    .unwrap_or_else(|_| format!("{json_body}"));
                                Json(JsonRpcResponse::ok(id, json!({
                                    "content": [{"type":"text","text": format!("✅ Authorized\n\n{receipt_text}")}]
                                })))
                            } else {
                                let msg = json_body.get("error").and_then(|e| e.as_str())
                                    .unwrap_or("Authorization denied");
                                Json(JsonRpcResponse::ok(id, json!({
                                    "content": [{"type":"text","text": format!("❌ Authorization denied: {msg}")}],
                                    "isError": true
                                })))
                            }
                        }
                        Err(e) => Json(JsonRpcResponse::err(id, -32000, format!("Gateway request failed: {e}"))),
                    }
                }

                "a1_inspect_passport" => {
                    let path = args.get("passport_path").and_then(|v| v.as_str()).unwrap_or("passport.json");
                    match std::fs::read_to_string(path) {
                        Ok(content) => {
                            match serde_json::from_str::<Value>(&content) {
                                Ok(passport) => {
                                    let ns = passport.get("namespace").and_then(|v| v.as_str()).unwrap_or("unknown");
                                    let caps = passport.get("capabilities")
                                        .map(|c| serde_json::to_string(c).unwrap_or_default())
                                        .unwrap_or_else(|| "none".to_string());
                                    let exp = passport.get("expiration_unix")
                                        .and_then(|v| v.as_i64())
                                        .map(|ts| {
                                            let now = std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap_or_default()
                                                .as_secs() as i64;
                                            if ts > now { "VALID" } else { "EXPIRED" }
                                        })
                                        .unwrap_or("unknown");
                                    Json(JsonRpcResponse::ok(id, json!({
                                        "content": [{
                                            "type": "text",
                                            "text": format!(
                                                "Passport: {path}\n  Namespace:    {ns}\n  Capabilities: {caps}\n  Status:       {exp}"
                                            )
                                        }]
                                    })))
                                }
                                Err(e) => Json(JsonRpcResponse::err(id, -32000, format!("Invalid passport JSON: {e}"))),
                            }
                        }
                        Err(e) => Json(JsonRpcResponse::err(id, -32000, format!("Cannot read passport file '{path}': {e}"))),
                    }
                }

                "a1_issue_cert" | "a1_revoke" => {
                    // Admin operations — require admin secret header
                    // Forward to the gateway's REST endpoints
                    let client = reqwest::Client::new();
                    let base = std::env::var("A1_PUBLIC_BASE_URL")
                        .unwrap_or_else(|_| "http://localhost:8080".into());
                    let admin_secret = std::env::var("A1_ADMIN_SECRET").ok();

                    let (path, body) = if tool_name == "a1_issue_cert" {
                        ("/v1/cert/issue", json!({
                            "delegate_pk_hex": args.get("delegate_pk_hex"),
                            "capabilities": args.get("capabilities"),
                            "ttl_seconds": args.get("ttl_seconds")
                        }))
                    } else {
                        ("/v1/cert/revoke", json!({ "fingerprint": args.get("fingerprint") }))
                    };

                    let mut req = client.post(format!("{base}{path}"))
                        .header("Content-Type", "application/json")
                        .json(&body);

                    if let Some(secret) = admin_secret {
                        req = req.header("Authorization", format!("Bearer {secret}"));
                    }

                    match req.send().await {
                        Ok(resp) => {
                            let ok = resp.status().is_success();
                            let val: Value = resp.json().await.unwrap_or(json!({}));
                            let text = serde_json::to_string_pretty(&val).unwrap_or_default();
                            Json(JsonRpcResponse::ok(id, json!({
                                "content": [{"type":"text","text": if ok { format!("✅ Done\n\n{text}") } else { format!("❌ Failed\n\n{text}") }}],
                                "isError": !ok
                            })))
                        }
                        Err(e) => Json(JsonRpcResponse::err(id, -32000, e.to_string())),
                    }
                }

                unknown => Json(JsonRpcResponse::err(
                    id, -32601,
                    format!("Unknown tool: '{unknown}'. Call tools/list to see available tools."),
                )),
            }
        }

        unknown => Json(JsonRpcResponse::err(
            id, -32601,
            format!("Method not found: '{unknown}'"),
        )),
    }
}

// ─── GET /mcp — SSE endpoint ──────────────────────────────────────────────────

pub async fn sse_handler(
    State(_state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
    // Emit the tool manifest immediately on connect, then keep alive
    let tools = tool_list();
    let tools_str = serde_json::to_string(&tools).unwrap_or_default();

    let stream = tokio_stream::iter(vec![
        Ok(Event::default()
            .event("tools")
            .data(tools_str)),
    ])
    .chain(tokio_stream::pending());

    Sse::new(stream).keep_alive(KeepAlive::default())
}

// ─── GET /mcp/tools — plain JSON manifest (non-MCP clients) ──────────────────

pub async fn tools_handler() -> Json<Value> {
    Json(tool_list())
}
