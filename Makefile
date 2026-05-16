# A1 — Know Your Agent
# One-command developer entry points.
# Non-coders: use setup.sh (Mac/Linux) or setup.bat (Windows) instead.

.PHONY: help start stop studio build test test-rust test-python test-ts lint fmt audit clean release-dry

VERSION := 2.8.0

help:
	@echo ""
	@echo "  A1 v$(VERSION) — developer commands"
	@echo ""
	@echo "  make start        Start the A1 gateway (downloads binary if needed)"
	@echo "  make stop         Stop the A1 gateway"
	@echo "  make studio       Rebuild studio/index.html from studio/src/"
	@echo "  make build        Build all Rust crates (release)"
	@echo "  make test         Run the full test suite (requires Docker)"
	@echo "  make test-rust    Rust unit + integration tests only"
	@echo "  make test-python  Python SDK tests only"
	@echo "  make test-ts      TypeScript SDK tests only"
	@echo "  make lint         cargo clippy + fmt check"
	@echo "  make fmt          cargo fmt --all"
	@echo "  make audit        cargo audit (security)"
	@echo "  make clean        Remove build artifacts"
	@echo "  make release-dry  Dry-run publish for all crates"
	@echo ""

start:
	@bash setup.sh start

stop:
	@bash setup.sh stop

studio:
	@bash scripts/build-studio.sh

build:
	cargo build --release --all-features

test:
	@bash scripts/test.sh

test-rust:
	cargo test --all-features

test-python:
	cd sdk/python && pip install -e ".[dev]" -q && pytest --tb=short

test-ts:
	cd sdk/typescript && npm ci --silent && npm test

lint:
	cargo fmt --all -- --check
	cargo clippy --all-targets --all-features -- -D warnings

fmt:
	cargo fmt --all

audit:
	cargo audit

clean:
	cargo clean
	cd sdk/typescript && rm -rf dist node_modules
	cd sdk/python && rm -rf dist build *.egg-info

release-dry:
	cargo publish --dry-run -p a1
