#!/usr/bin/env bash
# A1 — Know Your Agent  v2.8.0
# https://github.com/dyologician/a1
#
# Usage:
#   ./setup.sh          Start A1 (everything automatic)
#   ./setup.sh stop     Stop A1
#   ./setup.sh status   Check if A1 is running
#   ./setup.sh restart  Restart A1
#
# Supports: macOS (Apple Silicon + Intel), Linux (x86_64 + ARM64)

set -uo pipefail

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
RESET='\033[0m'

VERSION="2.8.0"
STUDIO_URL="http://localhost:8080/studio"
QUICKSTART_URL="http://localhost:8080/studio?tab=quickstart"
HEALTH_URL="http://localhost:8080/healthz"
BIN_DIR="${HOME}/.a1/bin"
BIN_PATH="${BIN_DIR}/a1-gateway"
PID_FILE="${HOME}/.a1/gateway.pid"
LOG_FILE="${HOME}/.a1/logs/gateway.log"
AUTOSTART_FLAG="${HOME}/.a1/autostart-enabled"
GH_RELEASES="https://github.com/dyologician/a1/releases/download/v${VERSION}"

mkdir -p "${HOME}/.a1/logs"

is_running()    { curl -sf "${HEALTH_URL}" &>/dev/null; }
os_name()       { case "$(uname -s)" in Darwin) echo "mac";; Linux) echo "linux";; *) echo "other";; esac; }

open_browser() {
  local url="${1:-${QUICKSTART_URL}}"
  if   command -v open     &>/dev/null; then open     "$url"
  elif command -v xdg-open &>/dev/null; then xdg-open "$url"
  elif command -v wslview  &>/dev/null; then wslview  "$url"
  fi
}

wait_for_health() {
  local max="${1:-45}"
  echo -n "  Starting"
  for i in $(seq 1 "${max}"); do
    is_running && { echo ""; return 0; }
    sleep 1
    echo -n "."
    # Every 30 seconds, print a newline hint so the user knows it's still working
    if [ $((i % 30)) -eq 0 ] && [ "${i}" -lt "${max}" ]; then
      echo ""
      echo -e "  ${DIM}Still starting… (${i}s elapsed — Docker may be compiling Rust on first run)${RESET}"
      echo -n "  "
    fi
  done
  echo ""; return 1
}

ensure_gitignore() {
  local block
  block="$(printf '# A1 — keep passport keys out of Git\npassport.json\n*-key.hex\n*.passport.json\n.a1/\n')"
  if [ -f ".gitignore" ]; then
    grep -q "passport.json" .gitignore 2>/dev/null || printf "\n%s\n" "$block" >> .gitignore
  elif git rev-parse --is-inside-work-tree &>/dev/null 2>&1; then
    printf "%s\n" "$block" > .gitignore
    echo -e "  ${DIM}Created .gitignore — passport keys protected from Git${RESET}"
  fi
}

enable_autostart() {
  [ -f "${AUTOSTART_FLAG}" ] && return 0
  case "$(os_name)" in
    mac)
      local plist="${HOME}/Library/LaunchAgents/com.a1.gateway.plist"
      mkdir -p "$(dirname "${plist}")"
      cat > "${plist}" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>             <string>com.a1.gateway</string>
  <key>ProgramArguments</key> <array><string>${BIN_PATH}</string></array>
  <key>RunAtLoad</key>         <true/>
  <key>KeepAlive</key>         <true/>
  <key>StandardOutPath</key>   <string>${LOG_FILE}</string>
  <key>StandardErrorPath</key> <string>${LOG_FILE}</string>
</dict>
</plist>
PLIST
      launchctl unload "${plist}" 2>/dev/null || true
      launchctl load   "${plist}" 2>/dev/null && touch "${AUTOSTART_FLAG}" || true
      ;;
    linux)
      if command -v systemctl &>/dev/null && systemctl --user daemon-reload &>/dev/null 2>&1; then
        local svc="${HOME}/.config/systemd/user/a1-gateway.service"
        mkdir -p "$(dirname "${svc}")"
        cat > "${svc}" <<SVC
[Unit]
Description=A1 Gateway
After=network.target

[Service]
ExecStart=${BIN_PATH}
Restart=always

[Install]
WantedBy=default.target
SVC
        systemctl --user daemon-reload
        systemctl --user enable --now a1-gateway 2>/dev/null || true
        touch "${AUTOSTART_FLAG}"
      else
        local line="# A1 auto-start\n[ -x '${BIN_PATH}' ] && pgrep -x a1-gateway >/dev/null || nohup '${BIN_PATH}' >>'${LOG_FILE}' 2>&1 &"
        for rc in "${HOME}/.bashrc" "${HOME}/.zshrc"; do
          [ -f "${rc}" ] && grep -q "A1 auto-start" "${rc}" 2>/dev/null || printf "\n%b\n" "${line}" >> "${rc}"
        done
        touch "${AUTOSTART_FLAG}"
      fi
      ;;
  esac
}

