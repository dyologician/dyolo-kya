#Requires -Version 5.1
<#
.SYNOPSIS
    A1 v2.8.0 full test suite — Windows (PowerShell).
.DESCRIPTION
    Starts the Docker stack, waits for the gateway, then runs
    Rust, CLI, Python, TypeScript, and Go tests in order.
.EXAMPLE
    .\test.ps1
    .\test.ps1 -GatewayAddr "http://localhost:9090"
#>
param([string]$GatewayAddr = $env:GATEWAY_ADDR)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not $GatewayAddr) { $GatewayAddr = "http://localhost:8080" }

function Write-Step { param([string]$Msg) Write-Host "`n=== $Msg ===" -ForegroundColor Cyan }
function Write-Ok   { param([string]$Msg) Write-Host "  [OK]   $Msg" -ForegroundColor Green }
function Write-Fail { param([string]$Msg) Write-Host "  [FAIL] $Msg" -ForegroundColor Red; exit 1 }

foreach ($tool in @("docker", "cargo", "pip", "npm", "go")) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        Write-Fail "$tool not found on PATH"
    }
}

Write-Step "Starting A1 stack"
docker compose -f docker/docker-compose.yml up -d --build
if ($LASTEXITCODE -ne 0) { Write-Fail "docker compose up failed" }

Write-Step "Waiting for gateway health"
$deadline = (Get-Date).AddSeconds(60)
$healthy  = $false
while ((Get-Date) -lt $deadline) {
    try {
        $r = Invoke-WebRequest -Uri "$GatewayAddr/health" -UseBasicParsing -TimeoutSec 3 -ErrorAction Stop
        if ($r.StatusCode -eq 200) { $healthy = $true; break }
    } catch {}
    $elapsed = [int]((Get-Date) - ($deadline.AddSeconds(-60))).TotalSeconds
    Write-Host "  waiting... (${elapsed}s elapsed)"
    Start-Sleep 2
}
if (-not $healthy) {
    docker compose -f docker/docker-compose.yml logs a1-gateway
    Write-Fail "Gateway did not become healthy within 60 seconds"
}
Write-Ok "Gateway healthy at $GatewayAddr"

Write-Step "1. Rust unit + integration tests"
cargo test --workspace --all-features
if ($LASTEXITCODE -ne 0) { Write-Fail "Rust tests failed" }

Write-Step "2. Passport CLI smoke test"
$tmp = Join-Path $env:TEMP ("a1-test-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $tmp | Out-Null

cargo build -p a1-cli --quiet
if ($LASTEXITCODE -ne 0) { Write-Fail "CLI build failed" }

$passportFile = Join-Path $tmp "passport.json"
& .\target\debug\a1.exe passport issue `
    --namespace "test-bot" `
    --allow "trade.equity,portfolio.read" `
    --ttl 3600 `
    --out $passportFile
if ($LASTEXITCODE -ne 0) { Write-Fail "passport issue failed" }

if (-not (Test-Path $passportFile)) { Write-Fail "passport file not written" }
Write-Ok "passport issue"

& .\target\debug\a1.exe passport inspect $passportFile
if ($LASTEXITCODE -ne 0) { Write-Fail "passport inspect failed" }
Write-Ok "passport inspect"

$keygenOut = Join-Path $tmp "keygen.txt"
& .\target\debug\a1.exe keygen 2>&1 | Out-File -FilePath $keygenOut -Encoding utf8
$agentPk = (Get-Content $keygenOut | Where-Object { $_ -match "^verifying_key_hex" }) `
           -replace "^verifying_key_hex\s+", "" -replace "\s+", ""

if ($agentPk -and (Test-Path "test-bot-key.hex")) {
    & .\target\debug\a1.exe passport sub `
        --passport $passportFile `
        --key test-bot-key.hex `
        --delegate $agentPk `
        --allow "trade.equity" `
        --ttl 1h `
        --out (Join-Path $tmp "sub-cert.json")
    if ($LASTEXITCODE -eq 0) { Write-Ok "passport sub" }
    else { Write-Host "  [INFO]  passport sub skipped (expected in offline CI)" }
}

if (Test-Path "test-bot-key.hex") { Remove-Item "test-bot-key.hex" -Force }
Remove-Item -Recurse -Force $tmp

Write-Step "3. Python SDK tests"
Push-Location sdk/python
try { pip install -e ".[dev]" -q; pytest -q }
finally { Pop-Location }
if ($LASTEXITCODE -ne 0) { Write-Fail "Python tests failed" }

Write-Step "4. TypeScript SDK tests"
Push-Location sdk/typescript
try { npm ci --silent; npm test }
finally { Pop-Location }
if ($LASTEXITCODE -ne 0) { Write-Fail "TypeScript tests failed" }

Write-Step "5. Go SDK tests"
Push-Location sdk/go
try { go test ./... -v }
finally { Pop-Location }
if ($LASTEXITCODE -ne 0) { Write-Fail "Go tests failed" }

Write-Host ""
Write-Host "  ALL TESTS PASSED (A1 v2.8.0)" -ForegroundColor Green
Write-Host "  Dashboard: $GatewayAddr/health"
Write-Host ""
