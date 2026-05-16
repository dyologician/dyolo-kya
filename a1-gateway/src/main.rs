mod routes;
mod state;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use axum_client_ip::{SecureClientIp, SecureClientIpSource};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::state::AppState;

async fn per_ip_rate_limit(
    State(state): State<Arc<AppState>>,
    ip: SecureClientIp,
    request: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Response {
    if state.rate_limiter.check_key(&ip.0).is_err() {
        return (StatusCode::TOO_MANY_REQUESTS, "rate limit exceeded").into_response();
    }
    next.run(request).await
}

async fn inject_protocol_header(
    request: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        "X-A1-Protocol",
        HeaderValue::from_static("dyolo_v2.8.0"),
    );
    response
}

async fn require_auth(
    State(state): State<Arc<AppState>>,
    request: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Response {
    if let Some(secret) = &state.admin_secret {
        let auth_header = request
            .headers()
            .get("Authorization")
            .and_then(|h| h.to_str().ok());
        let expected = format!("Bearer {}", secret);
        if auth_header != Some(expected.as_str()) {
            return (StatusCode::UNAUTHORIZED, "invalid or missing admin secret").into_response();
        }
    }
    next.run(request).await
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "a1_gateway=info,tower_http=info".into()),
        )
        .init();

    let state = match state::AppState::from_env().await {
        Ok(s) => Arc::new(s),
        Err(e) => {
            tracing::error!("Failed to initialize gateway state: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!(signing_pk = %state.gateway_pk_hex, "a1-gateway v2.8.0 starting");

    let cors = match std::env::var("A1_CORS_ALLOWED_ORIGIN") {
        Ok(origin) if origin == "*" => CorsLayer::permissive(),
        Ok(origin) => CorsLayer::new()
            .allow_origin(origin.parse::<HeaderValue>().expect("invalid CORS origin"))
            .allow_methods([Method::GET, Method::POST, Method::DELETE]),
        Err(_) => CorsLayer::new().allow_methods([Method::GET, Method::POST, Method::DELETE]),
    };

    let admin_routes = Router::new()
        .route("/v1/cert/issue",              post(routes::cert::issue_handler))
        .route("/v1/cert/issue-batch",        post(routes::cert::issue_batch_handler))
        .route("/v1/vc/issue",                post(routes::did::issue_vc_handler))
        .route("/v1/swarm/create",            post(routes::swarm::create_handler))
        .route("/v1/swarm/member/add",        post(routes::swarm::add_member_handler))
        .route("/v1/swarm/member/remove",     post(routes::swarm::remove_member_handler))
        .route("/v1/governance/audit-report", post(routes::governance::audit_report_handler))
        .route("/v1/webhook/test",            post(routes::webhook::test_handler))
        .route("/v1/jwt/exchange",            post(routes::jwt_bridge::exchange_handler))
        .layer(middleware::from_fn_with_state(state.clone(), require_auth));

    let public_routes = Router::new()
        // Health — both spellings accepted (scripts use /healthz)
        .route("/health",  get(routes::health::handler))
        .route("/healthz", get(routes::health::handler))
        // Studio UI
        .route("/studio",  get(routes::studio::handler))
        .route("/studio/", get(routes::studio::handler))
        // Well-known discovery
        .route("/.well-known/a1-configuration", get(routes::wellknown::handler))
        // Core authorization
        .route("/v1/authorize",          post(routes::authorize::handler))
        .route("/v1/authorize/batch",    post(routes::batch::handler))
        .route("/v1/passport/authorize", post(routes::passport::handler))
        // Studio lightweight policy check (no signed chain needed — for A1 Studio proof panel)
        .route("/v1/studio/check",       post(routes::governance::studio_check_handler))
        // Certificate lifecycle
        .route("/v1/cert/revoke",        post(routes::cert::revoke_handler))
        .route("/v1/cert/revoke-batch",  post(routes::cert::revoke_batch_handler))
        .route("/v1/cert/:fp",           get(routes::cert::inspect_handler))
        // Token verification
        .route("/v1/token/verify", post(routes::chain::verify_handler))
        // DID / Verifiable Credentials
        .route("/v1/did/gateway",  get(routes::did::gateway_did_handler))
        .route("/v1/did/:pk_hex",  get(routes::did::resolve_handler))
        .route("/v1/vc/verify",    post(routes::did::verify_vc_handler))
        // Anchor & negotiate
        .route("/v1/anchor",    post(routes::anchor::handler))
        .route("/v1/negotiate", post(routes::negotiate::handler))
        // Swarm (read-only public)
        .route("/v1/swarm/:swarm_id/members", get(routes::swarm::list_members_handler))
        // Governance (read-only public)
        .route("/v1/governance/policy",          get(routes::governance::policy_handler))
        .route("/v1/governance/approval/verify", post(routes::governance::verify_approval_handler))
        // AI proxy — local LLM relay
        .route("/v1/ai/status", get(routes::ai_proxy::status_handler))
        .route("/v1/ai/chat",   post(routes::ai_proxy::chat_handler))
        // MCP — zero-code agent integration
        .route("/mcp",       get(routes::mcp::sse_handler).post(routes::mcp::post_handler))
        .route("/mcp/tools", get(routes::mcp::tools_handler))
        // Agent auto-connect
        .route("/v1/agents/scan",       get(routes::agent_connect::scan_handler))
        .route("/v1/agents/connect",    post(routes::agent_connect::connect_handler))
        .route("/v1/agents/restart",    post(routes::agent_connect::restart_handler))
        // Agent pull/install (SSE streaming) and remove (uninstall)
        .route("/v1/agents/pull",       get(routes::agent_connect::pull_handler))
        .route("/v1/agents/remove",     post(routes::agent_connect::remove_handler))
        // Live connection proof — reads real config, runs binary, tests A1 policy
        .route("/v1/agents/probe-live", post(routes::agent_connect::probe_live_handler))
        // Agent relay — Direct Connect tab
        .route("/v1/agents/probe",             post(routes::agent_relay::probe_handler))
        .route("/v1/agents/relay",             post(routes::agent_relay::relay_handler))
        .route("/v1/agents/integration-check", get(routes::agent_relay::integration_check_handler))
        // AI Integration — read/write agent source files
        .route("/v1/agents/read-file",  post(routes::agent_patch::read_file_handler))
        .route("/v1/agents/write-file", post(routes::agent_patch::write_file_handler))
        .route("/v1/agents/list-files", get(routes::agent_patch::list_files_handler))
        // Passport lifecycle
        .route("/v1/passports/issue",               post(routes::passports::issue_passport_handler))
        .route("/v1/passports/list",                get(routes::passports::list_passports_handler))
        .route("/v1/passports/renew",               post(routes::passports::renew_passport_handler))
        .route("/v1/passports/read",                get(routes::passports::read_passport_handler))
        .route("/v1/passports/restore",             post(routes::passports::restore_passports_handler))
        .route("/v1/passports/revoke-by-namespace", post(routes::passports::revoke_by_namespace_handler))
        // System — autostart daemon
        .route("/v1/system/autostart",
            post(routes::passports::install_autostart_handler)
            .delete(routes::passports::remove_autostart_handler))
        // System — status (autostart state, docker presence, platform)
        .route("/v1/system/status", get(routes::automagic::get_status))
        // System — graceful shutdown (Studio Stop button)
        .route("/v1/system/shutdown", post(system_shutdown_handler))
        // Agents — disconnect (remove A1 from agent config)
        .route("/v1/agents/disconnect", post(routes::agent_connect::disconnect_handler))
        // System — trigger Docker Desktop installation
        .route("/v1/system/install-docker", post(routes::automagic::install_docker))
        // System — gitignore protection
        .route("/v1/system/gitignore-add", post(routes::passports::gitignore_add_handler))
        // Debug — plain-English error explanations
        .route("/v1/debug/explain-error", post(routes::passports::explain_error_handler))
        // Tenant — multi-tenant context and config
        .route("/v1/tenant/info",   get(routes::tenant::info_handler))
        .route("/v1/tenant/config", get(routes::tenant::config_handler))
        // Webhook — status (URL redacted for security)
        .route("/v1/webhook/status", get(routes::webhook::status_handler));

    let ip_source = match std::env::var("A1_TRUSTED_PROXY_MODE")
        .as_deref()
        .unwrap_or("")
    {
        "x-forwarded-for"  => SecureClientIpSource::RightmostXForwardedFor,
        "fly-client-ip"    => SecureClientIpSource::FlyClientIp,
        "cf-connecting-ip" => SecureClientIpSource::CfConnectingIp,
        _                  => SecureClientIpSource::ConnectInfo,
    };

    let app = Router::new()
        .merge(admin_routes)
        .merge(public_routes)
        .layer(middleware::from_fn_with_state(state.clone(), per_ip_rate_limit))
        .layer(middleware::from_fn(inject_protocol_header))
        .layer(ip_source.into_extension())
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    let addr = std::env::var("GATEWAY_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Failed to bind to {addr}: {e}");
            std::process::exit(1);
        });

    tracing::info!(addr = %addr, "listening");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}

async fn system_shutdown_handler() -> impl axum::response::IntoResponse {
    // Respond first, then schedule shutdown after a brief delay so the response arrives.
    tokio::spawn(async {
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        #[cfg(unix)]
        {
            // Send SIGTERM to ourselves — triggers the graceful_shutdown listener.
            let pid = std::process::id();
            unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM); }
        }
        #[cfg(not(unix))]
        std::process::exit(0);
    });
    axum::Json(serde_json::json!({"ok": true, "message": "Shutting down A1 gateway"}))
}


async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::warn!("Ctrl+C handler error: {e}");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                tracing::warn!("SIGTERM handler install failed: {e}");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received, starting graceful shutdown");
}
