use axum::response::Html;

/// GET /studio — Serve the A1 Studio dashboard.
///
/// Serving strategy (checked in order):
/// 1. Path in A1_STUDIO_PATH env var (allows hot-swap without Docker rebuild)
/// 2. /studio/index.html  (Docker volume-mount path, set in docker-compose.yml)
/// 3. Embedded bytes compiled in at build time (always available as fallback)
pub async fn handler() -> Html<String> {
    if let Ok(path) = std::env::var("A1_STUDIO_PATH") {
        if let Ok(html) = std::fs::read_to_string(&path) {
            return Html(html);
        }
    }
    if let Ok(html) = std::fs::read_to_string("/studio/index.html") {
        return Html(html);
    }
    Html(include_str!("../../../studio/index.html").to_string())
}