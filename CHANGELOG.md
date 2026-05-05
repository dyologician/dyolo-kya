# Changelog

All notable changes to dyolo-kya are documented here.

## [2.0.0] — 2026-05-05 (finalized)

### Audit fixes (second pass)

- **`sdk/go/kya/client_test.go`** — changed test package from `kya_test` (external) to `kya` (internal). External packages cannot access unexported struct fields (`base`, `http`, `headers`); internal tests can. All `TestNew_*` assertions against struct internals now compile correctly.
- **`sdk/typescript/tsconfig.test.json`** (new) — dedicated TypeScript config for Jest. Sets `rootDir: "."` (not `./src`) so ts-jest can compile `tests/` without a "rootDir is expected to contain all source files" error. `jest.config.ts` updated to reference this config.
- **`sdk/python/tests/test_client.py`** (new) — 16 pytest tests covering sync and async `KyaClient` using `respx` to mock `httpx`. Without this, `pytest` exits with code 5 ("no tests collected"), failing CI.
- **`sdk/python/pyproject.toml`** — added `[tool.pytest.ini_options]`: `asyncio_mode = "auto"` (required for `pytest-asyncio` ≥ 0.21) and `testpaths = ["tests"]`.
- **`wire/schema.json`** — replaced stub with the complete JSON Schema (Draft 2020-12) for `SignedChain` and `VerifiedToken`, including all nested types with exact regex constraints on hex-encoded fields.



### Finalization fixes

- **`sdk/go/go.mod`** — module path corrected to `github.com/dyologician/dyolo-kya/sdk/go`. No external dependencies; all imports are Go stdlib.
- **`sdk/go/kya/client_test.go`** — comprehensive unit test suite using `net/http/httptest`. Covers `New` option variants, `WellKnown`, `Authorize`, `AuthorizeBatch`, `RevokeCert`, `RevokeCertsBatch`, `InspectCert`, `KyaError` formatting, and non-JSON gateway error handling.
- **`sdk/python/dyolo_kya/py.typed`** — marker file is now correctly empty per PEP 561.
- **`sdk/typescript/tests/integrations.test.ts`** — full Jest test suite for all six integration builders: `buildLangChainKyaTool`, `buildLangChainKyaBatchTool`, `buildOpenAIKyaFunction`, `buildOpenAIKyaBatchFunction`, `withKyaGuard`, `withKyaBatchGuard`, and `KyaError`. Covers success paths, auth denial, batch partial failure, invalid JSON args, and downstream execute errors.
- **`sdk/typescript/package.json`** — added `jest`, `ts-jest`, `@types/jest`, `@jest/globals` as devDependencies; added `test` and `test:coverage` scripts; `prepublishOnly` now runs `build && test`.
- **`dyolo-kya-cli/src/commands/migrate.rs`** — new `migrate` subcommand. `dyolo-kya migrate --database-url postgres://...` applies the full `dyolo-kya-pg` schema migration in one command. `--print` flag emits the raw DDL for manual or CI application.
- **`dyolo-kya-cli/Cargo.toml`** — added `dyolo-kya-pg` and `sqlx` (postgres + runtime-tokio-rustls) as dependencies to support the new `migrate` command.
- **`README.md`** — fixed code example: `Intent::new("trade.equity")` → `Intent::new("trade.equity").unwrap()`.



### Core protocol

- **`src/audit.rs`** — new module. `AuditEvent`, `AuditOutcome`, `AuditSink`, `NoopAuditSink`, `LogAuditSink`, `CompositeAuditSink`, and async `AsyncAuditSink`. Every authorization attempt produces a structured NDJSON-compatible event. Feed directly into Splunk, Datadog Logs, Elasticsearch, or any SIEM.
- **`src/policy.rs`** — new module. `DelegationPolicy` (max depth, max TTL, capability sets, required extensions, trusted principals), `CapabilitySet` (prefix-matched allow lists with wildcard support), and `PolicySet` (ordered composition). Policy-as-code for enterprise guardrails on top of cryptographic chain validation.
- **`src/chain.rs`** — `DyoloChain::authorize_with_options` adds optional `PolicySet` and `AuditSink` parameters. `DyoloChain::authorize_batch` authorizes multiple intents atomically — if any intent fails, no nonces are consumed. `authorize_async_with_options` is the async equivalent.
- **`src/cert.rs`** — new `CertBundle` type for batch cert issuance in a single round-trip to the gateway.
- **`src/error.rs`** — two new `KyaError` variants: `PolicyViolation(String)` and `BatchItemFailed { index, reason }`. New `error_code() -> &'static str` method returns machine-readable codes suitable for HTTP responses and SIEM filtering.
- **`src/registry.rs`** — `NonceStore::try_consume` atomically checks and marks a nonce in a single critical section, eliminating the TOCTOU race present in sequential `is_consumed + mark_consumed` calls across all backends.

### Gateway (`dyolo-kya-gateway`)

- **Rate limiting** — global token-bucket rate limiter via `governor`. Configurable via `DYOLO_RATE_LIMIT_RPS` env var (default: 500 req/sec). Returns `429 Too Many Requests` on breach.
- **`GET /.well-known/kya-configuration`** — OIDC-style discovery document listing all endpoint URLs, the gateway's signing key, and supported algorithms. Enables zero-config client bootstrap.
- **`POST /v1/authorize/batch`** — batch authorize multiple intents atomically in one round-trip. All-or-nothing: if any intent fails, no nonces are consumed.
- **`POST /v1/cert/revoke-batch`** — CRL-style batch revocation. Revoke a list of cert fingerprints in one call. Returns per-fingerprint success/failure detail.
- **Error responses** — all error bodies now include `error_code` (machine-readable) alongside `error` (human-readable). Storage errors return `503` instead of `403`.

### Go SDK (`sdk/go`)

- New Go module at `github.com/dyologician/dyolo-kya/sdk/go`. Zero dependencies beyond the standard library.
- Full API: `WellKnown`, `IssueCert`, `Authorize`, `AuthorizeBatch`, `RevokeCert`, `RevokeCertsBatch`, `InspectCert`, `VerifyToken`.
- `KyaError` type carries both `Message` and `ErrorCode` for structured error handling.
- Configurable via `WithTimeout` and `WithHeader` options.

### TypeScript SDK (`sdk/typescript`)

- `authorizeBatch` — batch authorize with identical atomicity guarantees as the gateway.
- `revokeCertsBatch` — batch revocation in one round-trip.
- `wellKnown` — discovery document fetch for zero-config setup.
- `AuthorizeOptions` now accepts `requestId` for end-to-end correlation.
- All error responses now expose `code` (machine-readable `error_code`) for programmatic handling.

## [1.0.0] — 2025-05-03

Initial release.

- `DyoloChain` with Ed25519 batch signature verification.
- `DelegationCert` with Merkle sub-scope proofs and temporal monotonicity.
- `MemoryNonceStore` and `MemoryRevocationStore` with sharded locks and bloom filter fast-path.
- `dyolo-kya-redis` — async Redis storage backends.
- `dyolo-kya-pg` — async PostgreSQL storage backends.
- `dyolo-kya-gateway` — REST sidecar with cert issuance, single authorization, and token verification.
- Python SDK with LangChain and OpenAI tool adapters.
- TypeScript SDK with LangChain.js and OpenAI Agents adapters.
- Zero-knowledge rollup via RISC Zero (feature-gated).
