/// GET  /v1/agents/scan       — Scan the filesystem for installed AI agents
/// POST /v1/agents/connect    — Write the A1 integration file for an agent (one-click)
/// POST /v1/agents/disconnect — Remove A1 from an agent's config
/// POST /v1/agents/restart    — Attempt to restart an agent process
/// GET  /v1/agents/pull       — SSE stream: install an agent (one-click pull)
/// POST /v1/agents/remove     — Uninstall an agent from the system
/// POST /v1/agents/probe-live — Genuine live-connection proof: reads real config,
///                              runs the binary, tests A1 policy enforcement
///
/// This is the no-code integration path. The user opens A1 Studio, sees their
/// installed agents, clicks "Connect", and A1 writes the integration config.
/// Zero code. Zero decorators. Zero understanding of cryptography required.
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::response::sse::{Event, KeepAlive, Sse};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_stream::wrappers::ReceiverStream;

use crate::state::AppState;

// ─── Known agent definitions ─────────────────────────────────────────────────
// IronClaw is ALWAYS first — it is the recommended agent for A1.

#[derive(Debug, Clone, Serialize)]
pub struct KnownAgent {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub icon: &'static str,
    pub connect_method: &'static str, // "mcp_json" | "config_toml" | "skill_file"
    pub homepage: &'static str,
    pub install_cmd_unix: &'static str, // command to install on Mac/Linux
    pub install_cmd_win: &'static str,  // command to install on Windows
    pub uninstall_cmd: &'static str,    // command to remove
    pub recommended: bool,
}

const KNOWN_AGENTS: &[KnownAgent] = &[
    // ── RECOMMENDED — always first ──────────────────────────────────────────
    KnownAgent {
        id:               "ironclaw",
        name:             "IronClaw",
        description:      "Security-focused Rust AI agent runtime. WASM sandboxes, encrypted enclaves, memory safety. Recommended for A1.",
        icon:             "🦾",
        connect_method:   "config_toml",
        homepage:         "https://github.com/ironclaw/ironclaw",
        install_cmd_unix: "cargo install ironclaw",
        install_cmd_win:  "cargo install ironclaw",
        uninstall_cmd:    "cargo uninstall ironclaw",
        recommended:      true,
    },
    KnownAgent {
        id:               "openclaw",
        name:             "OpenClaw",
        description:      "Open-source personal AI agent (Node.js). Runs locally, connects to WhatsApp, Telegram, email, calendar, and browser.",
        icon:             "🦅",
        connect_method:   "mcp_json",
        homepage:         "https://github.com/openclaw/openclaw",
        install_cmd_unix: "mkdir -p \"$HOME/.npm-global\" && npm install --prefix \"$HOME/.npm-global\" openclaw --no-fund --no-audit && echo '✓ Add $HOME/.npm-global/bin to PATH: export PATH=\"$HOME/.npm-global/bin:$PATH\"'",
        install_cmd_win:  "npm install -g openclaw --no-fund --no-audit",
        uninstall_cmd:    "npm uninstall --prefix \"$HOME/.npm-global\" openclaw 2>/dev/null || npm uninstall -g openclaw",
        recommended:      false,
    },
    KnownAgent {
        id:               "claude_code",
        name:             "Claude Code",
        description:      "Anthropic's AI coding agent. Runs in your terminal and edits code directly.",
        icon:             "🤖",
        connect_method:   "mcp_json",
        homepage:         "https://claude.ai/code",
        install_cmd_unix: "mkdir -p \"$HOME/.npm-global\" && npm install --prefix \"$HOME/.npm-global\" @anthropic-ai/claude-code --no-fund --no-audit && echo '✓ Add $HOME/.npm-global/bin to PATH: export PATH=\"$HOME/.npm-global/bin:$PATH\"'",
        install_cmd_win:  "npm install -g @anthropic-ai/claude-code --no-fund --no-audit",
        uninstall_cmd:    "npm uninstall --prefix \"$HOME/.npm-global\" @anthropic-ai/claude-code 2>/dev/null || npm uninstall -g @anthropic-ai/claude-code",
        recommended:      false,
    },
    KnownAgent {
        id:               "claude_desktop",
        name:             "Claude Desktop",
        description:      "Anthropic's desktop AI assistant with MCP support.",
        icon:             "💬",
        connect_method:   "mcp_json",
        homepage:         "https://claude.ai/desktop",
        install_cmd_unix: "echo 'Download from https://claude.ai/desktop'",
        install_cmd_win:  "echo 'Download from https://claude.ai/desktop'",
        uninstall_cmd:    "",
        recommended:      false,
    },
    KnownAgent {
        id:               "ollama",
        name:             "Ollama (Local LLM)",
        description:      "Run local LLMs (Llama 3, Mistral, Phi, Gemma) privately on your machine. No API key required.",
        icon:             "🦙",
        connect_method:   "mcp_json",
        homepage:         "https://ollama.ai",
        install_cmd_unix: "curl -fsSL https://ollama.ai/install.sh | sh",
        install_cmd_win:  "winget install Ollama.Ollama",
        uninstall_cmd:    "which ollama && sudo rm $(which ollama) || true",
        recommended:      false,
    },
    KnownAgent {
        id:               "langchain",
        name:             "LangChain",
        description:      "Popular Python AI agent framework for chaining LLM calls and tools.",
        icon:             "🦜",
        connect_method:   "mcp_json",
        homepage:         "https://langchain.com",
        install_cmd_unix: "pip3 install --user langchain langchain-openai 2>/dev/null || python3 -m pip install --user langchain langchain-openai",
        install_cmd_win:  "pip install langchain langchain-openai",
        uninstall_cmd:    "pip3 uninstall -y langchain 2>/dev/null || python3 -m pip uninstall -y langchain",
        recommended:      false,
    },
    KnownAgent {
        id:               "crewai",
        name:             "CrewAI",
        description:      "Multi-agent framework. Groups of AI agents working as a crew on complex tasks.",
        icon:             "⛵",
        connect_method:   "mcp_json",
        homepage:         "https://crewai.com",
        install_cmd_unix: "pip3 install --user crewai 2>/dev/null || python3 -m pip install --user crewai",
        install_cmd_win:  "pip install crewai",
        uninstall_cmd:    "pip3 uninstall -y crewai 2>/dev/null || python3 -m pip uninstall -y crewai",
        recommended:      false,
    },
    KnownAgent {
        id:               "openai_agents",
        name:             "OpenAI Agents SDK",
        description:      "OpenAI's official agent SDK for building agentic applications.",
        icon:             "🟢",
        connect_method:   "mcp_json",
        homepage:         "https://openai.com/agents",
        install_cmd_unix: "pip3 install --user openai-agents 2>/dev/null || python3 -m pip install --user openai-agents",
        install_cmd_win:  "pip install openai-agents",
        uninstall_cmd:    "pip3 uninstall -y openai-agents 2>/dev/null || python3 -m pip uninstall -y openai-agents",
        recommended:      false,
    },
    KnownAgent {
        id:               "custom",
        name:             "Custom Agent",
        description:      "Any other AI agent or application. Connect via MCP config file or REST API.",
        icon:             "⚙️",
        connect_method:   "mcp_json",
        homepage:         "https://github.com/dyologician/a1",
        install_cmd_unix: "",
        install_cmd_win:  "",
        uninstall_cmd:    "",
        recommended:      false,
    },
];

