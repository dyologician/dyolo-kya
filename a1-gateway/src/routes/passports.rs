/// POST /v1/passports/issue       — Issue a new DyoloPassport from the Studio UI
/// POST /v1/passports/renew       — Re-issue a passport at the same path with new TTL
/// GET  /v1/passports/list        — List passport JSON files under ~/.a1/passports/
/// POST /v1/system/autostart      — Install gateway as a background service (launchd/systemd)
/// DELETE /v1/system/autostart    — Remove the autostart service
/// POST /v1/debug/explain-error   — Translate a raw A1 error code into plain English

use std::sync::Arc;

use axum::{extract::{Query, State}, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use a1::{DyoloIdentity, DyoloPassport, SystemClock};

use crate::state::AppState;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn passports_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let dir = home.join(".a1").join("passports");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn passport_path(namespace: &str) -> std::path::PathBuf {
    let safe = namespace
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>();
    passports_dir().join(format!("{safe}.json"))
}

fn ttl_str_to_secs(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(d) = s.strip_suffix('d') { return d.parse::<u64>().ok().map(|n| n * 86400); }
    if let Some(h) = s.strip_suffix('h') { return h.parse::<u64>().ok().map(|n| n * 3600); }
    if let Some(m) = s.strip_suffix('m') { return m.parse::<u64>().ok().map(|n| n * 60); }
    if let Some(y) = s.strip_suffix('y') { return y.parse::<u64>().ok().map(|n| n * 31536000); }
    s.parse::<u64>().ok()
}

// ─── POST /v1/passports/issue ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct IssuePassportRequest {
    /// Human-readable agent name, used as namespace
    pub namespace: String,
    /// Capability names: ["files.read", "web.search"]
    pub capabilities: Vec<String>,
    /// TTL as a string: "30d", "7d", "1y", or raw seconds "86400"
    pub ttl: String,
    /// Where to save the passport JSON. Defaults to ~/.a1/passports/<namespace>.json
    pub output_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct IssuePassportResponse {
    pub success: bool,
    pub namespace: String,
    pub path: String,
    pub public_key_hex: String,
    pub capabilities: Vec<String>,
    pub ttl_seconds: u64,
    pub expires_at: String,
    pub error: Option<String>,
}

pub async fn issue_passport_handler(
    _state: State<Arc<AppState>>,
    Json(req): Json<IssuePassportRequest>,
) -> impl IntoResponse {
    // Validate namespace
    let ns = req.namespace.trim().to_string();
    if ns.is_empty() {
        return Json(IssuePassportResponse {
            success: false,
            namespace: ns,
            path: String::new(),
            public_key_hex: String::new(),
            capabilities: vec![],
            ttl_seconds: 0,
            expires_at: String::new(),
            error: Some("Namespace cannot be empty".into()),
        });
    }

    // Validate capabilities
    if req.capabilities.is_empty() {
        return Json(IssuePassportResponse {
            success: false,
            namespace: ns,
            path: String::new(),
            public_key_hex: String::new(),
            capabilities: vec![],
            ttl_seconds: 0,
            expires_at: String::new(),
            error: Some("At least one capability is required".into()),
        });
    }

    // Parse TTL
    let ttl_secs = match ttl_str_to_secs(&req.ttl) {
        Some(s) if s > 0 => s,
        _ => return Json(IssuePassportResponse {
            success: false,
            namespace: ns,
            path: String::new(),
            public_key_hex: String::new(),
            capabilities: vec![],
            ttl_seconds: 0,
            expires_at: String::new(),
            error: Some(format!("Invalid TTL '{}'. Use '30d', '7d', '1y', '3600' etc.", req.ttl)),
        }),
    };

    // Generate a fresh Ed25519 keypair for this passport
    let identity = DyoloIdentity::generate();
    let pk_hex = hex::encode(identity.verifying_key().to_bytes());

    // Collect capabilities as &str slices
    let caps_refs: Vec<&str> = req.capabilities.iter().map(String::as_str).collect();

    // Issue passport
    match DyoloPassport::issue(&ns, &caps_refs, ttl_secs, &identity, &SystemClock) {
        Err(e) => Json(IssuePassportResponse {
            success: false,
            namespace: ns,
            path: String::new(),
            public_key_hex: pk_hex,
            capabilities: req.capabilities,
            ttl_seconds: ttl_secs,
            expires_at: String::new(),
            error: Some(format!("Passport issuance failed: {e}")),
        }),
        Ok(passport) => {
            // Determine output path
            let out_path = req.output_path
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| passport_path(&ns));

            // Ensure directory exists
            if let Some(parent) = out_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            // Compute expiry for display
            let expires_at = {
                let unix = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    + ttl_secs;
                // Simple ISO-8601 approximation
                format!("Unix timestamp: {unix}  (~{} days from now)", ttl_secs / 86400)
            };

            // Save passport
            match passport.save(&out_path) {
                Ok(()) => Json(IssuePassportResponse {
                    success: true,
                    namespace: ns,
                    path: out_path.display().to_string(),
                    public_key_hex: pk_hex,
                    capabilities: req.capabilities,
                    ttl_seconds: ttl_secs,
                    expires_at,
                    error: None,
                }),
                Err(e) => Json(IssuePassportResponse {
                    success: false,
                    namespace: ns,
                    path: out_path.display().to_string(),
                    public_key_hex: pk_hex,
                    capabilities: req.capabilities,
                    ttl_seconds: ttl_secs,
                    expires_at,
                    error: Some(format!("Passport issued but could not save to disk: {e}")),
                }),
            }
        }
    }
}

