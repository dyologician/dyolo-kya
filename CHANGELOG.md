# Changelog

All notable changes to A1 are documented here. Versions follow [Semantic Versioning](https://semver.org/).

---

## [2.8.0] — 2026-05-16 (patch 3 — dashboard overhaul)

### Bug fixes

- **`setup.sh`** — Fixed Docker compose file selection order. The script tried `docker/docker-compose.yml` first, creating project name `docker` (derived from the file's directory), which is different from the `a1test` project created when users run `docker compose up -d` from the root. Changed to try `docker-compose.yml` (root) first. Also fixed image detection: Docker strips ALL non-alphanumeric characters from the directory name, so "A1 test" → `a1test` not `a1-test`; updated `tr` expression accordingly. Added fallback detection via `docker ps -a`. Fixed `QUICKSTART_URL` to open `?tab=quickstart` (was `?tab=wizard`, which sent users to the wrong tab after setup).
- **`docker/docker-compose.yml`** — Fixed gateway healthcheck: image only has `curl`, not `wget` — changed to `curl -sf http://localhost:8080/healthz`. Added studio volume mount `../studio/index.html:/studio/index.html:ro` so the studio can be updated without rebuilding the Docker image.
- **`a1-gateway/src/routes/studio.rs`** — Changed from `include_str!()` (compile-time embed only) to a three-level fallback: `A1_STUDIO_PATH` env var → `/studio/index.html` volume mount → compiled-in bytes. Studio updates now take effect on container restart, not full Docker rebuild.
- **`a1-gateway/src/main.rs`** — Added `POST /v1/system/shutdown` endpoint: sends SIGTERM to itself after a 200ms delay so the HTTP response is delivered before shutdown. Added `POST /v1/agents/disconnect` route.
- **`a1-gateway/src/routes/agent_connect.rs`** — Added `disconnect_handler`: reads the agent's `.mcp.json`, removes the `"a1"` key from `mcpServers`, and writes the cleaned file back. Returns a clear message with the config file path.
- **`studio/src/js/components/08-connect-agents.js`** — Disconnect button was calling `connect(ag)` (the connect function) instead of a disconnect function. Added `disconnect(ag)` which calls `POST /v1/agents/disconnect` and reloads the agent list on success.
- **`studio/src/js/components/07-lifecycle.js`** — "Stop A1" button no longer shows a terminal command. It now calls `POST /v1/system/shutdown` directly via `fetch`. The Start A1 state shows a copyable `./setup.sh` command instead of asking users to type it. Added `stopping` state and a post-stop confirmation message.
- **`studio/src/js/components/16-passport-all.js`** — Revoke success now saves an entry to `localStorage('a1_revoke_history')` with namespace, fingerprint, timestamp, and path.
- **`studio/src/js/components/19-passports-hub.js`** — Added `🗑 Revoke History` sub-tab. Shows all locally-tracked revocation events with timestamps and fingerprints. Includes a Clear button. History persists across reloads (localStorage).
- **`studio/src/js/99-app.js`** — Added `AgentsBadge` component that shows a green count badge on the Connect Agents sidebar item when at least one agent is connected (polls `/v1/agents/scan` every 30 seconds).
- **`studio/index.html`** — Rebuilt from all fixed source files.

---

## [2.8.0] — 2026-05-16 (patch 2)

### Bug fixes

- **`setup.sh`** — Fixed Docker first-run timeout. On first run, Docker must compile the entire Rust gateway from source (~200 crates, 3–10 minutes). The previous 45-second health-check wait always expired before the build finished. `wait_for_health` now accepts a configurable timeout; `try_docker` detects whether the image is already cached and uses 300 seconds on first build vs 90 seconds on subsequent starts. Added mid-wait progress messages every 30 seconds so the user knows the process is still running. Added a terminal bell (`\a`) on successful start. Updated the failure message to explain the first-run build scenario as Option A instead of only mentioning Docker installation.
- **`README.md` / `GETTING-STARTED.md`** — Added a "Common mistake" callout under every CLI install section explaining that `cargo install --path . --bin a1-cli` fails because the workspace root is the library crate (named `a1`), not the CLI. The correct command is `cargo install --path a1-cli`. Added a "First run: 3–10 min" note to every Docker quickstart block.

---

## [2.8.0] — 2026-05-15

### Hybrid Signature Algorithm Framework

- **`src/hybrid.rs`** — New module implementing the full hybrid apost-quantum signature framework. Introduces `SignatureAlgorithm` (Ed25519 / HybridMlDsa44Ed25519 / HybridMlDsa65Ed25519), `HybridSignature` with PQ context binding commitment, `HybridPublicKey` with algorithm-tagged commitment, `HybridSigner` trait for KMS and HSM integration, `ClassicalHybridAdapter` for zero-effort migration of existing signers, `ChainAlgorithmCompatibility` for mixed-algorithm chain validation, and `negotiate_algorithm` for deployment-context algorithm selection. The framework is fully functional with Ed25519 today; enabling the `post-quantum` feature activates real ML-DSA verification when an ML-DSA crate is linked.

- **`src/error.rs`** — New error variants: `UnsupportedAlgorithm(u8)`, `AlgorithmMismatch`, `HybridSignatureInvalid`, `PqSignatureMissing`, `InvalidHybridKeyLength`. All carry `error_code()` and `http_status()` mappings.

- **`Cargo.toml`** — New `post-quantum` feature flag. When enabled, `HybridSignature::verify` requires non-empty `pq_sig_bytes` for hybrid algorithm certs. Without the flag, the `pq_context` binding commitment is still verified, providing cryptographic evidence of declared algorithm intent.

- **`spec/A1-PROTOCOL.md`** — Formal RFC-style protocol specification covering all wire formats, cryptographic pre-images, the verification algorithm, hybrid signature semantics, security considerations, and a normative conformance requirement list. This document is the authoritative reference for third-party implementations.

### Studio UX — Passport Management & Local AI

#### All Passports at a Glance (`studio/src/js/components/16-passport-all.js`)

- New **Passport Dashboard** tab under Manage. Shows every passport across all namespaces in a single sortable, searchable view. Sorted by urgency by default: expired passports first, then expiring soonest, then valid. Stats bar shows Total / Protected / Expiring / Expired counts at a glance. Inline quick-renew button visible on urgent rows without expanding. Search by namespace name or capability. One-click re-sort by expiry date or name. Backup link at the bottom for laptop-switch or reinstall scenarios.

#### Local AI Connect (`studio/src/js/components/17-local-llm.js`)

- New **Local AI** tab under Advanced. Auto-probes Ollama (port 11434), LM Studio (port 1234), and llama.cpp (port 8000) at load time. Displays detected status and lists all locally-running models. Selecting a provider generates ready-to-paste integration code in four targets: Python (using Ollama SDK or OpenAI-compat), TypeScript, LangChain, and `.mcp.json`. Model picker dropdown auto-populates from the live model list — no manual URL editing required. Works fully offline; no cloud contact.

#### Revoke Confirmation UX (`studio/src/js/components/13-revoke-confirm.js`)

- Revoke flow now explicitly explains reversibility before asking for confirmation. Step 1 shows a recovery path diagram: Revoke → Protect My Agent → New passport (30 sec) → Reconnect. Step 2 (final confirmation) reinforces the 60-second recovery time in green. Reduces hesitation for non-technical users who previously froze at the danger-coded button.

#### Enhanced Mobile (`studio/src/css/09-additions.css`)

- Larger touch targets on mobile: all buttons minimum 36 px tall, sidebar items 44 px tap zone on hamburger, select and text inputs 36 px minimum. iOS zoom-on-focus prevention (`font-size: 16px` on inputs). Two-column stats grid on screens narrower than 480 px.

### Enterprise Integration & Ecosystem Expansion

#### Python SDK — New integrations

- **`sdk/python/a1/llamaindex_tool.py`** — `a1_llamaindex_guard` decorator for LlamaIndex `FunctionTool` and `QueryEngineTool`. Drop-in one-liner for research and RAG agents.
- **`sdk/python/a1/langgraph_tool.py`** — `a1_node` decorator for LangGraph state-graph nodes. Per-node authorization with full audit trail.
- **`sdk/python/a1/semantic_kernel_tool.py`** — `a1_sk_guard` decorator and `DyoloKernelPlugin` wrapper for Microsoft Semantic Kernel. Compatible with `semantic-kernel >= 1.0`.
- **`sdk/python/a1/vault.py`** — `VaultSigner` base class + concrete implementations for AWS KMS, GCP Cloud KMS, HashiCorp Vault Transit, and Azure Key Vault. Removes the single largest CISO objection ("we can't store root keys in a .env file").
- **`sdk/python/a1/siem.py`** — Structured audit exporters: `DatadogSiemExporter`, `SplunkSiemExporter`, `OpenTelemetrySiemExporter`, `JsonlFileExporter`. Plugs directly into existing enterprise SIEM infrastructure.

#### TypeScript SDK — New integrations

- **`sdk/typescript/src/integrations.ts`** — Added `withDyoloLangGraphNode` and `withDyoloSkFunction` guards for LangGraph and Semantic Kernel agent patterns.

#### Compliance pack (`docs/compliance/`)

- **`docs/compliance/soc2-mapping.md`** — SOC 2 Type II Trust Service Criteria mapping. Present to your auditor with no further preparation.
- **`docs/compliance/iso27001-mapping.md`** — ISO/IEC 27001:2022 Annex A control mapping with evidence pointers into the codebase.
- **`docs/compliance/sample-audit-report.md`** — Pre-filled audit report template. Replace bracketed placeholders and submit.

#### Performance documentation

- **`docs/performance-benchmarks.md`** — Concrete criterion bench numbers: `NarrowingMatrix::is_subset_of` at ~150 ns, single-hop auth at ~5 µs, `guard_local` at ~12 µs. Comparison table showing the NarrowingMatrix is ~4000× faster than Groth16 ZK proofs.

#### Wiki

- **`wiki/KMS-Integration.md`** — Full integration guides for AWS KMS, GCP Cloud KMS, HashiCorp Vault Transit, and Azure Key Vault. Includes IAM policies, environment variables, and Rust + Python examples.
- **`wiki/How-It-Compares.md`** — Side-by-side comparison with JWT/OIDC, SPIFFE/SPIRE, and OAuth2 token exchange. Feature table, performance numbers, and recommended adoption patterns.

#### Examples (`examples/integrations/`)

- **`examples/integrations/llamaindex_example.py`** — LlamaIndex research agent with guarded tools.
- **`examples/integrations/langgraph_example.py`** — LangGraph trading agent with per-node authorization.
- **`examples/integrations/semantic_kernel_example.py`** — Semantic Kernel plugin with per-function guards.

#### Bug fixes

- **`studio/src/js/components/00-quickstart.js`** — Fixed QuickStart wizard getting stuck on the passport step. The passport card was rendered with `currentStep !== 'gateway'` which kept it visible even after the user advanced to the Connect step, making it appear as though "Continue" brought them back to step 2. Changed to `currentStep === 'passport'` so the card hides when passed, replaced by a compact "✅ Passport ready" completion badge. Also added a 30-second `AbortController` timeout to the passport creation fetch; if the gateway writes the file but the HTTP response never arrives, the code now polls `/v1/passports/list` to confirm creation instead of hanging forever. Fixed wrong request field name: `out:` → `output_path:` to match the gateway's `IssuePassportRequest` schema.
- **`studio/src/js/components/09-wizard.js`** — Added the same 30-second timeout and poll-on-abort logic to the wizard's passport creation fetch, preventing the "Creating…" button from hanging indefinitely when the gateway crashes after writing but before responding.
- **`tests/integration.rs`** — Removed unused import `ProvableReceipt` (line 693) that generated a compiler warning on every `cargo test` run.
- **`docker/docker-compose.yml`** — Removed `profiles: ["storage"]` from the `redis` service. Redis was gated behind a Docker profile, so `docker compose up -d` failed with `service "gateway" depends on undefined service "redis"` because the gateway's `depends_on: redis` is unconditional. Redis is a required dependency and must always start with the default stack. Also added the missing `redis_data` entry under `volumes:` (the named volume was referenced but not declared, which would cause a second Docker error on first run).
- **`src/crypto.rs`** — Added `#[allow(dead_code)]` to the `KmsSigner` trait. The trait is intentionally public API for external KMS integrations but is not instantiated inside the library itself, causing a spurious compiler warning on every build. The warning is now suppressed at the declaration site.
- **`a1-cli/src/commands/passport.rs`** — Fixed compile error: `passport.cert.expires_at` corrected to `passport.cert.expiration_unix`.
- **`a1-cli/Cargo.toml`** — Added `[features] default = ["wire"] wire = []` so `#[cfg(feature = "wire")]` guards in CLI code compile correctly. Previously the save/load code path was silently dead.
- **All sub-crate `Cargo.toml` files** — Version bumped to `2.8.0` to match workspace root.
- **`a1-gateway/src/main.rs`** — Startup log now correctly reads `v2.8.0`.

#### Version

- Core crate, Python SDK, TypeScript SDK, and all sub-crates: `2.8.0`

---

## [2.0.0] — 2026-05-05

### Identity Layer — Sovereign Passport System

#### Core Rust (`src/`)

- **`src/passport/mod.rs`** — `DyoloPassport`: the new first-class agent identity type.
  Issue once, delegate scoped sub-certs per task, guard with O(1) enforcement.
  Full lifecycle: `issue`, `issue_from_csv`, `issue_sub`, `issue_sub_from_csv`,
  `new_chain`, `guard`, `guard_local`, `save`, `load`.
- **`src/identity/narrowing.rs`** — `NarrowingMatrix`: a 256-bit capability bitmask
  with cryptographic narrowing guarantees. Capability names map deterministically
  to bit positions via Blake3. Narrowing check is pure bitwise AND — O(1), no network,
  no config. Enforces strict subset delegation at both issuance and guard time.
- **`src/identity/receipt.rs`** — `ProvableReceipt`: extends `VerificationReceipt`
  with passport namespace and a Blake3 commitment over the enforced capability mask.
  Archive for audit replay without retaining any secrets.
- **`src/error.rs`** — New `A1Error::PassportNarrowingViolation` variant with
  error code `PASSPORT_NARROWING_VIOLATION` and HTTP status 403. Also added
  `PolicyViolation(String)` and `BatchItemFailed { index, reason }`.
- **`src/identity.rs`** — Promoted to module root; declares `narrowing` and `receipt`
  submodules. All existing exports unchanged.
- **`src/lib.rs`** — Adds `pub mod passport`; re-exports `DyoloPassport`,
  `NarrowingMatrix`, `ProvableReceipt` at crate root for zero-friction adoption.
- **`src/audit.rs`** — New module. `AuditEvent`, `AuditOutcome`, `AuditSink`, `NoopAuditSink`, `LogAuditSink`, `CompositeAuditSink`, and async `AsyncAuditSink`.
- **`src/policy.rs`** — New module. `DelegationPolicy`, `CapabilitySet`, and `PolicySet`.
- **`src/chain.rs`** — `DyoloChain::authorize_with_options`, `authorize_batch`, `authorize_async_with_options`.
- **`src/cert.rs`** — New `CertBundle` type for batch cert issuance in a single round-trip.
- **`src/registry.rs`** — `NonceStore::try_consume` atomically checks and marks a nonce in a single critical section.

#### CLI (`a1-cli`)

- **`a1-cli/src/commands/passport.rs`** — New `passport` subcommand with three sub-subcommands:
  - `passport issue` — generate a key and issue a root passport in one command.
  - `passport inspect` — print all passport fields from a JSON file.
  - `passport sub` — issue a time-limited sub-delegation cert for a specific agent.
- **`a1-cli/src/commands/migrate.rs`** — New `migrate` subcommand. `a1 migrate --database-url postgres://...` applies the full `a1-pg` schema migration in one command.
- **`a1-cli/src/main.rs`** — Adds `Command::Passport` and `Command::Migrate` variants.
- **`a1-cli/src/commands/mod.rs`** — Declares `pub mod passport` and `pub mod migrate`.

#### Python SDK (`sdk/python`)

- **`sdk/python/a1/passport.py`** — `PassportClient`, `PassportReceipt`,
  `PassportError`, and `a1_guard` decorator. Works with any Python callable —
  async or sync. One decorator line protects any AI agent tool.
- **`sdk/python/a1/__init__.py`** — Exports all new passport symbols.
- **`sdk/python/tests/test_client.py`** — 16 pytest tests covering sync and async `A1Client` using `respx` to mock `httpx`.
- **`sdk/python/pyproject.toml`** — Added `[tool.pytest.ini_options]`: `asyncio_mode = "auto"` and `testpaths = ["tests"]`.

#### TypeScript SDK (`sdk/typescript`)

- **`sdk/typescript/src/passport.ts`** — `PassportClient`, `PassportReceipt`,
  `PassportError`, `withA1Passport` higher-order function, and `PassportGuard`
  Stage-3 class-method decorator. Exported under the `"a1/passport"` subpath.
- **`sdk/typescript/package.json`** — Adds `"./passport"` export map entry.
- `authorizeBatch`, `revokeCertsBatch`, `wellKnown`, `AuthorizeOptions.requestId`.
- All error responses now expose `code` (machine-readable `error_code`).
- **`sdk/typescript/tests/integrations.test.ts`** — Full Jest test suite for all six integration builders.
- **`sdk/typescript/tsconfig.test.json`** — Dedicated TypeScript config for Jest.

#### Go SDK (`sdk/go`)

- **`sdk/go/a1/passport.go`** — `PassportReceipt`, `PassportError`, `PassportOptions`,
  `Client.AuthorizePassport`, and generic `WithPassport[T, R]` guard function.
  Zero new dependencies; uses only stdlib `reflect` and `encoding/json`.
- **`sdk/go/go.mod`** — Module path `github.com/dyologician/a1/sdk/go`. No external dependencies.
- **`sdk/go/a1/client_test.go`** — Comprehensive unit test suite using `net/http/httptest`. Covers `New` option variants, `WellKnown`, `Authorize`, `AuthorizeBatch`, `RevokeCert`, `RevokeCertsBatch`, `InspectCert`, `A1Error` formatting, and non-JSON gateway error handling.

#### Gateway (`a1-gateway`)

- Rate limiting via `governor`. Configurable via `A1_RATE_LIMIT_RPS` (default: 500 req/sec).
- `GET /.well-known/a1-configuration` — OIDC-style discovery document.
- `POST /v1/authorize/batch` — Batch authorize multiple intents atomically.
- `POST /v1/cert/revoke-batch` — CRL-style batch revocation.
- Error responses now include `error_code` (machine-readable) alongside `error` (human-readable).

#### Wire schema

- **`wire/schema.json`** — Complete JSON Schema (Draft 2020-12) for `SignedChain` and `VerifiedToken`, including all nested types with exact regex constraints on hex-encoded fields.
- **`sdk/python/a1/py.typed`** — Marker file correctly empty per PEP 561.

### Backward Compatibility

All v1.0.0 APIs are unchanged. Wire format, JSON schema, cert serialization,
chain verification logic, existing tests — all untouched. v2.0.0 adds new types
alongside the existing ones with zero breaking changes.

---

## [1.0.0] — 2025-05-03

Initial release.

- `DyoloChain` with Ed25519 batch signature verification.
- `DelegationCert` with Merkle sub-scope proofs and temporal monotonicity.
- `MemoryNonceStore` and `MemoryRevocationStore` with sharded locks and bloom filter fast-path.
- `a1-redis` — async Redis storage backends.
- `a1-pg` — async PostgreSQL storage backends.
- `a1-gateway` — REST sidecar with cert issuance, single authorization, and token verification.
- Python SDK with LangChain and OpenAI tool adapters.
- TypeScript SDK with LangChain.js and OpenAI Agents adapters.
- Zero-knowledge rollup via RISC Zero (feature-gated).
