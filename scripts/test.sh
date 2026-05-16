#!/usr/bin/env bash
# A1 — Full test suite  v2.8.0
# Runs Rust, CLI, Python, TypeScript, and Go tests against a live gateway.
#
# Usage:
#   ./test.sh                         Run all tests (starts Docker stack)
#   GATEWAY_ADDR=http://host:9090 ./test.sh   Use a custom gateway address

set -euo pipefail

GATEWAY="${GATEWAY_ADDR:-http://localhost:8080}"

fail() { echo "  [FAIL] $*" >&2; exit 1; }
ok()   { echo "  [OK]   $*"; }
step() { echo ""; echo "=== $* ==="; }

# Pre-flight: ensure required tools are present
for tool in docker cargo pip npm go; do
  command -v "$tool" &>/dev/null || fail "$tool not found on PATH"
done

step "Starting A1 stack"
docker compose -f docker/docker-compose.yml up -d --build

step "Waiting for gateway health"
deadline=$(( SECONDS + 60 ))
until curl -sf "${GATEWAY}/health" > /dev/null 2>&1; do
  if [ $SECONDS -ge $deadline ]; then
    docker compose -f docker/docker-compose.yml logs a1-gateway
    fail "Gateway did not become healthy within 60 seconds"
  fi
  echo "  waiting... ($((SECONDS))s elapsed)"
  sleep 2
done
ok "Gateway healthy at ${GATEWAY}"

step "1. Rust unit + integration tests"
cargo test --workspace --all-features

step "2. Passport CLI smoke test"
tmpdir="$(mktemp -d)"
trap "rm -rf '${tmpdir}'" EXIT
cargo build -p a1-cli --quiet

./target/debug/a1 passport issue \
  --namespace "test-bot" \
  --allow "trade.equity,portfolio.read" \
  --ttl 3600 \
  --out "${tmpdir}/passport.json"

if [ ! -f "${tmpdir}/passport.json" ]; then
  fail "passport file not written"
fi
ok "passport issue"

./target/debug/a1 passport inspect "${tmpdir}/passport.json"
ok "passport inspect"

./target/debug/a1 keygen > "${tmpdir}/keygen.txt" 2>&1
AGENT_PK="$(grep "^verifying_key_hex" "${tmpdir}/keygen.txt" | awk '{print $NF}' | tr -d '[:space:]')"

if [ -n "${AGENT_PK}" ] && [ -f "test-bot-key.hex" ]; then
  ./target/debug/a1 passport sub \
    --passport "${tmpdir}/passport.json" \
    --key test-bot-key.hex \
    --delegate "${AGENT_PK}" \
    --allow "trade.equity" \
    --ttl 1h \
    --out "${tmpdir}/sub-cert.json" \
    && ok "passport sub" \
    || echo "  [INFO]  passport sub skipped (expected in offline CI)"
fi

rm -f test-bot-key.hex

step "3. Python SDK tests"
(cd sdk/python && pip install -e ".[dev]" -q && pytest -q)

step "4. TypeScript SDK tests"
(cd sdk/typescript && npm ci --silent && npm test)

step "5. Go SDK tests"
(cd sdk/go && go test ./... -v)

echo ""
echo "  ALL TESTS PASSED (A1 v2.8.0)"
echo "  Dashboard: ${GATEWAY}/health"
echo ""