// ─── GET /v1/passports/list ───────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct PassportEntry {
    pub filename: String,
    pub path: String,
    pub namespace: Option<String>,
    pub capabilities: Vec<String>,
    pub expiration_unix: Option<i64>,
    pub status: String,
    pub fingerprint_hex: Option<String>,
    pub days_remaining: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ListPassportsResponse {
    pub passports: Vec<PassportEntry>,
    pub directory: String,
}

pub async fn list_passports_handler(state: State<Arc<AppState>>) -> impl IntoResponse {
    let dir = passports_dir();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // Collect directory entries first (sync), then process async
    let entries: Vec<_> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let filename = path.file_name()?.to_string_lossy().into_owned();
            if !filename.ends_with(".json") || filename.ends_with(".bak") {
                return None;
            }
            Some((path, filename))
        })
        .collect();

    let mut passports: Vec<PassportEntry> = Vec::new();

    for (path, filename) in entries {
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let v: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue,
        };

            let namespace       = v.get("namespace").and_then(|n| n.as_str()).map(str::to_string);
            let expiration_unix = v.get("expiration_unix").and_then(|e| e.as_i64());
            let capabilities: Vec<String> = v.get("capabilities")
                .and_then(|c| c.as_array())
                .map(|arr| arr.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
                .unwrap_or_default();

            let days_remaining = expiration_unix.map(|exp| (exp - now) / 86400);

            // Load passport to get fingerprint — used for both display and revocation check
            let loaded_passport = DyoloPassport::load(&path).ok();
            let fingerprint_hex = loaded_passport.as_ref().map(|p| p.cert.fingerprint_hex());

            // Async revocation check — skip (and delete) any revoked passport files
            let is_revoked = if let Some(ref lp) = loaded_passport {
                let fp = lp.cert.fingerprint();
                state.revocation.is_revoked(&fp).await.unwrap_or(false)
            } else {
                false
            };

            if is_revoked {
                // Remove the file so it disappears even after an in-memory store reset
                let _ = std::fs::remove_file(&path);
                continue; // exclude from list
            }

            let status = match expiration_unix {
                Some(exp) if exp > now => "valid",
                Some(_) => "expired",
                None => "valid",
            }.to_string();

        passports.push(PassportEntry {
            filename,
            path: path.display().to_string(),
            namespace,
            capabilities,
            expiration_unix,
            status,
            fingerprint_hex,
            days_remaining,
        });
    }

    Json(ListPassportsResponse {
        passports,
        directory: dir.display().to_string(),
    })
}