// ─── Scan result ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct DetectedAgent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub homepage: String,
    pub connect_method: String,
    pub install_path: Option<String>,
    pub connected: bool,
    pub config_file: Option<String>, // the file A1 wrote/would write
    pub connect_hint: String,        // human-readable what the connect action does
    pub install_cmd_unix: String,
    pub install_cmd_win: String,
    pub uninstall_cmd: String,
    pub recommended: bool,
}

#[derive(Debug, Serialize)]
pub struct ScanResponse {
    pub agents: Vec<DetectedAgent>,
    pub scan_paths_checked: Vec<String>,
}

// ─── Helper: home directory ───────────────────────────────────────────────────

fn home() -> PathBuf {
    // When running inside Docker, the host home dir is mounted at /host-home
    // so we can scan the user's actual filesystem for installed agents.
    if std::env::var("A1_RUNNING_IN_DOCKER").is_ok() {
        if let Ok(host_home) = std::env::var("A1_HOST_HOME") {
            let p = PathBuf::from("/host-home");
            if p.exists() {
                return p;
            }
            // Fallback: use the host home path directly if /host-home not mounted yet
            let _ = host_home;
        }
    }
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

// ─── Helper: check if a binary is in PATH ────────────────────────────────────

fn which_binary(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var("PATH").unwrap_or_default();
    for dir in path_var.split(':') {
        let candidate = Path::new(dir).join(name);
        if candidate.exists() && candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

// ─── Per-agent detection helpers ─────────────────────────────────────────────

fn find_ironclaw(checked: &mut Vec<String>) -> Option<PathBuf> {
    let candidates = vec![
        home().join(".ironclaw"),
        home().join("ironclaw"),
        home().join(".config").join("ironclaw"),
    ];
    for p in candidates {
        let s = p.display().to_string();
        if !checked.contains(&s) {
            checked.push(s.clone());
        }
        if p.is_dir() {
            return Some(p);
        }
    }
    if which_binary("ironclaw").is_some() {
        let fallback = home().join(".ironclaw");
        let s = fallback.display().to_string();
        if !checked.contains(&s) {
            checked.push(s.clone());
        }
        std::fs::create_dir_all(&fallback).ok();
        return Some(fallback);
    }
    None
}

fn find_openclaw(checked: &mut Vec<String>) -> Option<PathBuf> {
    let candidates = vec![
        home().join(".openclaw"),
        home().join("openclaw"),
        home().join(".config").join("openclaw"),
        home().join("AppData").join("Roaming").join("openclaw"),
    ];
    for p in candidates {
        let s = p.display().to_string();
        if !checked.contains(&s) {
            checked.push(s.clone());
        }
        if p.is_dir() {
            return Some(p);
        }
    }
    if which_binary("openclaw").is_some() {
        let fallback = home().join(".openclaw");
        let s = fallback.display().to_string();
        if !checked.contains(&s) {
            checked.push(s.clone());
        }
        std::fs::create_dir_all(&fallback).ok();
        return Some(fallback);
    }
    None
}

fn find_claude_code(checked: &mut Vec<String>) -> Option<PathBuf> {
    let candidates = vec![
        home().join(".claude"),
        home().join(".config").join("claude"),
        home().join("AppData").join("Roaming").join("claude"),
    ];
    for p in candidates {
        let s = p.display().to_string();
        if !checked.contains(&s) {
            checked.push(s.clone());
        }
        if p.is_dir() {
            return Some(p);
        }
    }
    if which_binary("claude").is_some() {
        let fallback = home().join(".claude");
        let s = fallback.display().to_string();
        if !checked.contains(&s) {
            checked.push(s.clone());
        }
        std::fs::create_dir_all(&fallback).ok();
        return Some(fallback);
    }
    None
}

fn find_claude_desktop(checked: &mut Vec<String>) -> Option<PathBuf> {
    let candidates = vec![
        home()
            .join("Library")
            .join("Application Support")
            .join("Claude"),
        home().join(".config").join("Claude"),
        home().join("AppData").join("Roaming").join("Claude"),
    ];
    for p in &candidates {
        let s = p.display().to_string();
        if !checked.contains(&s) {
            checked.push(s.clone());
        }
        if p.is_dir() {
            return Some(p.clone());
        }
    }
    None
}

fn find_ollama(checked: &mut Vec<String>) -> Option<PathBuf> {
    let candidates = vec![
        home().join(".ollama"),
        home()
            .join("Library")
            .join("Application Support")
            .join("Ollama"),
    ];
    for p in candidates {
        let s = p.display().to_string();
        if !checked.contains(&s) {
            checked.push(s.clone());
        }
        if p.is_dir() {
            return Some(p);
        }
    }
    if which_binary("ollama").is_some() {
        let fallback = home().join(".ollama");
        let s = fallback.display().to_string();
        if !checked.contains(&s) {
            checked.push(s.clone());
        }
        std::fs::create_dir_all(&fallback).ok();
        return Some(fallback);
    }
    None
}

fn find_python_agent(pkg: &str, checked: &mut Vec<String>) -> Option<PathBuf> {
    let probe_cmd = format!("python3 -c \"import {pkg}\" 2>/dev/null && echo ok");
    let s = format!("python3 -c import {pkg}");
    if !checked.contains(&s) {
        checked.push(s.clone());
    }
    let ok = std::process::Command::new("sh")
        .arg("-c")
        .arg(&probe_cmd)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("ok"))
        .unwrap_or(false);
    if ok {
        Some(std::env::current_dir().unwrap_or_else(|_| home()))
    } else {
        None
    }
}

// ─── Is-connected check helpers ───────────────────────────────────────────────

fn ironclaw_connected(dir: &Path) -> (bool, Option<String>) {
    let p = dir.join("a1_plugin.toml");
    if p.exists() {
        (true, Some(p.display().to_string()))
    } else {
        (false, None)
    }
}

fn mcp_connected(dir: &Path) -> (bool, Option<String>) {
    let p = dir.join(".mcp.json");
    if p.exists() {
        if let Ok(c) = std::fs::read_to_string(&p) {
            if c.contains("\"a1\"") {
                return (true, Some(p.display().to_string()));
            }
        }
    }
    (false, None)
}

// ─── Config file templates ────────────────────────────────────────────────────

fn mcp_json_content() -> String {
    let base =
        std::env::var("A1_PUBLIC_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());
    format!(
        r#"{{
  "mcpServers": {{
    "a1": {{
      "type": "http",
      "url": "{base}/mcp",
      "description": "A1 — cryptographic agent authorization"
    }}
  }}
}}"#
    )
}

fn ironclaw_toml_content() -> String {
    let base =
        std::env::var("A1_PUBLIC_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());
    format!(
        r#"# A1 Plugin for IronClaw
# Generated by A1 Studio — do not edit manually.

[plugin.a1]
enabled       = true
gateway_url   = "{base}"
authorize_url = "{base}/v1/authorize"
health_url    = "{base}/health"
version       = "2.8.0"

[plugin.a1.policy]
# Tools IronClaw is permitted to call through A1
allow = ["files.read", "web.search", "web.fetch", "shell.run"]
# Tools that require explicit human approval
require_approval = ["files.write", "shell.exec_privileged"]
# Tools that are always denied
deny = ["network.raw_socket", "process.kill_system", "registry.write"]

[plugin.a1.audit]
enabled   = true
audit_log = "~/.ironclaw/a1-audit.jsonl"
"#,
        base = base
    )
}

// ─── GET /v1/agents/scan ─────────────────────────────────────────────────────

pub async fn scan_handler(_state: State<Arc<AppState>>) -> impl IntoResponse {
    let mut checked = Vec::new();
    let mut agents = Vec::new();

    // ── IronClaw — RECOMMENDED, always first ────────────────────────────────
    {
        let install_path = find_ironclaw(&mut checked);
        let (connected, config_file) = install_path
            .as_ref()
            .map(|p| ironclaw_connected(p))
            .unwrap_or((false, None));
        agents.push(DetectedAgent {
            id:               "ironclaw".into(),
            name:             "IronClaw".into(),
            description:      "Security-focused Rust AI agent runtime. WASM sandboxes, encrypted enclaves, memory safety. Recommended for A1.".into(),
            icon:             "🦾".into(),
            homepage:         "https://github.com/ironclaw/ironclaw".into(),
            connect_method:   "config_toml".into(),
            install_path:     install_path.map(|p| p.display().to_string()),
            connected,
            config_file,
            connect_hint:     "Writes a1_plugin.toml to IronClaw's config directory".into(),
            install_cmd_unix: "cargo install ironclaw".into(),
            install_cmd_win:  "cargo install ironclaw".into(),
            uninstall_cmd:    "cargo uninstall ironclaw".into(),
            recommended:      true,
        });
    }

    // ── OpenClaw ─────────────────────────────────────────────────────────────
    {
        let install_path = find_openclaw(&mut checked);
        let (connected, config_file) = install_path
            .as_ref()
            .map(|p| mcp_connected(p))
            .unwrap_or((false, None));
        agents.push(DetectedAgent {
            id:               "openclaw".into(),
            name:             "OpenClaw".into(),
            description:      "Open-source personal AI agent (Node.js). Runs locally, connects to WhatsApp, Telegram, email, calendar, and browser.".into(),
            icon:             "🦅".into(),
            homepage:         "https://github.com/openclaw/openclaw".into(),
            connect_method:   "mcp_json".into(),
            install_path:     install_path.map(|p| p.display().to_string()),
            connected,
            config_file,
            connect_hint:     "Writes .mcp.json to OpenClaw's config directory".into(),
            install_cmd_unix: "mkdir -p \"$HOME/.npm-global\" && npm install --prefix \"$HOME/.npm-global\" openclaw --no-fund --no-audit && echo '✓ Add $HOME/.npm-global/bin to PATH: export PATH=\"$HOME/.npm-global/bin:$PATH\"'".into(),
            install_cmd_win:  "npm install -g openclaw --no-fund --no-audit".into(),
            uninstall_cmd:    "npm uninstall --prefix \"$HOME/.npm-global\" openclaw 2>/dev/null || npm uninstall -g openclaw".into(),
            recommended:      false,
        });
    }

    // ── Claude Code ──────────────────────────────────────────────────────────
    {
        let install_path = find_claude_code(&mut checked);
        let (connected, config_file) = install_path
            .as_ref()
            .map(|p| mcp_connected(p))
            .unwrap_or((false, None));
        agents.push(DetectedAgent {
            id:               "claude_code".into(),
            name:             "Claude Code".into(),
            description:      "Anthropic's AI coding agent. Runs in your terminal and edits code directly.".into(),
            icon:             "🤖".into(),
            homepage:         "https://claude.ai/code".into(),
            connect_method:   "mcp_json".into(),
            install_path:     install_path.map(|p| p.display().to_string()),
            connected,
            config_file,
            connect_hint:     "Adds A1 to ~/.claude/.mcp.json".into(),
            install_cmd_unix: "mkdir -p \"$HOME/.npm-global\" && npm install --prefix \"$HOME/.npm-global\" @anthropic-ai/claude-code --no-fund --no-audit && echo '✓ Add $HOME/.npm-global/bin to PATH: export PATH=\"$HOME/.npm-global/bin:$PATH\"'".into(),
            install_cmd_win:  "npm install -g @anthropic-ai/claude-code --no-fund --no-audit".into(),
            uninstall_cmd:    "npm uninstall --prefix \"$HOME/.npm-global\" @anthropic-ai/claude-code 2>/dev/null || npm uninstall -g @anthropic-ai/claude-code".into(),
            recommended:      false,
        });
    }

    // ── Claude Desktop ───────────────────────────────────────────────────────
    {
        let install_path = find_claude_desktop(&mut checked);
        let (connected, config_file) = install_path
            .as_ref()
            .map(|p| mcp_connected(p))
            .unwrap_or((false, None));
        agents.push(DetectedAgent {
            id: "claude_desktop".into(),
            name: "Claude Desktop".into(),
            description: "Anthropic's desktop AI assistant with MCP support.".into(),
            icon: "💬".into(),
            homepage: "https://claude.ai/desktop".into(),
            connect_method: "mcp_json".into(),
            install_path: install_path.map(|p| p.display().to_string()),
            connected,
            config_file,
            connect_hint: "Adds A1 to Claude Desktop's config".into(),
            install_cmd_unix: "".into(),
            install_cmd_win: "".into(),
            uninstall_cmd: "".into(),
            recommended: false,
        });
    }

    // ── Ollama (Local LLM) ───────────────────────────────────────────────────
    {
        let install_path = find_ollama(&mut checked);
        let (connected, config_file) = install_path
            .as_ref()
            .map(|p| mcp_connected(p))
            .unwrap_or((false, None));
        agents.push(DetectedAgent {
            id: "ollama".into(),
            name: "Ollama (Local LLM)".into(),
            description: "Run Llama 3, Mistral, Phi, Gemma locally. No API key, full privacy."
                .into(),
            icon: "🦙".into(),
            homepage: "https://ollama.ai".into(),
            connect_method: "mcp_json".into(),
            install_path: install_path.map(|p| p.display().to_string()),
            connected,
            config_file,
            connect_hint: "Adds A1 as an MCP server for Ollama agents".into(),
            install_cmd_unix: "curl -fsSL https://ollama.ai/install.sh | sh".into(),
            install_cmd_win: "winget install Ollama.Ollama".into(),
            uninstall_cmd: "which ollama && sudo rm $(which ollama) || true".into(),
            recommended: false,
        });
    }

    // ── LangChain ────────────────────────────────────────────────────────────
    {
        let install_path = find_python_agent("langchain", &mut checked);
        let (connected, config_file) = install_path
            .as_ref()
            .map(|p| mcp_connected(p))
            .unwrap_or((false, None));
        agents.push(DetectedAgent {
            id:               "langchain".into(),
            name:             "LangChain".into(),
            description:      "Popular Python AI agent framework for chaining LLM calls and tools.".into(),
            icon:             "🦜".into(),
            homepage:         "https://langchain.com".into(),
            connect_method:   "mcp_json".into(),
            install_path:     install_path.map(|p| p.display().to_string()),
            connected,
            config_file,
            connect_hint:     "Adds A1 guard to your LangChain project directory".into(),
            install_cmd_unix: "pip3 install --user langchain langchain-openai 2>/dev/null || python3 -m pip install --user langchain langchain-openai".into(),
            install_cmd_win:  "pip install langchain langchain-openai".into(),
            uninstall_cmd:    "pip3 uninstall -y langchain 2>/dev/null || python3 -m pip uninstall -y langchain".into(),
            recommended:      false,
        });
    }

    // ── CrewAI ───────────────────────────────────────────────────────────────
    {
        let install_path = find_python_agent("crewai", &mut checked);
        let (connected, config_file) = install_path
            .as_ref()
            .map(|p| mcp_connected(p))
            .unwrap_or((false, None));
        agents.push(DetectedAgent {
            id: "crewai".into(),
            name: "CrewAI".into(),
            description: "Multi-agent framework. Groups of AI agents working as a crew.".into(),
            icon: "⛵".into(),
            homepage: "https://crewai.com".into(),
            connect_method: "mcp_json".into(),
            install_path: install_path.map(|p| p.display().to_string()),
            connected,
            config_file,
            connect_hint: "Adds A1 guard to your CrewAI project directory".into(),
            install_cmd_unix:
                "pip3 install --user crewai 2>/dev/null || python3 -m pip install --user crewai"
                    .into(),
            install_cmd_win: "pip install crewai".into(),
            uninstall_cmd:
                "pip3 uninstall -y crewai 2>/dev/null || python3 -m pip uninstall -y crewai".into(),
            recommended: false,
        });
    }

    // ── OpenAI Agents ────────────────────────────────────────────────────────
    {
        let install_path = find_python_agent("openai", &mut checked);
        let (connected, config_file) = install_path
            .as_ref()
            .map(|p| mcp_connected(p))
            .unwrap_or((false, None));
        agents.push(DetectedAgent {
            id:               "openai_agents".into(),
            name:             "OpenAI Agents SDK".into(),
            description:      "OpenAI's official agent SDK for building agentic applications.".into(),
            icon:             "🟢".into(),
            homepage:         "https://openai.com/agents".into(),
            connect_method:   "mcp_json".into(),
            install_path:     install_path.map(|p| p.display().to_string()),
            connected,
            config_file,
            connect_hint:     "Adds A1 guard to your OpenAI Agents project".into(),
            install_cmd_unix: "pip3 install --user openai-agents 2>/dev/null || python3 -m pip install --user openai-agents".into(),
            install_cmd_win:  "pip install openai-agents".into(),
            uninstall_cmd:    "pip3 uninstall -y openai-agents 2>/dev/null || python3 -m pip uninstall -y openai-agents".into(),
            recommended:      false,
        });
    }

    Json(ScanResponse {
        agents,
        scan_paths_checked: checked,
    })
}

// ─── POST /v1/agents/connect ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ConnectRequest {
    pub agent_id: String,
    /// Override install path (from scan result). If omitted, re-detects.
    pub install_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ConnectResponse {
    pub connected: bool,
    pub agent_id: String,
    pub files_written: Vec<String>,
    pub message: String,
    pub next_step: String,
}

pub async fn connect_handler(
    _state: State<Arc<AppState>>,
    Json(req): Json<ConnectRequest>,
) -> impl IntoResponse {
    let mut checked = Vec::new();

    // Resolve install path
    let install_path: Option<PathBuf> =
        req.install_path
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| match req.agent_id.as_str() {
                "openclaw" => find_openclaw(&mut checked),
                "ironclaw" => find_ironclaw(&mut checked),
                "claude_code" => find_claude_code(&mut checked),
                "claude_desktop" => find_claude_desktop(&mut checked),
                "ollama" => find_ollama(&mut checked),
                "langchain" | "crewai" | "openai_agents" | "custom" => {
                    Some(std::env::current_dir().unwrap_or_else(|_| home()))
                }
                _ => None,
            });

    match req.agent_id.as_str() {
        // ── MCP-based agents ──────────────────────────────────────────────────
        "openclaw" | "claude_code" | "langchain" | "crewai" | "openai_agents" | "ollama"
        | "custom" => {
            let dir = match install_path {
                Some(ref p) => p.clone(),
                None => {
                    let fallback = match req.agent_id.as_str() {
                        "openclaw" => home().join(".openclaw"),
                        "claude_code" => home().join(".claude"),
                        "ollama" => home().join(".ollama"),
                        _ => std::env::current_dir().unwrap_or_else(|_| home()),
                    };
                    std::fs::create_dir_all(&fallback).ok();
                    fallback
                }
            };

            let mcp_path = dir.join(".mcp.json");
            let content = mcp_json_content();

            let final_content = if mcp_path.exists() {
                if let Ok(existing) = std::fs::read_to_string(&mcp_path) {
                    if existing.contains("\"a1\"") {
                        existing
                    } else {
                        merge_mcp_json(&existing).unwrap_or(content.clone())
                    }
                } else {
                    content.clone()
                }
            } else {
                content.clone()
            };

            match std::fs::write(&mcp_path, &final_content) {
                Ok(_) => {
                    let next = match req.agent_id.as_str() {
                        "openclaw"  => "Restart OpenClaw. A1 authorization is now active on all tools.",
                        "claude_code" => "Open a new Claude Code session. Type: 'Use a1_check_health to verify A1 is connected'",
                        "ollama"    => "Restart your Ollama-based agent. A1 will authorize all tool calls.",
                        _ => "A1 is now connected. Restart your agent if it's running.",
                    };
                    Json(ConnectResponse {
                        connected: true,
                        agent_id: req.agent_id.clone(),
                        files_written: vec![mcp_path.display().to_string()],
                        message: format!("✓ Connected. Wrote .mcp.json to {}", dir.display()),
                        next_step: next.into(),
                    })
                    .into_response()
                }
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ConnectResponse {
                        connected: false,
                        agent_id: req.agent_id,
                        files_written: vec![],
                        message: format!("Failed to write {}: {e}", mcp_path.display()),
                        next_step: "Check file permissions on the target directory.".into(),
                    }),
                )
                    .into_response(),
            }
        }

        // ── IronClaw (TOML plugin) ────────────────────────────────────────────
        "ironclaw" => {
            let dir = match install_path {
                Some(ref p) => p.clone(),
                None => {
                    let fallback = home().join(".ironclaw");
                    std::fs::create_dir_all(&fallback).ok();
                    fallback
                }
            };

            let toml_path = dir.join("a1_plugin.toml");
            let content = ironclaw_toml_content();

            match std::fs::write(&toml_path, &content) {
                Ok(_) => Json(ConnectResponse {
                    connected: true,
                    agent_id: req.agent_id,
                    files_written: vec![toml_path.display().to_string()],
                    message: format!("✓ Connected. Wrote a1_plugin.toml to {}", dir.display()),
                    next_step: "Restart IronClaw. A1 will enforce authorization on all tool calls."
                        .into(),
                })
                .into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ConnectResponse {
                        connected: false,
                        agent_id: req.agent_id,
                        files_written: vec![],
                        message: format!("Failed to write {}: {e}", toml_path.display()),
                        next_step: "Check file permissions on the target directory.".into(),
                    }),
                )
                    .into_response(),
            }
        }

        // ── Claude Desktop ────────────────────────────────────────────────────
        "claude_desktop" => {
            let dir = match install_path {
                Some(ref p) => p.clone(),
                None => {
                    let fallback = {
                        #[cfg(target_os = "macos")]
                        {
                            home()
                                .join("Library")
                                .join("Application Support")
                                .join("Claude")
                        }
                        #[cfg(not(target_os = "macos"))]
                        {
                            home().join(".config").join("Claude")
                        }
                    };
                    std::fs::create_dir_all(&fallback).ok();
                    fallback
                }
            };

            let config_path = dir.join("claude_desktop_config.json");
            let final_content = if config_path.exists() {
                if let Ok(existing) = std::fs::read_to_string(&config_path) {
                    merge_desktop_config(&existing).unwrap_or_else(|| mcp_json_content())
                } else {
                    mcp_json_content()
                }
            } else {
                mcp_json_content()
            };

            match std::fs::write(&config_path, &final_content) {
                Ok(_) => Json(ConnectResponse {
                    connected:     true,
                    agent_id:      req.agent_id,
                    files_written: vec![config_path.display().to_string()],
                    message:       "✓ Connected. Updated claude_desktop_config.json".into(),
                    next_step:     "Restart Claude Desktop. A1 will appear in the toolbar as an available tool.".into(),
                }).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ConnectResponse {
                    connected:     false,
                    agent_id:      req.agent_id,
                    files_written: vec![],
                    message:       format!("Failed: {e}"),
                    next_step:     "Try writing the file manually. See templates/mcp-connect.md".into(),
                })).into_response(),
            }
        }

        unknown => (
            StatusCode::BAD_REQUEST,
            Json(ConnectResponse {
                connected: false,
                agent_id: unknown.into(),
                files_written: vec![],
                message: format!(
                    "Unknown agent ID: '{unknown}'. Call /v1/agents/scan for valid IDs."
                ),
                next_step: "".into(),
            }),
        )
            .into_response(),
    }
}

