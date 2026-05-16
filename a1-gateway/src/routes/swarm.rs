use std::sync::Arc;
use axum::{extract::{Path, State}, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use a1::{
    Clock, DyoloIdentity, SystemClock,
    passport::DyoloPassport,
    swarm::{SwarmPassport, SwarmRole},
};

#[derive(serde::Deserialize)]
pub struct RemoveMemberRequest {
    pub swarm_id: String,
    pub agent_did: String,
}

// In-memory swarm store — production should use Redis/Postgres backend.
use std::sync::Mutex;
static SWARM_STORE: std::sync::OnceLock<Mutex<std::collections::HashMap<String, SwarmPassport>>> =
    std::sync::OnceLock::new();

fn store() -> &'static Mutex<std::collections::HashMap<String, SwarmPassport>> {
    SWARM_STORE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

#[derive(Deserialize)]
pub struct CreateSwarmRequest {
    pub swarm_name: String,
    pub capabilities: Vec<String>,
    #[serde(default = "default_ttl_days")]
    pub ttl_days: u32,
    pub signing_key_hex: String,
}
fn default_ttl_days() -> u32 { 30 }

#[derive(Serialize)]
pub struct CreateSwarmResponse {
    pub swarm_id: String,
    pub swarm_id_hex: String,
    pub swarm_name: String,
}

pub async fn create_handler(Json(req): Json<CreateSwarmRequest>) -> impl IntoResponse {
    match create_swarm_inner(req) {
        Ok(r) => (StatusCode::CREATED, Json(serde_json::to_value(r).unwrap())).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

fn create_swarm_inner(req: CreateSwarmRequest) -> anyhow::Result<CreateSwarmResponse> {
    let sk_bytes = hex::decode(&req.signing_key_hex)?;
    let sk_arr: [u8; 32] = sk_bytes.try_into().map_err(|_| anyhow::anyhow!("signing key must be 32 bytes"))?;
    let identity = DyoloIdentity::from_signing_bytes(&sk_arr);
    let clock = SystemClock;
    let caps = req.capabilities.iter().map(String::as_str).collect::<Vec<_>>().join(",");
    let passport = DyoloPassport::issue_from_csv(
        &req.swarm_name, &caps, req.ttl_days as u64 * 86400, &identity, &clock,
    )?;
    let swarm = SwarmPassport::new(passport, &req.swarm_name);
    let swarm_id = swarm.swarm_id_hex();
    let name = swarm.swarm_name.clone();
    store().lock().unwrap().insert(swarm_id.clone(), swarm);
    Ok(CreateSwarmResponse { swarm_id: swarm_id.clone(), swarm_id_hex: swarm_id, swarm_name: name })
}

#[derive(Deserialize)]
pub struct AddMemberRequest {
    pub swarm_id: String,
    pub agent_pk_hex: String,
    pub role: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default = "default_ttl_secs")]
    pub ttl_seconds: u64,
    pub signing_key_hex: String,
}
fn default_ttl_secs() -> u64 { 3600 }

pub async fn add_member_handler(Json(req): Json<AddMemberRequest>) -> impl IntoResponse {
    match add_member_inner(req) {
        Ok(r) => (StatusCode::CREATED, Json(r)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

fn add_member_inner(req: AddMemberRequest) -> anyhow::Result<serde_json::Value> {
    let sk_bytes = hex::decode(&req.signing_key_hex)?;
    let sk_arr: [u8; 32] = sk_bytes.try_into().map_err(|_| anyhow::anyhow!("signing key must be 32 bytes"))?;
    let orchestrator = DyoloIdentity::from_signing_bytes(&sk_arr);
    let pk_bytes = hex::decode(&req.agent_pk_hex)?;
    let pk_arr: [u8; 32] = pk_bytes.try_into().map_err(|_| anyhow::anyhow!("agent_pk_hex must be 32 bytes"))?;
    let agent_pk = ed25519_dalek::VerifyingKey::from_bytes(&pk_arr)?;
    let role = match req.role.as_str() {
        "orchestrator" => SwarmRole::Orchestrator,
        "auditor" => SwarmRole::Auditor,
        "supervisor" => SwarmRole::Supervisor { capabilities: req.capabilities.clone(), max_worker_ttl_secs: req.ttl_seconds },
        _ => SwarmRole::Worker { capabilities: req.capabilities.clone() },
    };
    let mut store_lock = store().lock().unwrap();
    let swarm = store_lock.get_mut(&req.swarm_id).ok_or_else(|| anyhow::anyhow!("swarm not found"))?;
    let member = swarm.add_member(agent_pk, role, req.ttl_seconds, &orchestrator, &SystemClock)?;
    Ok(serde_json::json!({ "member": member }))
}

pub async fn remove_member_handler(Json(req): Json<RemoveMemberRequest>) -> impl IntoResponse {
    let mut store_lock = store().lock().unwrap();
    match store_lock.get_mut(&req.swarm_id) {
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "swarm not found" }))).into_response(),
        Some(swarm) => match swarm.remove_member(&req.agent_did) {
            None => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "member not found" }))).into_response(),
            Some(_) => (StatusCode::OK, Json(serde_json::json!({ "removed": req.agent_did }))).into_response(),
        },
    }
}

pub async fn list_members_handler(Path(swarm_id): Path<String>) -> impl IntoResponse {
    let store_lock = store().lock().unwrap();
    match store_lock.get(&swarm_id) {
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "swarm not found" }))).into_response(),
        Some(swarm) => {
            let now = SystemClock.unix_now();
            let members: Vec<_> = swarm.active_members(now);
            (StatusCode::OK, Json(serde_json::json!({ "members": members, "count": members.len() }))).into_response()
        }
    }
}