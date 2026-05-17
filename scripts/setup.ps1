# A1 — Know Your Agent  v2.8.0
# https://github.com/dyologician/a1
#
# Usage:
#   .\setup.ps1           Start A1
#   .\setup.ps1 stop      Stop A1
#   .\setup.ps1 status    Check if running
#   .\setup.ps1 restart   Restart A1
#
# Run as: Right-click → "Run with PowerShell" — or — .\setup.ps1

param([string]$Command = "start")

$ErrorActionPreference = "Continue"

$Version        = "2.8.0"
$StudioUrl      = "http://localhost:8080/studio"
$QuickstartUrl  = "http://localhost:8080/studio?tab=wizard"
$HealthUrl      = "http://localhost:8080/healthz"
$A1Dir          = "$env:USERPROFILE\.a1"
$BinDir         = "$A1Dir\bin"
$BinPath        = "$BinDir\a1-gateway.exe"
$LogFile        = "$A1Dir\logs\gateway.log"
$PidFile        = "$A1Dir\gateway.pid"
$AutostartFlag  = "$A1Dir\autostart-enabled"
$GhBase         = "https://github.com/dyologician/a1/releases/download/v$Version"

New-Item -ItemType Directory -Force -Path "$A1Dir\logs" | Out-Null

function Write-Ok   { param($msg) Write-Host "  [OK]  $msg" -ForegroundColor Green }
function Write-Warn { param($msg) Write-Host "  [!]   $msg" -ForegroundColor Yellow }
function Write-Err  { param($msg) Write-Host "  [X]   $msg" -ForegroundColor Red }

function Test-A1Running {
    try { $r = Invoke-WebRequest $HealthUrl -TimeoutSec 2 -UseBasicParsing -ErrorAction Stop; return $r.StatusCode -eq 200 }
    catch { return $false }
}

function Wait-ForHealth {
    Write-Host "  Starting" -NoNewline
    for ($i = 0; $i -lt 45; $i++) {
        Start-Sleep 1; Write-Host "." -NoNewline
        if (Test-A1Running) { Write-Host ""; return $true }
    }
    Write-Host ""; return $false
}

function Open-Browser([string]$Url = $QuickstartUrl) { Start-Process $Url }

function Enable-GitIgnore {
    $block = "`n# A1 - keep passport keys out of Git`npassport.json`n*-key.hex`n*.passport.json`n.a1/`n"
    if (Test-Path ".gitignore") {
        $c = Get-Content ".gitignore" -Raw -ErrorAction SilentlyContinue
        if ($c -notmatch "passport\.json") { Add-Content ".gitignore" $block }
    } elseif (& git rev-parse --is-inside-work-tree 2>$null) {
        Set-Content ".gitignore" $block.TrimStart()
        Write-Host "  Created .gitignore — passport keys protected from Git" -ForegroundColor DarkGray
    }
}

function Enable-Autostart {
    if (Test-Path $AutostartFlag) { return }
    try {
        $action   = New-ScheduledTaskAction -Execute $BinPath
        $trigger  = New-ScheduledTaskTrigger -AtLogOn
        $settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Hours 0) -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1)
        $principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -RunLevel Highest
        Register-ScheduledTask -TaskName "A1 Gateway" -Action $action -Trigger $trigger -Settings $settings -Principal $principal -Force | Out-Null
        New-Item -ItemType File -Force -Path $AutostartFlag | Out-Null
        Write-Ok "Auto-start enabled — A1 will run after every login"
    } catch {
        Write-Warn "Auto-start setup skipped (may need admin rights)"
    }
}