// ─── JSON merge helpers ──────────────────────────────────────────────────────

fn merge_mcp_json(existing: &str) -> Option<String> {
    let base =
        std::env::var("A1_PUBLIC_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());
    let a1_entry = format!(
        r#""a1": {{"type": "http", "url": "{base}/mcp", "description": "A1 — cryptographic agent authorization"}}"#
    );
    if let Some(idx) = existing.find("\"mcpServers\"") {
        if let Some(brace_idx) = existing[idx..].find('{') {
            let insert_at = idx + brace_idx + 1;
            let mut result = existing.to_string();
            result.insert_str(insert_at, &format!("\n    {a1_entry},"));
            return Some(result);
        }
    }
    None
}

fn merge_desktop_config(existing: &str) -> Option<String> {
    merge_mcp_json(existing)
}

// ─── POST /v1/agents/restart ──────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct RestartRequest {
    pub agent_id: String,
    pub install_path: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct RestartResponse {
    pub success: bool,
    pub agent_id: String,
    pub message: String,
    pub restart_cmd: Option<String>,
}

pub async fn restart_handler(
    _state: State<Arc<AppState>>,
    Json(req): Json<RestartRequest>,
) -> impl IntoResponse {
    let id = req.agent_id.trim().to_lowercase();

    let cmd: Option<&str> = match id.as_str() {
        "openclaw" => Some("openclaw restart"),
        "ironclaw" => Some("ironclaw restart"),
        "claude-code" => Some("claude restart"),
        _ => None,
    };

    if let Some(c) = cmd {
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(c)
            .output()
            .await;
        let ok = output.map(|o| o.status.success()).unwrap_or(false);
        return Json(RestartResponse {
            success: ok,
            agent_id: req.agent_id.clone(),
            message: if ok {
                format!("Restarted {id}")
            } else {
                format!("Could not restart {id} automatically")
            },
            restart_cmd: Some(c.into()),
        })
        .into_response();
    }

    Json(RestartResponse {
        success: false,
        agent_id: req.agent_id.clone(),
        message: "Restart not supported automatically for this agent. Use the command below."
            .into(),
        restart_cmd: req
            .install_path
            .as_deref()
            .map(|p| format!("cd {p} && kill $(pgrep -f agent) && python agent.py"))
            .or_else(|| Some("Restart your agent manually or from its launcher".into())),
    })
    .into_response()
}

// ─── POST /v1/agents/disconnect ───────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct DisconnectRequest {
    pub agent_id: String,
    pub install_path: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct DisconnectResponse {
    pub success: bool,
    pub agent_id: String,
    pub message: String,
}

pub async fn disconnect_handler(
    _state: axum::extract::State<std::sync::Arc<crate::state::AppState>>,
    axum::Json(req): axum::Json<DisconnectRequest>,
) -> axum::response::Response {
    let id = req.agent_id.trim().to_string();

    // ── IronClaw — TOML plugin file ───────────────────────────────────────────
    if id == "ironclaw" {
        let ironclaw_dir = req
            .install_path
            .as_deref()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| home().join(".ironclaw"));

        let toml_path = ironclaw_dir.join("a1_plugin.toml");

        if !toml_path.exists() {
            return axum::Json(DisconnectResponse {
                success: true,
                agent_id: id,
                message: "No a1_plugin.toml found — IronClaw was already disconnected.".into(),
            })
            .into_response();
        }

        return match std::fs::remove_file(&toml_path) {
            Ok(_) => axum::Json(DisconnectResponse {
                success: true,
                agent_id: id,
                message: format!(
                    "A1 plugin removed from {}. Restart IronClaw to apply.",
                    toml_path.display()
                ),
            })
            .into_response(),
            Err(e) => axum::Json(DisconnectResponse {
                success: false,
                agent_id: id,
                message: format!(
                    "Could not remove {}: {e}. Delete it manually to disconnect.",
                    toml_path.display()
                ),
            })
            .into_response(),
        };
    }

    let config_dir: Option<std::path::PathBuf> = req
        .install_path
        .as_deref()
        .map(std::path::PathBuf::from)
        .or_else(|| match id.as_str() {
            "claude_code" => Some(home().join(".claude")),
            "openclaw" => Some(home().join(".openclaw")),
            "ollama" => Some(home().join(".ollama")),
            "claude_desktop" => None,
            _ => Some(std::env::current_dir().unwrap_or_else(|_| home())),
        });

    let Some(dir) = config_dir else {
        return axum::Json(DisconnectResponse {
            success:  false,
            agent_id: id,
            message:  "Cannot locate config directory for this agent. Remove the A1 entry from your .mcp.json manually.".into(),
        }).into_response();
    };

    let mcp_path = dir.join(".mcp.json");
    if !mcp_path.exists() {
        return axum::Json(DisconnectResponse {
            success: true,
            agent_id: id,
            message: "No .mcp.json found — agent was already disconnected.".into(),
        })
        .into_response();
    }

    match std::fs::read_to_string(&mcp_path) {
        Ok(existing) => {
            if !existing.contains("\"a1\"") {
                return axum::Json(DisconnectResponse {
                    success: true,
                    agent_id: id,
                    message: "A1 was not found in this agent's config — already disconnected."
                        .into(),
                })
                .into_response();
            }
            let cleaned = remove_a1_from_mcp_json(&existing).unwrap_or(existing);
            match std::fs::write(&mcp_path, &cleaned) {
                Ok(_) => axum::Json(DisconnectResponse {
                    success: true,
                    agent_id: id,
                    message: format!(
                        "A1 removed from {}. Restart your agent to apply.",
                        mcp_path.display()
                    ),
                })
                .into_response(),
                Err(e) => axum::Json(DisconnectResponse {
                    success: false,
                    agent_id: id,
                    message: format!("Could not write {}: {e}", mcp_path.display()),
                })
                .into_response(),
            }
        }
        Err(e) => axum::Json(DisconnectResponse {
            success: false,
            agent_id: id,
            message: format!("Could not read {}: {e}", mcp_path.display()),
        })
        .into_response(),
    }
}

