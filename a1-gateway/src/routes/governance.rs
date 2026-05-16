use std::sync::Arc;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use a1::governance::{GovernancePolicy, AuditReport, ApprovalToken};
use crate::state::AppState;

// ─── Tools that A1 always blocks (deny-list) ─────────────────────────────────
const BLOCKED_TOOLS: &[&str] = &[
    "network.raw_socket",
    "network.raw_tcp",
    "process.kill_system",
    "process.kill_all",
    "system.format",
    "shell.exec_root",
    "shell.rm_rf",
    "fs.wipe",
];

// ─── Tools that require explicit human approval ───────────────────────────────
const APPROVAL_TOOLS: &[&str] = &[
    "files.write",
    "files.delete",
    "process.spawn",
    "network.outbound_unrestricted",
];

/// POST /v1/studio/check — lightweight policy check for A1 Studio proof panel.
/// Accepts {agent_id, tool, context} without requiring a signed chain.
/// Returns {authorized, decision, reason} — used only for live UX proof in Studio.
#[derive(serde::Deserialize)]
pub struct StudioCheckRequest {
    pub agent_id: Option<String>,
    pub tool:     String,
    pub context:  Option<serde_json::Value>,
}

#[derive(serde::Serialize)]
pub struct StudioCheckResponse {
    pub authorized: bool,
    pub decision:   String,
    pub reason:     String,
    pub tool:       String,
}

pub async fn studio_check_handler(Json(req): Json<StudioCheckRequest>) -> impl IntoResponse {
    let tool = req.tool.trim().to_lowercase();

    if BLOCKED_TOOLS.iter().any(|&t| t == tool.as_str()) {
        return Json(StudioCheckResponse {
            authorized: false,
            decision:   "block".into(),
            reason:     format!(
                "A1 policy: '{}' is on the capability deny-list. Blocked — the agent never sees this request.",
                tool
            ),
            tool: req.tool,
        }).into_response();
    }

    if APPROVAL_TOOLS.iter().any(|&t| t == tool.as_str()) {
        return Json(StudioCheckResponse {
            authorized: false,
            decision:   "require_approval".into(),
            reason:     format!(
                "A1 policy: '{}' requires a human approval token before the agent may proceed.",
                tool
            ),
            tool: req.tool,
        }).into_response();
    }

    Json(StudioCheckResponse {
        authorized: true,
        decision:   "allow".into(),
        reason:     format!(
            "A1 policy: '{}' is permitted. Gateway authorized — forwarding to agent.",
            tool
        ),
        tool: req.tool,
    }).into_response()
}

pub async fn policy_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match GovernancePolicy::from_env() {
        Ok(Some(policy)) => (StatusCode::OK, Json(serde_json::to_value(policy).unwrap())).into_response(),
        Ok(None) => (StatusCode::OK, Json(serde_json::json!({ "policy": "default", "note": "Set A1_GOVERNANCE_POLICY_FILE to enable" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(serde::Deserialize)]
pub struct AuditReportRequest {
    pub scope: String,
    pub period_start_unix: u64,
    pub period_end_unix: u64,
    pub total_authorizations: Option<u64>,
    pub denied_authorizations: Option<u64>,
    pub revocations_issued: Option<u64>,
}

pub async fn audit_report_handler(Json(req): Json<AuditReportRequest>) -> impl IntoResponse {
    let policy = GovernancePolicy::from_env().unwrap_or_default().unwrap_or_default();
    match AuditReport::new(&req.scope, req.period_start_unix, req.period_end_unix, &policy) {
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
        Ok(mut report) => {
            if let Some(v) = req.total_authorizations { report.total_authorizations = v; }
            if let Some(v) = req.denied_authorizations { report.denied_authorizations = v; }
            if let Some(v) = req.revocations_issued { report.revocations_issued = v; }
            let _ = report.finalize();
            (StatusCode::OK, Json(serde_json::to_value(report).unwrap())).into_response()
        }
    }
}

#[derive(serde::Deserialize)]
pub struct VerifyApprovalRequest {
    pub token: ApprovalToken,
}

pub async fn verify_approval_handler(Json(req): Json<VerifyApprovalRequest>) -> impl IntoResponse {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    match req.token.verify(now) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "valid": true, "capability": req.token.capability, "agent_did": req.token.agent_did }))).into_response(),
        Err(e) => (StatusCode::OK, Json(serde_json::json!({ "valid": false, "error": e.to_string() }))).into_response(),
    }
}