// ─── POST /v1/passports/revoke-by-namespace ───────────────────────────────────
//
// Revokes the root passport cert for the given namespace by loading its file,
// extracting the fingerprint, and writing it to the revocation store.
// This is the path that does not require the caller to know the fingerprint.

#[derive(Debug, Deserialize)]
pub struct RevokeByNamespaceRequest {
    pub namespace: String,
    pub passport_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RevokeByNamespaceResponse {
    pub success: bool,
    pub namespace: String,
    pub fingerprint_hex: Option<String>,
    pub error: Option<String>,
}

pub async fn revoke_by_namespace_handler(
    state: State<Arc<AppState>>,
    Json(req): Json<RevokeByNamespaceRequest>,
) -> impl IntoResponse {
    let ns = req.namespace.trim().to_string();
    if ns.is_empty() {
        return Json(RevokeByNamespaceResponse {
            success: false, namespace: ns, fingerprint_hex: None,
            error: Some("Namespace cannot be empty".into()),
        });
    }

    let path = req.passport_path
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| passport_path(&ns));

    let passport = match DyoloPassport::load(&path) {
        Ok(p)  => p,
        Err(e) => return Json(RevokeByNamespaceResponse {
            success: false, namespace: ns, fingerprint_hex: None,
            error: Some(format!("Could not load passport at '{}': {e}", path.display())),
        }),
    };

    let fp = passport.cert.fingerprint();
    let fp_hex = hex::encode(fp);

    if let Err(e) = state.revocation.revoke(&fp).await {
        return Json(RevokeByNamespaceResponse {
            success: false, namespace: ns,
            fingerprint_hex: Some(fp_hex),
            error: Some(format!("Revocation store error: {e}")),
        });
    }

    // Remove the passport file from disk so it no longer appears in the list.
    // If deletion fails (e.g. custom path not in passports dir) we still report
    // success — the cryptographic revocation already took effect.
    let _ = std::fs::remove_file(&path);

    Json(RevokeByNamespaceResponse {
        success: true, namespace: ns,
        fingerprint_hex: Some(fp_hex),
        error: None,
    })
}

// ─── POST /v1/passports/renew ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RenewPassportRequest {
    pub path: String,
    pub ttl: String,
}

pub async fn renew_passport_handler(
    state: State<Arc<AppState>>,
    Json(req): Json<RenewPassportRequest>,
) -> impl IntoResponse {
    // Read existing passport to get namespace and capabilities
    let path = std::path::PathBuf::from(&req.path);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({
            "success": false,
            "error": format!("Cannot read passport at '{}': {e}", req.path)
        }))).into_response(),
    };

    let v: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "success": false,
            "error": format!("Invalid passport JSON: {e}")
        }))).into_response(),
    };

    let namespace = v.get("namespace")
        .and_then(|n| n.as_str())
        .unwrap_or("unknown")
        .to_string();

    let capabilities: Vec<String> = v.get("capabilities")
        .and_then(|c| c.as_array())
        .map(|arr| arr.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
        .unwrap_or_default();

    // Re-issue with same namespace + capabilities, new TTL
    let issue_req = IssuePassportRequest {
        namespace,
        capabilities,
        ttl: req.ttl,
        output_path: Some(req.path.clone()),
    };

    issue_passport_handler(state, Json(issue_req)).await.into_response()
}

// ─── POST /v1/system/autostart ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct AutostartResponse {
    pub success: bool,
    pub method: String,
    pub path: Option<String>,
    pub message: String,
    pub error: Option<String>,
}

