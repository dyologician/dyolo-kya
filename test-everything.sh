#!/bin/bash
set -e

echo "=== Starting full dyolo-kya v2 stack ==="
docker compose -f docker/docker-compose.yml up -d --build

echo "=== Waiting for services (10 seconds) ==="
sleep 10

echo "=== Checking if gateway is running ==="
curl -s http://localhost:8080/health || echo "❌ Gateway not reachable yet"

echo "=== Running ALL tests ==="
echo "1. Rust tests"
cargo test --workspace --all-features

echo "2. Python SDK tests"
cd sdk/python && pip install -e ".[dev]" && pytest -q && cd ../..

echo "3. TypeScript SDK tests"
cd sdk/typescript && npm ci && npm test && cd ../..

echo "4. Go SDK tests"
cd sdk/go && go test ./... -v && cd ../..

echo ""
echo "✅ ALL TESTS DONE"
echo "Open dashboard → http://localhost:8080/health should now work"
echo "Or open the file: test-dashboard.html in Safari"