create_desktop_launcher() {
  case "$(os_name)" in
    mac)
      local cmd="${HOME}/Desktop/A1 Gateway.command"
      [ -f "${cmd}" ] && return 0
      cat > "${cmd}" << 'CMD'
#!/usr/bin/env bash
HEALTH="http://localhost:8080/healthz"
BIN="${HOME}/.a1/bin/a1-gateway"
LOG="${HOME}/.a1/logs/gateway.log"
if ! curl -sf "$HEALTH" &>/dev/null; then
  echo "Starting A1..."
  nohup "$BIN" >> "$LOG" 2>&1 &
  for i in $(seq 1 30); do curl -sf "$HEALTH" &>/dev/null && break; sleep 1; done
fi
open "http://localhost:8080/studio"
CMD
      chmod +x "${cmd}"
      echo -e "  ${DIM}Desktop launcher created: 'A1 Gateway' on your Desktop${RESET}"
      ;;
    linux)
      local dt="${HOME}/Desktop/A1-Gateway.desktop"
      [ -f "${dt}" ] && return 0
      mkdir -p "${HOME}/Desktop"
      cat > "${dt}" << DT
[Desktop Entry]
Version=1.0
Type=Application
Name=A1 Gateway
Comment=Open A1 Studio
Exec=bash -c '${BIN_PATH}; xdg-open http://localhost:8080/studio'
Icon=security-high
Terminal=false
Categories=Utility;
DT
      chmod +x "${dt}"
      ;;
  esac
}

install_docker_mac() {
  echo -e "  ${YELLOW}Downloading Docker Desktop (~600 MB, 1-3 min)...${RESET}"
  local arch; arch="$(uname -m)"
  local url
  if [ "$arch" = "arm64" ]; then
    url="https://desktop.docker.com/mac/main/arm64/Docker.dmg"
  else
    url="https://desktop.docker.com/mac/main/amd64/Docker.dmg"
  fi
  local tmp="/tmp/Docker-A1-Install.dmg"
  curl -fL --progress-bar "${url}" -o "${tmp}" || return 1
  hdiutil attach "${tmp}" -quiet -nobrowse
  [ -d "/Volumes/Docker/Docker.app" ] && cp -R "/Volumes/Docker/Docker.app" /Applications/
  hdiutil detach "/Volumes/Docker" -quiet 2>/dev/null || true
  rm -f "${tmp}"
  open -a Docker
  echo -n "  Waiting for Docker Desktop"
  for i in $(seq 1 30); do
    sleep 2; echo -n "."
    docker info &>/dev/null 2>&1 && { echo ""; return 0; }
  done
  echo ""
  return 1
}

ensure_docker() {
  command -v docker &>/dev/null && docker info &>/dev/null 2>&1 && return 0
  if command -v docker &>/dev/null && [ -d "/Applications/Docker.app" ]; then
    open -a Docker
    echo -n "  Waiting for Docker Desktop"
    for i in $(seq 1 30); do
      sleep 2; echo -n "."
      docker info &>/dev/null 2>&1 && { echo ""; return 0; }
    done
    echo ""
  fi
  case "$(os_name)" in
    mac)   install_docker_mac ;;
    linux)
      echo -e "  ${YELLOW}Installing Docker Engine...${RESET}"
      curl -fsSL https://get.docker.com | sh &>/dev/null \
        && sudo usermod -aG docker "${USER}" &>/dev/null \
        && sudo systemctl start docker &>/dev/null \
        && return 0
      return 1
      ;;
    *) return 1 ;;
  esac
}

try_binary() {
  is_running && return 0
  local OS ARCH FILENAME
  OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
  ARCH="$(uname -m)"
  case "$ARCH" in x86_64|amd64) ARCH="x86_64";; arm64|aarch64) ARCH="aarch64";; *) return 1;; esac
  case "$OS" in
    darwin) FILENAME="a1-gateway-${VERSION}-${ARCH}-apple-darwin" ;;
    linux)  FILENAME="a1-gateway-${VERSION}-${ARCH}-unknown-linux-gnu" ;;
    *)      return 1 ;;
  esac

  if [ ! -x "${BIN_PATH}" ]; then
    echo -e "  ${DIM}Downloading A1 (one-time, ~10 MB)...${RESET}"
    mkdir -p "${BIN_DIR}"
    local url="${GH_RELEASES}/${FILENAME}"
    curl -fsSL "${url}" -o "${BIN_PATH}" 2>/dev/null \
      || wget -qO "${BIN_PATH}" "${url}" 2>/dev/null \
      || return 1
    chmod +x "${BIN_PATH}"
  fi

  mkdir -p "$(dirname "${PID_FILE}")"
  nohup "${BIN_PATH}" > "${LOG_FILE}" 2>&1 &
  echo $! > "${PID_FILE}"
  wait_for_health && return 0
  kill "$(cat "${PID_FILE}")" 2>/dev/null || true
  rm -f "${PID_FILE}"
  return 1
}