pub async fn install_autostart_handler(_state: State<Arc<AppState>>) -> impl IntoResponse {
    let gw_bin = std::env::var("HOME")
        .map(|h| format!("{h}/.a1/bin/a1-gateway"))
        .unwrap_or_else(|_| "~/.a1/bin/a1-gateway".into());

    let log_file = std::env::var("HOME")
        .map(|h| format!("{h}/.a1/logs/gateway.log"))
        .unwrap_or_else(|_| "~/.a1/logs/gateway.log".into());

    #[cfg(target_os = "macos")]
    {
        // launchd plist
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let agents_dir = format!("{home}/Library/LaunchAgents");
        let plist_path = format!("{agents_dir}/com.dyolo.a1-gateway.plist");
        let _ = std::fs::create_dir_all(&agents_dir);

        let plist = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.dyolo.a1-gateway</string>
    <key>ProgramArguments</key>
    <array>
        <string>{gw_bin}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{log_file}</string>
    <key>StandardErrorPath</key>
    <string>{log_file}</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>RUST_LOG</key>
        <string>a1_gateway=info</string>
    </dict>
</dict>
</plist>"#);

        match std::fs::write(&plist_path, &plist) {
            Ok(()) => {
                // Load the service immediately
                let _ = std::process::Command::new("launchctl")
                    .args(["load", "-w", &plist_path])
                    .output();
                return Json(AutostartResponse {
                    success: true,
                    method: "launchd".into(),
                    path: Some(plist_path),
                    message: "A1 will now start automatically every time you log in to your Mac.".into(),
                    error: None,
                }).into_response();
            }
            Err(e) => return Json(AutostartResponse {
                success: false,
                method: "launchd".into(),
                path: Some(plist_path),
                message: String::new(),
                error: Some(format!("Could not write plist: {e}")),
            }).into_response(),
        }
    }

    #[cfg(target_os = "linux")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let service_dir = format!("{home}/.config/systemd/user");
        let service_path = format!("{service_dir}/a1-gateway.service");
        let _ = std::fs::create_dir_all(&service_dir);

        let unit = format!(r#"[Unit]
Description=A1 Gateway — Know Your Agent
After=network.target

[Service]
ExecStart={gw_bin}
Restart=always
RestartSec=5
StandardOutput=append:{log_file}
StandardError=append:{log_file}
Environment=RUST_LOG=a1_gateway=info

[Install]
WantedBy=default.target
"#);

        match std::fs::write(&service_path, &unit) {
            Ok(()) => {
                let _ = std::process::Command::new("systemctl")
                    .args(["--user", "daemon-reload"])
                    .output();
                let _ = std::process::Command::new("systemctl")
                    .args(["--user", "enable", "--now", "a1-gateway"])
                    .output();
                return Json(AutostartResponse {
                    success: true,
                    method: "systemd --user".into(),
                    path: Some(service_path),
                    message: "A1 will now start automatically every time you log in.".into(),
                    error: None,
                }).into_response();
            }
            Err(e) => return Json(AutostartResponse {
                success: false,
                method: "systemd".into(),
                path: Some(service_path),
                message: String::new(),
                error: Some(format!("Could not write service file: {e}")),
            }).into_response(),
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows Task Scheduler
        let gw_bin_win = gw_bin.replace('/', "\\");
        let result = std::process::Command::new("schtasks")
            .args([
                "/create", "/tn", "A1Gateway", "/tr", &gw_bin_win,
                "/sc", "ONLOGON", "/rl", "LIMITED", "/f",
            ])
            .output();
        match result {
            Ok(o) if o.status.success() => return Json(AutostartResponse {
                success: true,
                method: "Task Scheduler".into(),
                path: None,
                message: "A1 will now start automatically when you log in to Windows.".into(),
                error: None,
            }).into_response(),
            Ok(o) => return Json(AutostartResponse {
                success: false,
                method: "Task Scheduler".into(),
                path: None,
                message: String::new(),
                error: Some(String::from_utf8_lossy(&o.stderr).into_owned()),
            }).into_response(),
            Err(e) => return Json(AutostartResponse {
                success: false,
                method: "Task Scheduler".into(),
                path: None,
                message: String::new(),
                error: Some(e.to_string()),
            }).into_response(),
        }
    }

    #[allow(unreachable_code)]
    Json(AutostartResponse {
        success: false,
        method: "unknown".into(),
        path: None,
        message: String::new(),
        error: Some("Autostart is not supported on this platform. Add 'a1 start' to your shell profile manually.".into()),
    }).into_response()
}

// ─── DELETE /v1/system/autostart ─────────────────────────────────────────────

pub async fn remove_autostart_handler(_state: State<Arc<AppState>>) -> impl IntoResponse {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());

    // Try all platforms
    let plist = format!("{home}/Library/LaunchAgents/com.dyolo.a1-gateway.plist");
    let service = format!("{home}/.config/systemd/user/a1-gateway.service");

    let mut removed = vec![];

    if std::path::Path::new(&plist).exists() {
        let _ = std::process::Command::new("launchctl").args(["unload", &plist]).output();
        if std::fs::remove_file(&plist).is_ok() { removed.push(plist); }
    }
    if std::path::Path::new(&service).exists() {
        let _ = std::process::Command::new("systemctl").args(["--user","disable","a1-gateway"]).output();
        if std::fs::remove_file(&service).is_ok() { removed.push(service); }
    }

    let _ = std::process::Command::new("schtasks").args(["/delete","/tn","A1Gateway","/f"]).output();

    if removed.is_empty() {
        Json(AutostartResponse {
            success: true,
            method: "none found".into(),
            path: None,
            message: "No autostart entries were found (A1 was not set to start on login).".into(),
            error: None,
        })
    } else {
        Json(AutostartResponse {
            success: true,
            method: "removed".into(),
            path: Some(removed.join(", ")),
            message: "Autostart removed. A1 will no longer start automatically.".into(),
            error: None,
        })
    }
}