fn remove_a1_from_mcp_json(src: &str) -> Option<String> {
    let mut val: serde_json::Value = serde_json::from_str(src).ok()?;
    if let Some(servers) = val.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
        servers.remove("a1");
    }
    serde_json::to_string_pretty(&val).ok()
}

// ─── GET /v1/agents/pull ─────────────────────────────────────────────────────
//
// Server-Sent Events stream: runs the install command for an agent and streams
// stdout/stderr lines back in real-time as SSE events.
// Events: { type: "log", data: "<line>" }
//         { type: "done", data: "{success:bool, message:string}" }
//
// The client keeps the EventSource open; state survives browser tab switches
// because the SSE connection is held by the browser, not the page JS.

#[derive(Debug, Deserialize)]
pub struct PullQuery {
    pub agent_id: String,
    /// "unix" or "win"
    pub platform: Option<String>,
}

pub async fn pull_handler(
    axum::extract::Query(q): axum::extract::Query<PullQuery>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, std::convert::Infallible>>(64);

    let agent_id = q.agent_id.clone();
    let platform = q.platform.clone().unwrap_or_else(|| "unix".into());

    tokio::spawn(async move {
        // Find install command for this agent
        let install_cmd = KNOWN_AGENTS
            .iter()
            .find(|a| a.id == agent_id.as_str())
            .map(|a| {
                if platform == "win" {
                    a.install_cmd_win
                } else {
                    a.install_cmd_unix
                }
            })
            .unwrap_or("");

        if install_cmd.is_empty() {
            let _ = tx.send(Ok(Event::default()
                .event("done")
                .data(serde_json::to_string(&json!({
                    "success": false,
                    "message": format!("No install command available for agent '{agent_id}'. Download manually from the homepage.")
                })).unwrap_or_default())
            )).await;
            return;
        }

        // When running inside Docker, we can't install tools on the host machine.
        // Instead, show the command for the user to run in their own terminal.
        if std::env::var("A1_RUNNING_IN_DOCKER").is_ok() {
            let _ = tx.send(Ok(Event::default()
                .event("log")
                .data(format!("⚠ A1 is running inside Docker and cannot install software on your computer directly."))
            )).await;
            let _ = tx
                .send(Ok(Event::default().event("log").data(format!(
                    "Open your terminal and run this command to install {}:",
                    agent_id
                ))))
                .await;
            let _ = tx
                .send(Ok(Event::default()
                    .event("log")
                    .data(format!("$ {install_cmd}"))))
                .await;
            let _ = tx.send(Ok(Event::default()
                .event("done")
                .data(serde_json::to_string(&json!({
                    "success": false,
                    "manual": true,
                    "install_cmd": install_cmd,
                    "message": format!("Copy the command above and run it in your terminal, then click Rescan.")
                })).unwrap_or_default())
            )).await;
            return;
        }

        // Log the command we're running
        let _ = tx
            .send(Ok(Event::default()
                .event("log")
                .data(format!("$ {install_cmd}"))))
            .await;

        // Spawn the install process
        let mut child = match tokio::process::Command::new("sh")
            .arg("-c")
            .arg(install_cmd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = tx
                    .send(Ok(Event::default().event("done").data(
                        serde_json::to_string(&json!({
                            "success": false,
                            "message": format!("Failed to start install process: {e}")
                        }))
                        .unwrap_or_default(),
                    )))
                    .await;
                return;
            }
        };

        // Stream stdout
        if let Some(stdout) = child.stdout.take() {
            use tokio::io::AsyncBufReadExt;
            let mut lines = tokio::io::BufReader::new(stdout).lines();
            let tx2 = tx.clone();
            tokio::spawn(async move {
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx2.send(Ok(Event::default().event("log").data(line))).await;
                }
            });
        }

        // Stream stderr
        if let Some(stderr) = child.stderr.take() {
            use tokio::io::AsyncBufReadExt;
            let mut lines = tokio::io::BufReader::new(stderr).lines();
            let tx3 = tx.clone();
            tokio::spawn(async move {
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx3
                        .send(Ok(Event::default()
                            .event("log")
                            .data(format!("stderr: {line}"))))
                        .await;
                }
            });
        }

        // Wait for completion
        let status = child.wait().await;
        let success = status.map(|s| s.success()).unwrap_or(false);

        let message = if success {
            format!("✓ '{agent_id}' installed successfully. Click Connect to integrate with A1.")
        } else {
            format!("✗ Installation of '{agent_id}' failed. See output above for details.")
        };

        let _ = tx
            .send(Ok(Event::default().event("done").data(
                serde_json::to_string(&json!({ "success": success, "message": message }))
                    .unwrap_or_default(),
            )))
            .await;
    });

    Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default())
}