try_cargo() {
  is_running && return 0
  command -v cargo &>/dev/null || return 1

  local CARGO_BIN="./target/release/a1-gateway"

  # ── Fast path: release binary already compiled ──────────────────────────────
  if [ -x "${CARGO_BIN}" ]; then
    echo -e "  ${DIM}Starting A1 from local Rust build...${RESET}"
    mkdir -p "$(dirname "${PID_FILE}")"
    nohup "${CARGO_BIN}" >> "${LOG_FILE}" 2>&1 &
    echo $! > "${PID_FILE}"
    wait_for_health 45 && return 0
    kill "$(cat "${PID_FILE}")" 2>/dev/null || true
    rm -f "${PID_FILE}"
    return 1
  fi

  # ── First-time path: compile with cargo ─────────────────────────────────────
  echo ""
  echo -e "  ${YELLOW}Rust detected — compiling A1 gateway (first-time build, ~3–10 min)...${RESET}"
  echo -e "  ${DIM}Go grab a coffee ☕  This only happens once.${RESET}"
  echo ""

  # Stream meaningful compiler lines so the user sees progress
  cargo build -p a1-gateway --release 2>&1 \
    | grep --line-buffered -E "^(error|warning\[|Compiling|Finished|Blocking)" \
    || true

  [ -x "${CARGO_BIN}" ] || return 1

  echo ""
  mkdir -p "$(dirname "${PID_FILE}")"
  nohup "${CARGO_BIN}" >> "${LOG_FILE}" 2>&1 &
  echo $! > "${PID_FILE}"
  wait_for_health 60 && return 0
  kill "$(cat "${PID_FILE}")" 2>/dev/null || true
  rm -f "${PID_FILE}"
  return 1
}

try_docker() {
  is_running && return 0
  ensure_docker || return 1
  local compose_file=""
  for f in "docker-compose.yml" "docker/docker-compose.yml"; do
    [ -f "$f" ] && compose_file="$f" && break
  done
  [ -z "${compose_file:-}" ] && return 1

  # Detect whether the gateway image is already built.
  # On first run Docker must compile the full Rust project (~3–10 min).
  # Docker Compose derives the project name from the directory: lowercase, strip
  # everything except [a-z0-9], so "A1 test" → "a1test".
  local image_ready=0
  local project_name
  project_name="$(basename "$(pwd)" | tr '[:upper:]' '[:lower:]' | tr -cd 'a-z0-9')"
  # Also accept common alternate image names
  docker image ls --format '{{.Repository}}' 2>/dev/null \
    | grep -qiE "(${project_name}|a1-gateway|a1_gateway|a1gateway)" && image_ready=1
  # If still not found, check running containers (image is always built if containers ran before)
  if [ "${image_ready}" -eq 0 ]; then
    docker ps -a --format '{{.Image}}' 2>/dev/null \
      | grep -qiE "(${project_name}|a1.gateway)" && image_ready=1
  fi

  if [ "${image_ready}" -eq 0 ]; then
    echo ""
    echo -e "  ${YELLOW}First-time setup: building the Docker image...${RESET}"
    echo -e "  ${DIM}This compiles the Rust gateway (~3–10 min). It only happens once.${RESET}"
    echo -e "  ${DIM}Go grab a coffee — this terminal will beep when it's ready.${RESET}"
    echo ""
    # Stream build output so the user can see progress, suppress errors
    docker compose -f "${compose_file}" build 2>/dev/null || true
  fi

  echo -e "  ${DIM}Starting via Docker Compose...${RESET}"

  # ── Auto-generate .env if missing (required for gateway to start) ──────────
  if [ ! -f ".env" ]; then
    echo -e "  ${DIM}First-time setup: generating secure keys...${RESET}"
    printf "A1_SIGNING_KEY_HEX=%s\nA1_MAC_KEY_HEX=%s\nA1_ADMIN_SECRET=%s\n" \
      "$(openssl rand -hex 32 2>/dev/null || python3 -c 'import secrets; print(secrets.token_hex(32))')" \
      "$(openssl rand -hex 32 2>/dev/null || python3 -c 'import secrets; print(secrets.token_hex(32))')" \
      "$(openssl rand -hex 16 2>/dev/null || python3 -c 'import secrets; print(secrets.token_hex(16))')" \
      > .env
    echo -e "  ${GREEN}Secure keys generated!${RESET}"
  fi

  docker compose -f "${compose_file}" up -d --quiet-pull 2>/dev/null \
    || docker-compose -f "${compose_file}" up -d 2>/dev/null \
    || return 1

  # After a fresh build all three services (gateway + redis + postgres) need to
  # initialise. Use 300 s on first run, 90 s if the image was already cached.
  local wait_secs=90
  [ "${image_ready}" -eq 0 ] && wait_secs=300

  wait_for_health "${wait_secs}" && { printf '\a'; return 0; } || return 1
}