// ─── POST /v1/debug/explain-error ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ExplainErrorRequest {
    pub error: String,
    pub error_code: Option<String>,
    pub context: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ExplainErrorResponse {
    pub plain_english: String,
    pub likely_cause: String,
    pub fix: String,
    pub fix_steps: Vec<String>,
    pub fix_type: String,
    pub severity: String,
}

pub async fn explain_error_handler(
    _state: State<Arc<AppState>>,
    Json(req): Json<ExplainErrorRequest>,
) -> impl IntoResponse {
    let err  = req.error.to_lowercase();
    let code = req.error_code.as_deref().unwrap_or("").to_uppercase();

    struct Entry {
        plain:     &'static str,
        cause:     &'static str,
        fix:       &'static str,
        steps:     &'static [&'static str],
        fix_type:  &'static str,
        severity:  &'static str,
    }

    let e: Entry = if err.contains("capability not granted") || err.contains("capabilitynotgranted") || code == "E4003" {
        Entry {
            plain:    "Your agent tried to do something it's not allowed to do.",
            cause:    "The capability is not listed in the agent's passport. The passport was issued without this permission.",
            fix:      "Go to 'Protect My Agent', add the missing capability, and issue a new passport.",
            steps:    &[
                "Open the 'Protect My Agent' tab from the sidebar.",
                "Add the missing capability (e.g. 'trade.equity') to the capabilities list.",
                "Issue a new passport — the wizard saves a new passport.json.",
                "Restart your agent so it picks up the updated passport.",
            ],
            fix_type: "capability",
            severity: "medium",
        }
    } else if err.contains("expired") || err.contains("certificate expired") || code == "E4010" {
        Entry {
            plain:    "Your agent's authorization has expired.",
            cause:    "The passport or delegation certificate reached its expiry date (TTL elapsed).",
            fix:      "Open the Passport Vault and click Renew next to the expired agent.",
            steps:    &[
                "Open the 'Passport Vault' tab from the sidebar.",
                "Find the expired passport (shown with a red badge).",
                "Select a new expiry duration from the dropdown, then click Renew.",
                "Restart your agent — it will pick up the renewed passport automatically.",
            ],
            fix_type: "expired",
            severity: "high",
        }
    } else if err.contains("revoked") || code == "E4012" {
        Entry {
            plain:    "This agent's access was revoked.",
            cause:    "The certificate was explicitly revoked, or the passport file was manually removed.",
            fix:      "Issue a new passport via 'Protect My Agent'.",
            steps:    &[
                "Open 'Protect My Agent' and create a new passport with the same agent name.",
                "Update the passport path in your agent configuration.",
                "Restart your agent.",
                "If you didn't revoke this yourself, review who has gateway admin access.",
            ],
            fix_type: "revoked",
            severity: "high",
        }
    } else if err.contains("narrowing") || err.contains("narrowing violation") {
        Entry {
            plain:    "An agent tried to grant a sub-agent more permissions than it has.",
            cause:    "Delegation narrowing enforcement: a parent agent cannot delegate capabilities it was not itself granted.",
            fix:      "Reduce the sub-agent's requested capabilities to a strict subset of the parent's.",
            steps:    &[
                "Check what capabilities the parent agent's passport contains.",
                "Open 'Protect My Agent' and re-issue the sub-agent's passport.",
                "Ensure the sub-agent's capabilities are a subset of the parent's list.",
                "Restart both agents after updating their passports.",
            ],
            fix_type: "narrowing",
            severity: "medium",
        }
    } else if err.contains("replay") || err.contains("nonce") || code == "E4011" {
        Entry {
            plain:    "A1 blocked a duplicate request.",
            cause:    "The same authorization nonce was seen twice. This prevents replay attacks.",
            fix:      "Ensure your agent generates a fresh nonce for every request.",
            steps:    &[
                "This usually resolves itself — the A1 SDK generates fresh nonces automatically.",
                "If it keeps happening, check your agent code for request caching or retries that reuse the same payload.",
                "Ensure you are not manually constructing nonces or reusing authorization tokens.",
            ],
            fix_type: "nonce",
            severity: "low",
        }
    } else if err.contains("signature") || err.contains("invalid signature") || code == "E4001" {
        Entry {
            plain:    "The agent's cryptographic identity could not be verified.",
            cause:    "The delegation certificate signature is invalid — usually a corrupted or wrong passport file.",
            fix:      "Re-issue the passport via 'Protect My Agent' and make sure your agent loads the new file.",
            steps:    &[
                "Open 'Protect My Agent' and issue a fresh passport for this agent.",
                "Make sure the passport.json path in your agent config matches the newly issued file.",
                "Delete any old passport.json files that might be getting loaded by mistake.",
                "Restart your agent.",
            ],
            fix_type: "signature",
            severity: "high",
        }
    } else if err.contains("missing chain") || err.contains("missing signed chain") || code == "E4002" {
        Entry {
            plain:    "A1 received a request with no delegation chain attached.",
            cause:    "The agent did not include the signed_chain in its authorization request. The A1 integration code may be incomplete.",
            fix:      "Run the AI Integration Assistant to re-patch your agent's tool files.",
            steps:    &[
                "Open the 'AI Integration' tab from the sidebar.",
                "Select your framework and let the assistant patch your tool files.",
                "The @a1_guard / withDyoloPassport decorators inject the chain automatically.",
                "Restart your agent and retry the action.",
            ],
            fix_type: "chain",
            severity: "medium",
        }
    } else if err.contains("namespace") || code == "E4005" {
        Entry {
            plain:    "The agent used a passport issued for a different namespace.",
            cause:    "The passport namespace in the file doesn't match what the gateway expects. Usually a wrong file path.",
            fix:      "Check which passport.json your agent is loading and ensure it matches this agent's namespace.",
            steps:    &[
                "Open the 'Passport Vault' and check the namespace on each passport.",
                "In your agent config, verify the passport_path points to the correct file.",
                "If needed, re-issue a fresh passport for this specific agent namespace.",
            ],
            fix_type: "namespace",
            severity: "medium",
        }
    } else if err.contains("gateway") || err.contains("connection refused") || err.contains("econnrefused") {
        Entry {
            plain:    "A1 is not running.",
            cause:    "The A1 gateway process stopped — after a reboot, terminal close, or crash.",
            fix:      "Go to the 'Start / Stop' tab and start A1.",
            steps:    &[
                "Open the 'Start / Stop' tab from the sidebar.",
                "Click 'Start A1' and run the command shown in your terminal.",
                "Enable 'Auto-start on login' so this doesn't happen again after reboots.",
                "Once the green dot appears, retry your agent's action.",
            ],
            fix_type: "gateway",
            severity: "high",
        }
    } else if err.contains("rate limit") || code == "E4029" {
        Entry {
            plain:    "Your agent is sending requests too fast.",
            cause:    "The gateway's per-IP rate limit was exceeded. Default is 500 req/s.",
            fix:      "Reduce request frequency in your agent code, or increase A1_RATE_LIMIT_RPS.",
            steps:    &[
                "Add throttling or backoff logic to your agent's tool calls.",
                "For high-volume deployments, set A1_RATE_LIMIT_RPS=2000 in gateway environment variables.",
                "Batch authorization calls where possible using POST /v1/authorize/batch.",
            ],
            fix_type: "rate",
            severity: "low",
        }
    } else if err.contains("depth") || err.contains("max depth") || code == "E4007" {
        Entry {
            plain:    "The delegation chain is too long.",
            cause:    "Too many agents delegated to each other in sequence — chain depth limit reached.",
            fix:      "Reduce the number of delegation hops in your agent architecture.",
            steps:    &[
                "Most setups need only 1-2 levels: human → orchestrator → executor.",
                "Refactor so orchestrators delegate directly to final executors instead of chaining through intermediaries.",
                "If deeper chains are needed, increase A1_MAX_CHAIN_DEPTH in gateway config.",
            ],
            fix_type: "chain",
            severity: "medium",
        }
    } else {
        Entry {
            plain:    "An authorization error occurred.",
            cause:    "The exact cause is unclear from the error message alone.",
            fix:      "Check the Live Log for the full request and response JSON.",
            steps:    &[
                "Open the 'Live Log' tab in A1 Studio.",
                "Find the failed request and expand it to see the full response JSON.",
                "Look for the 'error_code' field and paste it into this Error Help form.",
            ],
            fix_type: "unknown",
            severity: "unknown",
        }
    };

    Json(ExplainErrorResponse {
        plain_english: e.plain.into(),
        likely_cause:  e.cause.into(),
        fix:           e.fix.into(),
        fix_steps:     e.steps.iter().map(|s| s.to_string()).collect(),
        fix_type:      e.fix_type.into(),
        severity:      e.severity.into(),
    })
}

