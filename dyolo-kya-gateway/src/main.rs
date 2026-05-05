mod routes;
mod state;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Router,
    extract::State,
    http::{StatusCode, HeaderValue, Method},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use axum_client_ip::{SecureClientIp, SecureClientIpSource};
use tower_http::{cors::{CorsLayer, AllowOrigin}, trace::TraceLayer};

use crate::state::AppState;

async fn per_ip_rate_limit(
    State(state): State<Arc<AppState>>,
    ip: SecureClientIp,
    request: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Response {
    // SecureClientIp enforces strict resolution. If an attacker injects a spoofed
    // X-Forwarded-For header, it is ignored unless the request originates from a
    // explicitly trusted proxy subnet (or the fallback mode is active).
    let addr = std::net::SocketAddr::new(ip.0, 0);
    if state.rate_limiter.check_key(&addr).is_err() {
        return (StatusCode::TOO_MANY_REQUESTS, "rate limit exceeded").into_response();
    }
    next.run(request).await
}

async fn require_auth(
    State(state): State<Arc<AppState>>,
    request: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Response {
    if let Some(secret) = &state.admin_secret {
        let auth_header = request.headers().get("Authorization").and_then(|h| h.to_str().ok());
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
                .unwrap_or_else(|_| "dyolo_kya_gateway=info,tower_http=info".into()),
        )
        .init();

    let state = match state::AppState::from_env().await {
        Ok(s) => Arc::new(s),
        Err(e) => {
            tracing::error!("Failed to initialize gateway state: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!(signing_pk = %state.gateway_pk_hex, "dyolo-kya-gateway v2.0.0 starting");

    let cors = match std::env::var("DYOLO_CORS_ALLOWED_ORIGIN") {
        Ok(origin) if origin == "*" => CorsLayer::permissive(),
        Ok(origin) => CorsLayer::new()
            .allow_origin(origin.parse::<HeaderValue>().expect("invalid CORS origin"))
            .allow_methods([Method::GET, Method::POST]),
        Err(_) => CorsLayer::new().allow_methods([Method::GET, Method::POST]), // default restrictive
    };

    let admin_routes = Router::new()
        .route("/v1/cert/issue",       post(routes::cert::issue_handler))
        .route("/v1/cert/issue-batch", post(routes::cert::issue_batch_handler))
        .layer(middleware::from_fn_with_state(state.clone(), require_auth));

    let public_routes = Router::new()
        .route("/health",                          get(routes::health::handler))
        .route("/.well-known/kya-configuration",   get(routes::wellknown::handler))
        .route("/v1/authorize",                    post(routes::authorize::handler))
        .route("/v1/authorize/batch",              post(routes::batch::handler))
        .route("/v1/cert/revoke",                  post(routes::cert::revoke_handler))
        .route("/v1/cert/revoke-batch",            post(routes::cert::revoke_batch_handler))
        .route("/v1/cert/:fp",                     get(routes::cert::inspect_handler))
        .route("/v1/token/verify",                 post(routes::chain::verify_handler));

    let ip_source = match std::env::var("DYOLO_TRUSTED_PROXY_MODE") {
        Ok(val) if val == "x-forwarded-for" => SecureClientIpSource::RightmostXForwardedFor,
        Ok(val) if val == "fly-client-ip"   => SecureClientIpSource::FlyClientIp,
        Ok(val) if val == "cf-connecting-ip"=> SecureClientIpSource::CfConnectingIp,
        _ => SecureClientIpSource::ConnectInfo, // Default strictly to raw socket layer to prevent spoofing
    };

    let app = Router::new()
        .merge(admin_routes)
        .merge(public_routes)
        .layer(middleware::from_fn_with_state(state.clone(), per_ip_rate_limit))
        // Extract IP securely using the configured proxy topology
        .layer(ip_source.into_extension()) 
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    let addr = std::env::var("GATEWAY_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap_or_else(|e| {
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

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received, starting graceful shutdown");
}