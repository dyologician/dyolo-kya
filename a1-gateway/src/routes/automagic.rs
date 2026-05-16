use axum::{extract::State, response::Json};
use serde::Serialize;
use std::process::Command;
use std::path::PathBuf;
use std::sync::Arc;
use std::env;
use crate::state::AppState;

#[derive(Serialize)]
pub struct AutomagicStatus {
    pub autostart_enabled: bool,
    pub docker_installed:  bool,
    pub automagic_mode:    bool,
    pub platform:          String,
}

#[derive(Serialize)]
pub struct SimpleResult {
    pub success: bool,
    pub message: String,
}

#[derive(Serialize)]
pub struct InstallStarted {
    pub started: bool,
    pub message: String,
}

pub async fn get_status(State(_s): State<Arc<AppState>>) -> Json<AutomagicStatus> {
    Json(AutomagicStatus {
        autostart_enabled: autostart_is_enabled(),
        docker_installed:  docker_is_present(),
        automagic_mode:    env::var("AUTOMAGIC_MODE").as_deref() == Ok("1"),
        platform:          platform_name(),
    })
}

pub async fn enable_autostart(State(_s): State<Arc<AppState>>) -> Json<SimpleResult> {
    match try_enable_autostart() {
        Ok(msg)  => Json(SimpleResult { success: true,  message: msg }),
        Err(msg) => Json(SimpleResult { success: false, message: msg }),
    }
}

pub async fn install_docker(State(_s): State<Arc<AppState>>) -> Json<InstallStarted> {
    tokio::spawn(async { run_docker_install().await });
    Json(InstallStarted {
        started: true,
        message: "Docker Desktop installation started. This takes 2-3 minutes.".into(),
    })
}

fn try_enable_autostart() -> Result<String, String> {
    let bin = binary_path();

    #[cfg(target_os = "macos")]
    {
        let home   = dirs_home().ok_or("Cannot find home directory")?;
        let agents = home.join("Library/LaunchAgents");
        std::fs::create_dir_all(&agents).ok();
        let plist_path = agents.join("com.a1.gateway.plist");

        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>             <string>com.a1.gateway</string>
  <key>ProgramArguments</key> <array><string>{bin}</string></array>
  <key>RunAtLoad</key>         <true/>
  <key>KeepAlive</key>         <true/>
  <key>StandardOutPath</key>   <string>{home}/.a1/gateway.log</string>
  <key>StandardErrorPath</key> <string>{home}/.a1/gateway.log</string>
</dict>
</plist>"#,
            bin  = bin,
            home = home.display()
        );

        std::fs::write(&plist_path, plist)
            .map_err(|e| format!("Could not write plist: {e}"))?;
        let _ = Command::new("launchctl").args(["unload", &plist_path.to_string_lossy()]).output();
        Command::new("launchctl")
            .args(["load", &plist_path.to_string_lossy()])
            .output()
            .map_err(|e| format!("launchctl load failed: {e}"))?;
        mark_autostart_done();
        return Ok("Auto-start enabled via launchd.".into());
    }

    #[cfg(target_os = "linux")]
    {
        let home    = dirs_home().ok_or("Cannot find home directory")?;
        let svc_dir = home.join(".config/systemd/user");
        std::fs::create_dir_all(&svc_dir).ok();
        let svc_path = svc_dir.join("a1-gateway.service");

        let svc = format!(
            "[Unit]\nDescription=A1 Gateway\nAfter=network.target\n\n\
             [Service]\nExecStart={bin}\nRestart=always\n\n\
             [Install]\nWantedBy=default.target\n",
            bin = bin
        );
        std::fs::write(&svc_path, svc).map_err(|e| format!("Cannot write service: {e}"))?;
        let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).output();
        Command::new("systemctl")
            .args(["--user", "enable", "--now", "a1-gateway"])
            .output()
            .map_err(|e| format!("systemctl enable failed: {e}"))?;
        mark_autostart_done();
        return Ok("Auto-start enabled via systemd user service.".into());
    }

    #[cfg(target_os = "windows")]
    {
        let bin_w = bin.replace('/', "\\");
        let out   = Command::new("schtasks")
            .args(["/Create", "/F", "/SC", "ONLOGON", "/TN", "A1Gateway",
                   "/TR", &format!("\"{}\"", bin_w), "/RL", "HIGHEST"])
            .output()
            .map_err(|e| format!("schtasks failed: {e}"))?;

        if out.status.success() {
            mark_autostart_done();
            Ok("Auto-start enabled via Windows Task Scheduler.".into())
        } else {
            Err(format!("schtasks error: {}", String::from_utf8_lossy(&out.stderr)))
        }
    }

    #[allow(unreachable_code)]
    Err("Auto-start not supported on this platform.".into())
}

async fn run_docker_install() {
    #[cfg(target_os = "macos")]
    {
        use std::process::Stdio;
        let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "amd64" };
        let url  = format!("https://desktop.docker.com/mac/main/{arch}/Docker.dmg");
        let tmp  = "/tmp/Docker-A1-Install.dmg";
        let _ = Command::new("curl").args(["-fL", "--progress-bar", &url, "-o", tmp])
            .stdout(Stdio::null()).stderr(Stdio::null()).status();
        let _ = Command::new("hdiutil").args(["attach", tmp, "-quiet", "-nobrowse"]).status();
        let _ = Command::new("cp").args(["-R", "/Volumes/Docker/Docker.app", "/Applications/"]).status();
        let _ = Command::new("hdiutil").args(["detach", "/Volumes/Docker", "-quiet"]).status();
        let _ = std::fs::remove_file(tmp);
        let _ = Command::new("open").args(["-a", "Docker"]).status();
    }

    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("sh").args(["-c", "curl -fsSL https://get.docker.com | sh"]).status();
        let _ = Command::new("sudo")
            .args(["usermod", "-aG", "docker", &env::var("USER").unwrap_or_default()])
            .status();
    }
}

fn binary_path() -> String {
    dirs_home().unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".a1/bin/a1-gateway")
        .to_string_lossy()
        .to_string()
}

fn dirs_home() -> Option<PathBuf> {
    env::var("HOME").ok().map(PathBuf::from)
        .or_else(|| env::var("USERPROFILE").ok().map(PathBuf::from))
}

fn docker_is_present() -> bool {
    Command::new("docker").arg("info").output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn autostart_is_enabled() -> bool {
    dirs_home().unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".a1/autostart-enabled")
        .exists()
}

fn mark_autostart_done() {
    let home = dirs_home().unwrap_or_else(|| PathBuf::from("/tmp"));
    let _ = std::fs::create_dir_all(home.join(".a1"));
    let _ = std::fs::write(home.join(".a1/autostart-enabled"), b"1");
}

fn platform_name() -> String {
    #[cfg(target_os = "macos")]   { return "macos".into(); }
    #[cfg(target_os = "linux")]   { return "linux".into(); }
    #[cfg(target_os = "windows")] { return "windows".into(); }
    #[allow(unreachable_code)]
    "unknown".into()
}