// ─── POST /v1/system/gitignore-add ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GitignoreAddRequest {
    pub directory: String,
    pub pattern: String,
}

#[derive(Debug, Serialize)]
pub struct GitignoreAddResponse {
    pub success: bool,
    pub gitignore_path: Option<String>,
    pub pattern: String,
    pub was_already_present: bool,
    pub error: Option<String>,
}

// ─── GET /v1/passports/read?path=... ─────────────────────────────────────────
//
// Returns the raw JSON content of a passport file for backup purposes.
// Only files inside ~/.a1/passports/ are served — path traversal is rejected.

pub async fn read_passport_handler(
    _state: State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    use axum::http::StatusCode;

    let requested = match params.get("path") {
        Some(p) if !p.is_empty() => std::path::PathBuf::from(p),
        _ => return (StatusCode::BAD_REQUEST, axum::Json(serde_json::json!({"error":"path required"}))).into_response(),
    };

    let allowed_dir = passports_dir();
    let canonical = match requested.canonicalize() {
        Ok(c) => c,
        Err(_) => return (StatusCode::NOT_FOUND, axum::Json(serde_json::json!({"error":"file not found"}))).into_response(),
    };

    if !canonical.starts_with(&allowed_dir) {
        return (StatusCode::FORBIDDEN, axum::Json(serde_json::json!({"error":"access denied"}))).into_response();
    }

    match std::fs::read_to_string(&canonical)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
    {
        Some(v) => axum::Json(v).into_response(),
        None    => (StatusCode::UNPROCESSABLE_ENTITY, axum::Json(serde_json::json!({"error":"invalid passport file"}))).into_response(),
    }
}

