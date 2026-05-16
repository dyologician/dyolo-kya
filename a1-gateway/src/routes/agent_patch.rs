/// POST /v1/agents/read-file  — Read an agent skill/tool file from disk
/// POST /v1/agents/write-file — Write patched content to a file (auto-backup)
/// GET  /v1/agents/list-files — List Python/TS/Rust files in a directory
///
/// These routes exist solely to support the AI Integration Assistant in
/// A1 Studio. The assistant (Claude) reads a user's agent skill file,
/// patches it to add A1 guards, and writes it back — with an automatic
/// .bak backup created before every write.
///
/// Security model:
/// - Only files inside the user's home directory are accessible.
/// - Symlinks are rejected.
/// - /etc, /sys, /proc, /dev, /boot, /var/run are blocked.
/// - Max file size for reading: 256 KB (enough for any agent skill file).
/// - Max write size: 512 KB.
/// - Gateway runs on localhost — network-level access control is the
///   responsibility of the deploying user. Do not expose this port publicly.
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

// ─── Path validation ─────────────────────────────────────────────────────────

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

/// Returns Ok(canonical_path) if the path is safe to access, Err(reason) otherwise.
fn validate_path(raw: &str) -> Result<PathBuf, String> {
    if raw.trim().is_empty() {
        return Err("Path is empty".into());
    }

    // Expand ~ to home dir
    let expanded = if raw.starts_with("~/") {
        home_dir().join(&raw[2..])
    } else if raw == "~" {
        home_dir()
    } else {
        PathBuf::from(raw)
    };

    // Must be absolute for canonicalization to be reliable
    let absolute = if expanded.is_absolute() {
        expanded
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| home_dir())
            .join(expanded)
    };

    // Canonicalize (resolves .. and symlinks)
    let canonical = match absolute.canonicalize() {
        Ok(p) => p,
        Err(e) => return Err(format!("Cannot resolve path: {e}")),
    };

    // Reject symlinks — the resolved path must match what we computed
    if canonical.read_link().is_ok() {
        return Err("Symlinks are not allowed".into());
    }

    // Must be inside home directory or /tmp (for CI/test environments)
    let home = home_dir();
    let in_home = canonical.starts_with(&home);
    let in_tmp = canonical.starts_with("/tmp") || canonical.starts_with("/var/tmp");
    let in_cwd = canonical.starts_with(
        std::env::current_dir().unwrap_or_else(|_| home_dir())
    );

    if !in_home && !in_tmp && !in_cwd {
        return Err(format!(
            "Path must be inside your home directory ({}). Got: {}",
            home.display(),
            canonical.display()
        ));
    }

    // Block system directories
    const BLOCKED: &[&str] = &["/etc", "/sys", "/proc", "/dev", "/boot", "/var/run", "/sbin", "/bin", "/usr/bin"];
    for blocked in BLOCKED {
        if canonical.starts_with(blocked) {
            return Err(format!("Access to {} is blocked", blocked));
        }
    }

    Ok(canonical)
}

// ─── POST /v1/agents/read-file ───────────────────────────────────────────────

const MAX_READ_BYTES: u64 = 256 * 1024; // 256 KB

#[derive(Debug, Deserialize)]
pub struct ReadFileRequest {
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct ReadFileResponse {
    pub path: String,
    pub content: Option<String>,
    pub lines: usize,
    pub size_bytes: usize,
    pub error: Option<String>,
}

pub async fn read_file_handler(
    _state: State<Arc<AppState>>,
    Json(req): Json<ReadFileRequest>,
) -> impl IntoResponse {
    match validate_path(&req.path) {
        Err(e) => Json(ReadFileResponse {
            path: req.path,
            content: None,
            lines: 0,
            size_bytes: 0,
            error: Some(e),
        }),
        Ok(path) => {
            // Size check before full read
            match std::fs::metadata(&path) {
                Ok(meta) if meta.len() > MAX_READ_BYTES => {
                    Json(ReadFileResponse {
                        path: path.display().to_string(),
                        content: None,
                        lines: 0,
                        size_bytes: meta.len() as usize,
                        error: Some(format!(
                            "File is too large ({} KB). Max readable size is 256 KB.",
                            meta.len() / 1024
                        )),
                    })
                }
                Err(e) => Json(ReadFileResponse {
                    path: path.display().to_string(),
                    content: None,
                    lines: 0,
                    size_bytes: 0,
                    error: Some(format!("Cannot read file: {e}")),
                }),
                Ok(_) => match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        let lines = content.lines().count();
                        let size = content.len();
                        Json(ReadFileResponse {
                            path: path.display().to_string(),
                            content: Some(content),
                            lines,
                            size_bytes: size,
                            error: None,
                        })
                    }
                    Err(e) => Json(ReadFileResponse {
                        path: path.display().to_string(),
                        content: None,
                        lines: 0,
                        size_bytes: 0,
                        error: Some(format!("Cannot read: {e} (is this a binary file?)")),
                    }),
                },
            }
        }
    }
}