post_start() {
  local method="$1"
  echo ""
  echo -e "  ${BOLD}${GREEN}A1 is running!${RESET}  ${DIM}(via ${method})${RESET}"
  echo ""
  ensure_gitignore
  enable_autostart 2>/dev/null \
    && echo -e "  ${GREEN}Auto-start enabled — A1 will run on every login${RESET}" \
    || true
  create_desktop_launcher 2>/dev/null || true
  echo ""
  echo -e "  ${CYAN}Opening A1 Studio...${RESET}  ${DIM}${QUICKSTART_URL}${RESET}"
  open_browser "${QUICKSTART_URL}"
  echo ""
  echo -e "  ${DIM}Stop:    ./setup.sh stop${RESET}"
  echo -e "  ${DIM}Status:  ./setup.sh status${RESET}"
  echo ""
}

cmd_stop() {
  echo ""
  if [ -f "${PID_FILE}" ]; then
    local pid; pid="$(cat "${PID_FILE}")"
    kill "${pid}" 2>/dev/null && echo -e "  ${GREEN}A1 stopped (PID ${pid})${RESET}" || true
    rm -f "${PID_FILE}"
  fi
  local cf
  for f in "docker-compose.yml" "docker/docker-compose.yml"; do
    [ -f "$f" ] && cf="$f" && break
  done
  if [ -n "${cf:-}" ] && command -v docker &>/dev/null && docker info &>/dev/null 2>&1; then
    docker compose -f "${cf}" down --quiet 2>/dev/null || true
  fi
  echo ""
}

cmd_status() {
  echo ""
  if is_running; then
    echo -e "  ${GREEN}A1 is running${RESET}  →  ${STUDIO_URL}"
  else
    echo -e "  ${RED}A1 is not running${RESET}  — run ./setup.sh to start"
  fi
  echo ""
}

case "${1:-start}" in
  stop)    cmd_stop;                   exit 0 ;;
  status)  cmd_status;                 exit 0 ;;
  restart) cmd_stop 2>/dev/null; bash "$0" start; exit 0 ;;
esac

echo ""
echo -e "  ${BOLD}A1 — Know Your Agent${RESET}  ${DIM}v${VERSION}${RESET}"
echo ""

if is_running; then
  echo -e "  ${GREEN}A1 is already running${RESET}"
  open_browser "${QUICKSTART_URL}"
  exit 0
fi

echo -e "  ${DIM}Trying pre-built binary...${RESET}"
if try_binary 2>/dev/null; then
  post_start "pre-built binary"
  exit 0
fi

# ── Cargo fallback (works if Rust is installed — common on dev machines) ──────
if command -v cargo &>/dev/null; then
  echo -e "  ${DIM}Binary unavailable — trying local Rust (cargo)...${RESET}"
  if try_cargo; then
    post_start "local Rust build"
    exit 0
  fi
fi

echo -e "  ${YELLOW}Trying Docker (auto-install if needed)...${RESET}"
echo ""
if try_docker 2>/dev/null; then
  post_start "Docker"
  exit 0
fi

echo ""
echo -e "  ${RED}Could not start A1 automatically.${RESET}"
echo ""
echo -e "  ${BOLD}Option A${RESET} — Have Rust / cargo installed? Run directly (fastest):"
echo "    cargo run -p a1-gateway --release"
echo ""
echo -e "  ${BOLD}Option B${RESET} — Docker first-run still compiling? (takes 3–10 min)"
echo "    Run this to watch the build live, then re-run ./setup.sh when done:"
echo "    docker compose build"
echo ""
echo -e "  ${BOLD}Option C${RESET} — Install Docker Desktop if not installed (free, 2 min):"
echo "    https://docs.docker.com/get-docker/"
echo "    Then run:  ./setup.sh"
echo ""
echo -e "  ${BOLD}Option D${RESET} — Port 8080 in use?"
echo "    lsof -i :8080"
echo ""
echo -e "  ${BOLD}Option E${RESET} — Get help:"
echo "    https://github.com/dyologician/a1/issues"
echo ""
exit 1