// ─── POST /v1/passports/restore ──────────────────────────────────────────────
//
// Receives decrypted passport payloads from the Studio backup/restore flow and
// writes them to ~/.a1/passports/. The browser handles AES-GCM decryption with
// the user's passphrase — the gateway never sees the encrypted bytes.

#[derive(Debug, Deserialize)]
pub struct RestorePassport {
    pub path:      Option<String>,
    pub namespace: Option<String>,
    pub content:   serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct RestoreRequest {
    pub passports: Vec<RestorePassport>,
}

#[derive(Debug, Serialize)]
pub struct RestoreResponse {
    pub success:  bool,
    pub restored: usize,
    pub skipped:  usize,
    pub error:    Option<String>,
}

pub async fn restore_passports_handler(
    _state: State<Arc<AppState>>,
    Json(req): Json<RestoreRequest>,
) -> impl IntoResponse {
    let dir = passports_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return Json(RestoreResponse { success: false, restored: 0, skipped: 0, error: Some(format!("Cannot create passports dir: {e}")) });
    }

    let mut restored = 0usize;
    let mut skipped  = 0usize;

    for pp in req.passports {
        let filename = pp.namespace
            .as_deref()
            .map(|ns| ns.to_string() + ".json")
            .or_else(|| {
                pp.path.as_deref()
                    .and_then(|p| std::path::Path::new(p).file_name())
                    .map(|n| n.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| format!("restored-{}.json", restored + skipped));

        if !filename.ends_with(".json") || filename.contains("..") || filename.contains('/') {
            skipped += 1;
            continue;
        }

        let dest = dir.join(&filename);
        let json = match serde_json::to_string_pretty(&pp.content) {
            Ok(j) => j,
            Err(_) => { skipped += 1; continue; }
        };

        if dest.exists() {
            let bak = dest.with_extension("json.bak");
            let _ = std::fs::rename(&dest, &bak);
        }

        if std::fs::write(&dest, json).is_ok() { restored += 1; } else { skipped += 1; }
    }

    Json(RestoreResponse { success: true, restored, skipped, error: None })
}

pub async fn gitignore_add_handler(
    _state: State<Arc<AppState>>,
    Json(req): Json<GitignoreAddRequest>,
) -> impl IntoResponse {
    let dir = std::path::PathBuf::from(&req.directory);

    // Find the git root by walking up
    let git_root = {
        let mut current = dir.clone();
        loop {
            if current.join(".git").is_dir() {
                break Some(current);
            }
            if !current.pop() {
                break None;
            }
        }
    };

    let root = match git_root {
        Some(r) => r,
        None => return Json(GitignoreAddResponse {
            success: false,
            gitignore_path: None,
            pattern: req.pattern.clone(),
            was_already_present: false,
            error: Some("Not inside a Git repository. No .gitignore needed (but keep the file safe regardless).".into()),
        }),
    };

    let gi_path = root.join(".gitignore");

    // Read existing content
    let existing = std::fs::read_to_string(&gi_path).unwrap_or_default();

    // Check if pattern already present
    let pattern_clean = req.pattern.trim().to_string();
    if existing.lines().any(|l| l.trim() == pattern_clean) {
        return Json(GitignoreAddResponse {
            success: true,
            gitignore_path: Some(gi_path.display().to_string()),
            pattern: pattern_clean,
            was_already_present: true,
            error: None,
        });
    }

    // Append pattern
    let new_content = if existing.is_empty() || existing.ends_with('\n') {
        format!("{existing}# A1 passport — do not commit\n{pattern_clean}\n")
    } else {
        format!("{existing}\n# A1 passport — do not commit\n{pattern_clean}\n")
    };

    match std::fs::write(&gi_path, new_content) {
        Ok(()) => Json(GitignoreAddResponse {
            success: true,
            gitignore_path: Some(gi_path.display().to_string()),
            pattern: pattern_clean,
            was_already_present: false,
            error: None,
        }),
        Err(e) => Json(GitignoreAddResponse {
            success: false,
            gitignore_path: Some(gi_path.display().to_string()),
            pattern: pattern_clean,
            was_already_present: false,
            error: Some(format!("Could not write .gitignore: {e}")),
        }),
    }
}