function New-DesktopShortcut {
    $shortcut = "$env:USERPROFILE\Desktop\A1 Gateway.lnk"
    if (Test-Path $shortcut) { return }
    try {
        $wsh = New-Object -ComObject WScript.Shell
        $lnk = $wsh.CreateShortcut($shortcut)
        $lnk.TargetPath       = "powershell.exe"
        $lnk.Arguments        = "-WindowStyle Hidden -Command `"Start-Process '$BinPath'; Start-Sleep 3; Start-Process '$StudioUrl'`""
        $lnk.WorkingDirectory = $A1Dir
        $lnk.Description      = "Open A1 Studio"
        $lnk.Save()
        Write-Host "  Desktop shortcut created — double-click 'A1 Gateway' to open Studio" -ForegroundColor DarkGray
    } catch {}
}

function Install-Docker {
    Write-Warn "Docker not found — installing Docker Desktop automatically..."
    $url = "https://desktop.docker.com/win/main/amd64/Docker%20Desktop%20Installer.exe"
    $tmp = "$env:TEMP\DockerInstaller-A1.exe"
    try {
        Invoke-WebRequest $url -OutFile $tmp -UseBasicParsing -ErrorAction Stop
    } catch {
        Write-Err "Docker download failed. Install manually: https://docs.docker.com/get-docker/"
        return $false
    }
    Write-Host "  Installing Docker Desktop..." -ForegroundColor DarkGray
    Start-Process $tmp -ArgumentList "install --quiet" -Wait
    Remove-Item $tmp -Force -ErrorAction SilentlyContinue
    $dockerExe = "C:\Program Files\Docker\Docker\Docker Desktop.exe"
    if (Test-Path $dockerExe) {
        Start-Process $dockerExe
        Write-Host "  Waiting for Docker Desktop..." -ForegroundColor DarkGray
        for ($i = 0; $i -lt 30; $i++) {
            Start-Sleep 2
            try { docker info 2>$null | Out-Null; if ($LASTEXITCODE -eq 0) { Write-Ok "Docker is ready!"; return $true } } catch {}
        }
    }
    Write-Warn "Docker installed but still starting. Open Docker Desktop, wait for the green icon, then re-run .\setup.ps1"
    return $false
}

function Start-Binary {
    if (Test-A1Running) { return $true }
    New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
    if (-not (Test-Path $BinPath)) {
        Write-Host "  Downloading A1 (one-time, ~10 MB)..." -ForegroundColor DarkGray
        $url = "$GhBase/a1-gateway-$Version-x86_64-pc-windows-msvc.exe"
        try { Invoke-WebRequest $url -OutFile $BinPath -UseBasicParsing -ErrorAction Stop }
        catch { Write-Warn "Download failed: $_"; return $false }
    }
    $proc = Start-Process -FilePath $BinPath -RedirectStandardOutput $LogFile -WindowStyle Hidden -PassThru
    $proc.Id | Out-File $PidFile
    return (Wait-ForHealth)
}

function Start-ViaDocker {
    if (Test-A1Running) { return $true }
    $dockerOk = $false
    try { docker info 2>$null | Out-Null; $dockerOk = ($LASTEXITCODE -eq 0) } catch {}
    if (-not $dockerOk) { $dockerOk = Install-Docker }
    if (-not $dockerOk) { return $false }

    $cf = if (Test-Path "docker\docker-compose.yml") { "docker\docker-compose.yml" }
           elseif (Test-Path "docker-compose.yml")   { "docker-compose.yml" }
           else { return $false }

    Write-Host "  Starting via Docker Compose..." -ForegroundColor DarkGray

    # Auto-generate .env if missing (required for gateway to start)
    if (-not (Test-Path ".env")) {
        Write-Host "  First-time setup: generating secure keys..." -ForegroundColor DarkGray
        $key1 = -join ((1..32) | ForEach-Object { '{0:x2}' -f (Get-Random -Max 256) })
        $key2 = -join ((1..32) | ForEach-Object { '{0:x2}' -f (Get-Random -Max 256) })
        $key3 = -join ((1..16) | ForEach-Object { '{0:x2}' -f (Get-Random -Max 256) })
        "A1_SIGNING_KEY_HEX=$key1`nA1_MAC_KEY_HEX=$key2`nA1_ADMIN_SECRET=$key3" | Set-Content ".env"
        Write-Ok "Secure keys generated!"
    }

    docker compose -f $cf up -d --quiet-pull 2>$null
    return (Wait-ForHealth)
}

function Invoke-PostStart([string]$Method) {
    Write-Host ""
    Write-Ok "A1 is running! (via $Method)"
    Write-Host ""
    Enable-GitIgnore
    Enable-Autostart
    New-DesktopShortcut
    Write-Host ""
    Write-Host "  Opening A1 Studio..." -ForegroundColor Cyan
    Open-Browser $QuickstartUrl
    Write-Host ""
    Write-Host "  Stop:    .\setup.ps1 stop" -ForegroundColor DarkGray
    Write-Host "  Status:  .\setup.ps1 status" -ForegroundColor DarkGray
    Write-Host ""
}

switch ($Command) {
    "stop" {
        Write-Host ""
        if (Test-Path $PidFile) {
            $p = Get-Content $PidFile -ErrorAction SilentlyContinue
            if ($p) { Stop-Process -Id $p -Force -ErrorAction SilentlyContinue; Write-Ok "A1 stopped" }
            Remove-Item $PidFile -Force -ErrorAction SilentlyContinue
        }
        $cf = if (Test-Path "docker\docker-compose.yml") { "docker\docker-compose.yml" }
               elseif (Test-Path "docker-compose.yml")   { "docker-compose.yml" }
               else { $null }
        if ($cf) { try { docker compose -f $cf down 2>$null | Out-Null } catch {} }
        Write-Host ""
        exit 0
    }
    "status" {
        Write-Host ""
        if (Test-A1Running) { Write-Ok "A1 is running  -> $StudioUrl" }
        else { Write-Err "A1 is not running — run .\setup.ps1 to start" }
        Write-Host ""
        exit 0
    }
    "restart" {
        Write-Host ""; Write-Host "  Restarting A1..." -ForegroundColor DarkGray
        & $PSCommandPath stop 2>$null; Start-Sleep 1; & $PSCommandPath start
        exit 0
    }
}

Write-Host ""
Write-Host "  A1 — Know Your Agent  v$Version" -ForegroundColor White
Write-Host ""

if (Test-A1Running) {
    Write-Ok "A1 is already running"
    Open-Browser $QuickstartUrl
    exit 0
}

Write-Host "  Trying pre-built binary..." -ForegroundColor DarkGray
$ok = Start-Binary

if (-not $ok) {
    Write-Warn "Binary unavailable — trying Docker (auto-install if needed)..."
    Write-Host ""
    $ok = Start-ViaDocker
}

if ($ok) { Invoke-PostStart (if ($ok -and (Test-Path $BinPath)) { "pre-built binary" } else { "Docker" }); exit 0 }

Write-Host ""
Write-Err "Could not start A1 automatically."
Write-Host ""
Write-Host "  Option A — Install Docker Desktop (free, 2 min):" -ForegroundColor White
Write-Host "    https://docs.docker.com/get-docker/" -ForegroundColor Cyan
Write-Host "    Then run: .\setup.ps1"
Write-Host ""
Write-Host "  Option B — Port 8080 in use?" -ForegroundColor White
Write-Host "    netstat -ano | findstr :8080"
Write-Host ""
Write-Host "  Option C — Get help:" -ForegroundColor White
Write-Host "    https://github.com/dyologician/a1/issues"
Write-Host ""
exit 1