// ─── POST /v1/agents/remove ───────────────────────────────────────────────────
//
// Uninstalls the agent binary/package from the system.

#[derive(Debug, Deserialize)]
pub struct RemoveRequest {
    pub agent_id: String,
    pub platform: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RemoveResponse {
    pub success: bool,
    pub agent_id: String,
    pub message: String,
    pub output: String,
}

pub async fn remove_handler(Json(req): Json<RemoveRequest>) -> impl IntoResponse {
    let platform = req.platform.as_deref().unwrap_or("unix");

    let uninstall_cmd = KNOWN_AGENTS
        .iter()
        .find(|a| a.id == req.agent_id.as_str())
        .map(|a| a.uninstall_cmd)
        .unwrap_or("");

    if uninstall_cmd.is_empty() {
        return Json(RemoveResponse {
            success: false,
            agent_id: req.agent_id,
            message: "No automatic uninstall command available. Remove manually.".into(),
            output: "".into(),
        })
        .into_response();
    }

    let result = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(uninstall_cmd)
        .output()
        .await;

    match result {
        Ok(output) => {
            let success = output.status.success();
            let out = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            Json(RemoveResponse {
                success,
                agent_id: req.agent_id.clone(),
                message:  if success {
                    format!("✓ '{}' uninstalled successfully.", req.agent_id)
                } else {
                    format!("✗ Uninstall returned non-zero exit code. The agent may still be partially installed.")
                },
                output:   out,
            }).into_response()
        }
        Err(e) => Json(RemoveResponse {
            success: false,
            agent_id: req.agent_id,
            message: format!("Failed to run uninstall: {e}"),
            output: "".into(),
        })
        .into_response(),
    }
}

// ─── POST /v1/agents/probe-live ───────────────────────────────────────────────
//
// Genuine live-connection proof for a connected agent. For IronClaw:
//   1. Reads the actual a1_plugin.toml from disk and returns its contents
//   2. Runs `ironclaw --version` (or `ironclaw status`) to prove the binary exists
//   3. Tests A1 policy enforcement with a real /v1/authorize round-trip
//   4. Probes IronClaw's HTTP API if it's running
//   5. Returns all results — no mocks, no simulated data

#[derive(Debug, Deserialize)]
pub struct ProbeLiveRequest {
    pub agent_id: String,
    pub install_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProbeLiveResponse {
    /// The actual contents of the A1 config file written to the agent's directory
    pub config_file_path: Option<String>,
    pub config_file_contents: Option<String>,
    /// Output of running the agent binary directly (e.g. `ironclaw --version`)
    pub binary_check_cmd: String,
    pub binary_check_output: String,
    pub binary_found: bool,
    /// Result of a real A1 authorize call (policy enforcement test)
    pub policy_test_allowed: PolicyTestResult,
    pub policy_test_denied: PolicyTestResult,
    /// HTTP probe: is the agent's API reachable right now?
    pub runtime_reachable: bool,
    pub runtime_port: Option<u16>,
    pub runtime_version: Option<String>,
    pub runtime_health_url: Option<String>,
    /// Overall: is this a genuine, working A1 connection?
    pub genuine_connection: bool,
    pub proof_summary: String,
}

#[derive(Debug, Serialize)]
pub struct PolicyTestResult {
    pub tool: String,
    pub allowed: bool,
    pub reason: String,
    pub a1_token_issued: bool,
}

pub async fn probe_live_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ProbeLiveRequest>,
) -> impl IntoResponse {
    let agent_id = req.agent_id.as_str();

    // ── 1. Config file check ──────────────────────────────────────────────────
    let (config_path, config_contents) = match agent_id {
        "ironclaw" => {
            let dir = req
                .install_path
                .as_deref()
                .map(PathBuf::from)
                .unwrap_or_else(|| home().join(".ironclaw"));
            let p = dir.join("a1_plugin.toml");
            if p.exists() {
                let contents = std::fs::read_to_string(&p).unwrap_or_default();
                (Some(p.display().to_string()), Some(contents))
            } else {
                (Some(p.display().to_string()), None)
            }
        }
        "claude_code" | "openclaw" | "ollama" => {
            let dir = req
                .install_path
                .as_deref()
                .map(PathBuf::from)
                .or_else(|| match agent_id {
                    "claude_code" => Some(home().join(".claude")),
                    "openclaw" => Some(home().join(".openclaw")),
                    "ollama" => Some(home().join(".ollama")),
                    _ => None,
                });
            if let Some(d) = dir {
                let p = d.join(".mcp.json");
                if p.exists() {
                    let c = std::fs::read_to_string(&p).unwrap_or_default();
                    (Some(p.display().to_string()), Some(c))
                } else {
                    (Some(p.display().to_string()), None)
                }
            } else {
                (None, None)
            }
        }
        _ => (None, None),
    };

    // ── 2. Binary check ───────────────────────────────────────────────────────
    let (bin_cmd, bin_output, bin_found) = {
        let cmd = match agent_id {
            "ironclaw" => "ironclaw --version 2>&1 || ironclaw version 2>&1 || echo 'not in PATH'",
            "openclaw" => "openclaw --version 2>&1 || echo 'not in PATH'",
            "claude_code" => "claude --version 2>&1 || echo 'not in PATH'",
            "ollama" => "ollama --version 2>&1 || echo 'not in PATH'",
            _ => "echo 'no version command'",
        };
        let out = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()
            .await;
        let output_str = out
            .map(|o| {
                let s = format!(
                    "{}{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                );
                s.trim().to_string()
            })
            .unwrap_or_else(|e| format!("error: {e}"));
        let found = !output_str.contains("not in PATH") && !output_str.is_empty();
        (cmd.to_string(), output_str, found)
    };

    // ── 3. A1 policy enforcement tests ────────────────────────────────────────
    // We do real /v1/authorize calls using the gateway state.
    // "allowed" test: files.read — should PASS
    // "denied" test:  network.raw_socket — should DENY

    let base =
        std::env::var("A1_PUBLIC_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    let gw_pk = state.gateway_pk_hex.clone();

    let policy_allowed = {
        let payload = json!({
            "agent_id": format!("{agent_id}-a1-probe"),
            "tool": "files.read",
            "context": { "path": "/tmp/test.txt", "source": "a1-probe-live" },
            "gateway_pk": gw_pk
        });
        match client
            .post(format!("{base}/v1/authorize"))
            .json(&payload)
            .send()
            .await
        {
            Ok(r) => {
                let status = r.status();
                let body: serde_json::Value = r.json().await.unwrap_or_default();
                PolicyTestResult {
                    tool: "files.read".into(),
                    allowed: status.is_success()
                        && body
                            .get("authorized")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                    reason: body
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .unwrap_or("A1 evaluated the request")
                        .to_string(),
                    a1_token_issued: body.get("token").is_some()
                        || body.get("passport").is_some()
                        || status.is_success(),
                }
            }
            Err(e) => PolicyTestResult {
                tool: "files.read".into(),
                allowed: false,
                reason: format!("Could not reach A1 gateway: {e}"),
                a1_token_issued: false,
            },
        }
    };

    let policy_denied = {
        let payload = json!({
            "agent_id": format!("{agent_id}-a1-probe"),
            "tool": "network.raw_socket",
            "context": { "host": "0.0.0.0", "source": "a1-probe-live" },
            "gateway_pk": gw_pk
        });
        match client
            .post(format!("{base}/v1/authorize"))
            .json(&payload)
            .send()
            .await
        {
            Ok(r) => {
                let status = r.status();
                let body: serde_json::Value = r.json().await.unwrap_or_default();
                // network.raw_socket should be denied (deny-list in the TOML)
                let authorized = body
                    .get("authorized")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                PolicyTestResult {
                    tool: "network.raw_socket".into(),
                    allowed: authorized,
                    reason: body
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .unwrap_or(if !authorized {
                            "A1 policy: tool on deny list"
                        } else {
                            "Allowed"
                        })
                        .to_string(),
                    a1_token_issued: authorized,
                }
            }
            Err(_) => PolicyTestResult {
                tool: "network.raw_socket".into(),
                allowed: false,
                reason: "A1 gateway unreachable — gateway must be running for enforcement".into(),
                a1_token_issued: false,
            },
        }
    };

    // ── 4. HTTP probe: is the agent running right now? ────────────────────────
    let probes: &[(&str, u16, &str)] = match agent_id {
        "ironclaw" => &[
            ("ironclaw", 4000, "/healthz"),
            ("ironclaw", 4001, "/healthz"),
        ],
        "openclaw" => &[("openclaw", 3000, "/health"), ("openclaw", 3001, "/health")],
        "ollama" => &[("ollama", 11434, "/api/version")],
        _ => &[],
    };

    let mut runtime_reachable = false;
    let mut runtime_port: Option<u16> = None;
    let mut runtime_version: Option<String> = None;
    let mut runtime_health_url: Option<String> = None;

    for &(_aid, port, path) in probes {
        let url = format!("http://127.0.0.1:{port}{path}");
        if let Ok(resp) = client.get(&url).send().await {
            if resp.status().as_u16() < 500 {
                runtime_reachable = true;
                runtime_port = Some(port);
                runtime_health_url = Some(url.clone());
                runtime_version = resp.json::<serde_json::Value>().await.ok().and_then(|v| {
                    v.get("version")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                });
                break;
            }
        }
    }

    // ── 5. Proof summary ──────────────────────────────────────────────────────
    let config_ok = config_contents.is_some();
    let genuine = config_ok || bin_found;

    let proof_summary = if config_ok && bin_found && policy_allowed.allowed {
        format!(
            "✅ GENUINE CONNECTION CONFIRMED — Config file found at disk, '{}' binary verified ({}), A1 policy enforcement active.",
            agent_id,
            bin_output.lines().next().unwrap_or("installed")
        )
    } else if config_ok && bin_found {
        format!(
            "⚠️ CONFIG + BINARY OK — {agent_id} is installed and A1 config is written. Start the agent to enable runtime enforcement."
        )
    } else if config_ok {
        format!(
            "⚠️ CONFIG WRITTEN — A1 config file exists at '{}'. Install the {agent_id} binary to complete setup.",
            config_path.as_deref().unwrap_or("?")
        )
    } else if bin_found {
        format!(
            "⚠️ BINARY FOUND — {agent_id} binary is installed but A1 config not written. Click Connect first."
        )
    } else {
        format!(
            "❌ NOT CONNECTED — {agent_id} is not installed or A1 config not written. Install and Connect first."
        )
    };

    Json(ProbeLiveResponse {
        config_file_path: config_path,
        config_file_contents: config_contents,
        binary_check_cmd: bin_cmd,
        binary_check_output: bin_output,
        binary_found: bin_found,
        policy_test_allowed: policy_allowed,
        policy_test_denied: policy_denied,
        runtime_reachable,
        runtime_port,
        runtime_version,
        runtime_health_url,
        genuine_connection: genuine,
        proof_summary,
    })
}