// ─── POST /v1/agents/write-file ──────────────────────────────────────────────

const MAX_WRITE_BYTES: usize = 512 * 1024; // 512 KB

#[derive(Debug, Deserialize)]
pub struct WriteFileRequest {
    pub path: String,
    pub content: String,
    /// If true, create a .bak copy before writing. Default true.
    pub backup: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct WriteFileResponse {
    pub path: String,
    pub backup_path: Option<String>,
    pub bytes_written: usize,
    pub success: bool,
    pub error: Option<String>,
}

pub async fn write_file_handler(
    _state: State<Arc<AppState>>,
    Json(req): Json<WriteFileRequest>,
) -> impl IntoResponse {
    if req.content.len() > MAX_WRITE_BYTES {
        return (StatusCode::PAYLOAD_TOO_LARGE, Json(WriteFileResponse {
            path: req.path,
            backup_path: None,
            bytes_written: 0,
            success: false,
            error: Some("Content exceeds 512 KB write limit".into()),
        })).into_response();
    }

    match validate_path(&req.path) {
        Err(e) => (StatusCode::BAD_REQUEST, Json(WriteFileResponse {
            path: req.path,
            backup_path: None,
            bytes_written: 0,
            success: false,
            error: Some(e),
        })).into_response(),

        Ok(path) => {
            // Create backup if requested (default: true) and file already exists
            let backup_path = if req.backup.unwrap_or(true) && path.exists() {
                let bak = path.with_extension(
                    path.extension()
                        .map(|e| format!("{}.bak", e.to_string_lossy()))
                        .unwrap_or_else(|| "bak".into())
                );
                if let Err(e) = std::fs::copy(&path, &bak) {
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(WriteFileResponse {
                        path: path.display().to_string(),
                        backup_path: None,
                        bytes_written: 0,
                        success: false,
                        error: Some(format!("Could not create backup: {e}")),
                    })).into_response();
                }
                Some(bak.display().to_string())
            } else {
                None
            };

            // Ensure parent directory exists
            if let Some(parent) = path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(WriteFileResponse {
                        path: path.display().to_string(),
                        backup_path,
                        bytes_written: 0,
                        success: false,
                        error: Some(format!("Cannot create parent directory: {e}")),
                    })).into_response();
                }
            }

            let bytes_written = req.content.len();
            match std::fs::write(&path, &req.content) {
                Ok(()) => Json(WriteFileResponse {
                    path: path.display().to_string(),
                    backup_path,
                    bytes_written,
                    success: true,
                    error: None,
                }).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(WriteFileResponse {
                    path: path.display().to_string(),
                    backup_path,
                    bytes_written: 0,
                    success: false,
                    error: Some(format!("Write failed: {e}")),
                })).into_response(),
            }
        }
    }
}

// ─── GET /v1/agents/list-files ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListFilesQuery {
    pub path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListFilesResponse {
    pub directory: String,
    pub files: Vec<FileEntry>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub extension: String,
}

pub async fn list_files_handler(
    _state: State<Arc<AppState>>,
    axum::extract::Query(query): axum::extract::Query<ListFilesQuery>,
) -> impl IntoResponse {
    let raw = query.path.unwrap_or_else(|| "~".into());

    match validate_path(&raw) {
        Err(e) => Json(ListFilesResponse {
            directory: raw,
            files: vec![],
            error: Some(e),
        }),
        Ok(dir) => {
            if !dir.is_dir() {
                return Json(ListFilesResponse {
                    directory: dir.display().to_string(),
                    files: vec![],
                    error: Some("Path is not a directory".into()),
                });
            }

            // Only show code files relevant to AI agents
            const CODE_EXTS: &[&str] = &["py", "ts", "js", "mts", "mjs", "go", "rs", "rb", "toml", "json", "yaml", "yml"];

            let mut files: Vec<FileEntry> = std::fs::read_dir(&dir)
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|entry| {
                    let path = entry.path();
                    if !path.is_file() { return None; }
                    let name = path.file_name()?.to_string_lossy().into_owned();
                    // Skip hidden files and backups
                    if name.starts_with('.') || name.ends_with(".bak") { return None; }
                    let ext = path.extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if !CODE_EXTS.contains(&ext.as_str()) { return None; }
                    let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                    Some(FileEntry {
                        name,
                        path: path.display().to_string(),
                        size_bytes: size,
                        extension: ext,
                    })
                })
                .collect();

            files.sort_by(|a, b| a.name.cmp(&b.name));

            Json(ListFilesResponse {
                directory: dir.display().to_string(),
                files,
                error: None,
            })
        }
    }
}
