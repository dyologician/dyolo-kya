# A1 Capabilities Reference — v2.8.0

This document describes every capability A1 provides, what it does, and why it matters. It is written for everyone: from a non-technical decision-maker evaluating A1 for their organization, to a senior security engineer integrating A1 into a regulated production system.

---

## How to read this document

Each section follows the same format:

- **What it is** — a plain-English description for anyone.
- **What it solves** — the specific problem this feature addresses.
- **How it works** — the technical detail for engineers.
- **Key properties** — what you can rely on.
- **Relevant files** — where to find the implementation.

---

## Core Capability: Agent Identity (DyoloPassport)

**What it is.** A long-lived, cryptographically signed identity for an AI agent that encodes exactly what the agent is allowed to do.

**What it solves.** Without a passport, an agent is anonymous. It might have a JWT or an API key, but nothing in that token proves *which human* said it was allowed to do *this action*, with *these capabilities*, *right now*.

**How it works.**
- A human generates an Ed25519 keypair and issues a passport for an agent.
- The passport records a named capability set (e.g., `trade.equity`, `portfolio.read`), encoded as a 256-bit bitmask via Blake3.
- The passport is a self-signed root certificate valid for a configurable duration (days, months, years).
- All sub-delegations from this passport are mathematically constrained to stay within its capability set.
- Passports are saved as JSON and can be loaded from a file path, a secrets manager, or a KMS backend.

**Key properties.**
- Offline — no network call required to verify a passport-issued chain.
- Immutable — the capability mask is committed in the certificate signature; tampering invalidates the signature.
- Portable — JSON file, storable in a vault, HSM, or encrypted object store.

**Relevant files.** `src/passport/mod.rs`, `src/identity/narrowing.rs`

---

## Core Capability: Capability Enforcement (NarrowingMatrix)

**What it is.** A 256-bit bitmask that enforces capability subset relationships at O(1) speed.

**What it solves.** In every other delegation system, capability checking involves string comparison, database lookups, or policy engine evaluation. A1 reduces it to a single CPU instruction sequence — eight 64-bit AND operations.

**How it works.**
- Each capability name (e.g., `trade.equity`) maps to a bit position via Blake3 with domain prefix `dyolo::narrowing::v1`.
- A sub-delegation's mask must be a bitwise subset of the parent: `sub & parent == sub`.
- This is checked in one 32-byte AND operation on modern hardware, regardless of chain depth.
- `enforce_narrowing()` returns `PassportNarrowingViolation` if the sub mask exceeds the parent.
- `from_capabilities(&["trade.equity", "portfolio.read"])` is all the configuration needed.

**Key properties.**
- O(1) regardless of capability count.
- No network call, no external registry, no configuration file at verification time.
- Collision-resistant for typical deployments (< 0.1% collision probability for 20 capabilities against 256 slots). `CapabilityRegistry` provides collision-free explicit bit assignment for larger deployments.

**Relevant files.** `src/identity/narrowing.rs`

---

## Core Capability: Collision-Free Capability Registry (CapabilityRegistry)

**What it is.** An explicit name-to-bit assignment registry for deployments that need more than ~100 distinct capability names, or that require zero collision probability across a large shared capability namespace.

**What it solves.** `NarrowingMatrix` maps capability names to bit positions via Blake3 hashing. For most deployments (fewer than 20–30 distinct capabilities), the collision probability is negligible. For large enterprises running hundreds of distinct capability types, the probability of two capability names sharing a bit becomes material. `CapabilityRegistry` eliminates this by assigning bit positions explicitly and statically, with a collision check on registration.

**How it works.**
- `CapabilityRegistry::new()` creates an empty registry.
- `registry.register("trade.equity", 0)` assigns capability `"trade.equity"` to bit 0.
- `registry.build_mask(&["trade.equity", "portfolio.read"])` produces a `NarrowingMatrix` using only the explicitly assigned bits.
- Registration fails at startup if two names map to the same bit — no silent collisions at runtime.
- The registry is passed to `A1Context` and used everywhere `NarrowingMatrix::from_capabilities` would otherwise be called.

**Key properties.**
- Zero collision probability — all bit assignments are explicit.
- O(1) lookup — registry is a static hash map.
- Startup-time failure — misconfigured registries fail fast, not during authorization.

**Relevant files.** `src/registry.rs`

---

## Core Capability: Delegation Chain Verification (DyoloChain)

**What it is.** A verifiable chain of signed delegation certificates from a root authority to an executing agent.

**What it solves.** In multi-agent systems, Agent A delegates to Agent B which delegates to Agent C. Without chain verification, C has no proof that its authorization traces back to the original human intent. A1 makes this chain cryptographically verifiable at every hop.

**How it works.** `DyoloChain::authorize` verifies the entire chain in one pass:

1. **Namespace check** — Namespace must match; mismatch fails before any crypto.
2. **Revocation fast-path** — Principal cert fingerprint checked against `RevocationStore`.
3. **Chain traversal** — For each cert: Ed25519 signature verification, temporal validity, depth budget, SubScopeProof Merkle containment, per-cert revocation.
4. **Intent authorization** — Intent hash verified against terminal cert's scope tree.
5. **Nonce consumption** — `NonceStore::consume(intent_nonce)` — atomic, replay-safe.
6. **Audit emission** — Event emitted to all registered `AuditSink` instances.
7. **Receipt production** — `AuthorizedAction` with `VerificationReceipt` returned.

**Key properties.**
- Single-pass, linear in chain length.
- Ed25519 batch verification — amortized cost per cert drops with chain length.
- Atomic nonce consumption — either the whole batch of nonces is consumed or none are.
- Namespace isolation — a chain scoped to `tenant-a` cannot authorize under `tenant-b`.

**Relevant files.** `src/chain.rs`, `src/cert.rs`

---

## Core Capability: Post-Quantum Hybrid Signatures

**What it is.** A wire format that supports classical Ed25519 today and ML-DSA (CRYSTALS-Dilithium) hybrid signatures without any migration required.

**What it solves.** Ed25519 is secure today but will be broken by sufficiently powerful quantum computers. Any system built today without a quantum migration path is a future liability. A1's wire format supports the migration without breaking existing chains or requiring a hard cutover.

**How it works.**
- Every cert carries a `SignatureAlgorithm` tag:
  - `Ed25519 = 1` — classical, default for all v2.8.0 deployments.
  - `HybridMlDsa44Ed25519 = 2` — ML-DSA-44 + Ed25519, NIST Level 2 (128-bit post-quantum).
  - `HybridMlDsa65Ed25519 = 3` — ML-DSA-65 + Ed25519, NIST Level 3 (192-bit post-quantum, recommended for financial and government).
- `HybridSignature` carries both components. Both must pass for the cert to be accepted.
- A `pq_context` field carries a Blake3 commitment over the algorithm ID, message, and PQ signature bytes. This commitment is verified even when the `post-quantum` feature is disabled — providing cryptographic evidence of declared algorithm intent.
- A chain can migrate monotonically: Ed25519 root certs followed by hybrid leaf certs. Reverse order is rejected.

**Key properties.**
- Zero breaking changes. Ed25519 chains are fully valid and verified forever.
- Wire-stable: the hybrid cert format is fixed as of v2.8.0.
- Algorithm negotiation via `negotiate_algorithm()` — picks the strongest algorithm the build supports.
- Key sizes: ML-DSA-44 public key = 1312 bytes, signature = 2420 bytes. ML-DSA-65 public key = 1952 bytes, signature = 3309 bytes.

**Feature flag.** `features = ["post-quantum"]`

**Relevant files.** `src/hybrid.rs`

---

## Core Capability: Provable Receipts (ProvableReceipt)

**What it is.** An extended authorization receipt that permanently records which agent acted, under which capability, authorized by which passport, with a cryptographic commitment to the enforced scope.

**What it solves.** After an authorization succeeds, you need proof — not just a log entry that an action happened, but a cryptographic record that proves exactly what was authorized. `ProvableReceipt` is that proof.

**What it contains.**
- Chain depth (number of delegation hops)
- Chain fingerprint (Blake3 over all certs — changes if any cert is altered)
- Authorized intent hash
- Passport namespace
- Enforced capability mask (hex)
- Blake3 commitment over the capability mask (proves the mask at authorization time)
- Optional `ProvenanceRoot` (Merkle commitment to the agent's full reasoning trace)

**Reasoning trace.** Agents can record a step-by-step trace of thoughts, tool calls, and observations as they execute. `ReasoningTrace::finalize()` produces a Merkle root bound to the chain fingerprint. Individual steps can be selectively disclosed to auditors via `ProvenanceStepProof` — proving one step without revealing the full trace.

**Relevant files.** `src/identity/receipt.rs`, `src/provenance.rs`

---

## Core Capability: Zero-Knowledge Chain Commitments (ZkChainCommitment)

**What it is.** A compact commitment that proves an authorization occurred without revealing the delegation chain.

**What it solves.** Shipping a full delegation chain to every verifier — a blockchain, a downstream service, a mobile client — is expensive and may expose confidential organizational structure. A commitment lets you distribute the proof without the chain.

**How it works.**
- `ZkChainCommitment::seal(chain, intent, narrowing, timestamp, authority)` produces a 32-byte commitment: `Blake3("a1::dyolo::zk::commit::v2.8.0" ‖ chain_fingerprint ‖ intent ‖ narrowing_commitment ‖ timestamp)`, signed by the sealing authority.
- `commitment.verify_commitment(narrowing, now, max_age)` checks commitment integrity, freshness, and authority signature. O(1) regardless of original chain depth.
- `commitment.with_zk_proof(bytes)` upgrades the commitment to `ZkProofMode::ExternalZkvm` by attaching a real zkVM proof (RISC Zero, Jolt, SP1). The wire format is identical.
- `anchor_hash(commitment)` produces a 32-byte value to submit to a smart contract or transparency log.

**Feature flag.** `features = ["zk"]`

**Relevant files.** `src/zk.rs`, `src/zk_guest/src/main.rs` (RISC Zero guest program)

---

## Core Capability: DID and W3C Verifiable Credentials

**What it is.** Every agent identity in A1 maps to a W3C Decentralized Identifier (DID) and can issue W3C Verifiable Credentials (VCs).

**What it solves.** Regulated industries and cross-organization deployments increasingly require credentials that conform to open W3C standards. A1 bridges its internal identity model to W3C DID and VC formats so that A1-issued credentials can be verified by any W3C-compliant toolchain.

**How it works.**
- `AgentDid::from_pk(verifying_key)` derives a `did:key:` URI from an Ed25519 public key.
- `DidDocument` is the W3C-conformant JSON-LD representation, resolvable via the gateway's `GET /v1/did/:pk_hex` endpoint.
- `VerifiableCredential::issue(claims, issuer_identity)` signs a set of claims as a W3C VC.
- `VerifiableCredential::verify(vc_json)` verifies the signature and extracts claims.
- The gateway exposes `POST /v1/vc/issue` and `POST /v1/vc/verify` for language-agnostic access.

**Feature flag.** `features = ["did"]`

**Relevant files.** `src/did.rs`, `a1-gateway/src/routes/did.rs`

---

## Core Capability: Policy Engine (PolicySet)

**What it is.** A YAML-driven policy layer that imposes additional constraints on delegation beyond the mathematical narrowing.

**What it solves.** Some constraints cannot be expressed as capability bitmasks — for example, "the sub-cert TTL must not exceed 1 hour" or "the `trade.equity` capability requires a minimum chain depth of 2". `PolicySet` enforces these organizational rules declaratively.

**How it works.**
- A YAML policy file is loaded via `PolicySet::from_yaml(path)` or the `a1 policy -f policy.yaml` CLI command.
- The `PolicySet` is attached to an `A1Context` via `A1Context::builder().policy(policy_set)`.
- During `DyoloChain::authorize`, if a `PolicySet` is present, it is evaluated after signature verification and before receipt production.
- Policy violations return a `PolicyViolation` error with the violated rule name.

**Example policy:**

```yaml
rules:
  - name: max-ttl-trade
    capability: trade.equity
    max_ttl_seconds: 3600

  - name: min-depth-trade
    capability: trade.equity
    min_chain_depth: 2

  - name: allowed-namespaces
    namespaces:
      - trading-prod
      - trading-staging
```

**Feature flag.** `features = ["policy-yaml"]`

**Relevant files.** `src/policy.rs`, `a1-cli/src/commands/policy.rs`

---

## Core Capability: Production Storage Backends

### a1-redis (Redis-backed stores)

- **`AsyncRevocationStore`** — fingerprint deny-list backed by Redis.
- **`AsyncNonceStore`** — nonce tracking backed by Redis.
- Nonce consumption uses `SET NX PX` — atomic, one round-trip, no TOCTOU possible.
- Batch nonce consumption uses a Lua script that executes check + set atomically in a single Redis command.
- Bloom filter fast-path: O(1) negative lookups for revocation without a network round-trip.
- Configurable key namespaces and TTLs. Connection pooling via `deadpool-redis`.

**Install:**

```toml
[dependencies]
a1-redis = "2.8"
```

### a1-pg (PostgreSQL-backed stores)

- **`AsyncRevocationStore`** — fingerprint deny-list backed by Postgres.
- **`AsyncNonceStore`** — nonce tracking backed by Postgres.
- Nonce consumption: `INSERT INTO nonces (nonce) VALUES ($1) ON CONFLICT DO NOTHING` — single-roundtrip atomic.
- Batch nonce consumption: SERIALIZABLE transaction with automatic retry on serialization conflicts (up to 3×).
- Multi-tenant: `NamespacedRevocationStore` scopes all keys by tenant ID.
- Schema migration: `PgRevocationStore::run_migration()` or `a1 migrate` CLI command.

**Install:**

```toml
[dependencies]
a1-pg = "2.8"
```

**Relevant files.** `a1-redis/src/lib.rs`, `a1-pg/src/lib.rs`

---

## Core Capability: Enterprise KMS Integration (VaultSigner)

**What it is.** A signing backend that calls a remote KMS (HashiCorp Vault Transit, AWS KMS, GCP KMS, Azure Key Vault) for all signing operations. Root key material never touches application memory.

**What it solves.** Regulations and security standards (FIPS 140-2, PCI-DSS, SOC 2, ISO 27001) require that cryptographic private keys be stored in hardware security modules or approved KMS services, not in application memory or environment variables.

**How it works.**
- `VaultSigner` implements `AsyncSigner`. Its `sign_message` method forwards the payload to the KMS and returns the signature.
- The `context` field in the KMS request binds the operation to the A1 domain — visible in enterprise KMS audit logs as a permanent record.
- Zero KMS calls at verification time — verification uses only the public key embedded in the cert.

**Supported backends:**

| Backend | Class | Python extra |
|---|---|---|
| AWS KMS | `AwsKmsSigner` | `pip install "a1[vault-aws]"` |
| GCP KMS | `GcpKmsSigner` | `pip install "a1[vault-gcp]"` |
| HashiCorp Vault Transit | `HashiCorpVaultSigner` | `pip install "a1[vault-hashicorp]"` |
| Azure Key Vault | `AzureKeyVaultSigner` | `pip install "a1[vault-azure]"` |
| Local file (dev only) | `LocalFileSigner` | included in base |

**Feature flag (Rust).** `features = ["async"]`

**Relevant files.** `src/identity.rs`, `sdk/python/a1/vault.py`

---

## Core Capability: SIEM Integration (AuditSink)

**What it is.** A composable audit event system that forwards every authorization event to your existing security information and event management (SIEM) infrastructure.

**What it solves.** Enterprise security teams require that every agent authorization event appear in their SIEM within seconds — for anomaly detection, compliance reporting, and incident response. A1 does this automatically with zero configuration beyond specifying your SIEM endpoint.

**Supported exporters:**

| Exporter | Python class | Notes |
|---|---|---|
| Datadog Logs | `DatadogLogExporter` | API key authentication |
| Splunk HEC | `SplunkHecExporter` | HEC token authentication |
| OpenTelemetry | `OpenTelemetryExporter` | OTLP/HTTP endpoint |
| NDJSON file | `JsonlFileExporter` | Local file or stdout |
| Composite | `CompositeExporter` | Fan-out to multiple destinations |

Every exported event includes: timestamp, chain fingerprint, namespace, capability mask, authorized intent hash, chain depth, and agent public key.

**Python extras:**

```bash
pip install "a1[siem-datadog]"   # Datadog
pip install "a1[siem-splunk]"    # Splunk
pip install "a1[siem-otel]"      # OpenTelemetry
```

**Relevant files.** `src/audit.rs`, `sdk/python/a1/siem.py`

---

## Core Capability: Self-Hosted Gateway

**What it is.** A language-agnostic REST service that exposes all A1 operations over HTTP/JSON. Any service in any programming language can authorize AI agent actions without a Rust dependency.

**How to start:**

```bash
docker compose up -d
```

**Endpoints:**

| Method | Path | Description |
|---|---|---|
| `POST` | `/v1/authorize` | Authorize a single intent against a delegation chain |
| `POST` | `/v1/authorize/batch` | Authorize up to 256 intents in one request |
| `POST` | `/v1/passport/authorize` | Passport-aware authorization (metrics-separated) |
| `POST` | `/v1/cert/issue` | Issue a delegation certificate (admin) |
| `POST` | `/v1/cert/issue-batch` | Issue multiple certs atomically (admin) |
| `POST` | `/v1/cert/revoke` | Revoke a certificate by fingerprint (admin) |
| `POST` | `/v1/cert/revoke-batch` | Bulk revocation (admin) |
| `GET`  | `/v1/cert/:fp` | Inspect revocation status by fingerprint |
| `POST` | `/v1/token/verify` | Verify a `VerifiedToken` HMAC receipt |
| `GET`  | `/v1/did/gateway` | Gateway's own DID Document |
| `GET`  | `/v1/did/:pk_hex` | Resolve any Ed25519 key to a DID Document |
| `POST` | `/v1/vc/issue` | Issue a W3C Verifiable Credential (admin) |
| `POST` | `/v1/vc/verify` | Verify a W3C VC and extract claims |
| `POST` | `/v1/swarm/create` | Create a new agent swarm (admin) |
| `POST` | `/v1/swarm/member/add` | Add an agent to a swarm (admin) |
| `POST` | `/v1/swarm/member/remove` | Remove an agent from a swarm (admin) |
| `GET`  | `/v1/swarm/:swarm_id/members` | List active swarm members |
| `GET`  | `/v1/governance/policy` | Return the active delegation policy |
| `POST` | `/v1/governance/approval/verify` | Verify a governance approval record |
| `POST` | `/v1/governance/audit-report` | Generate a structured audit report (admin) |
| `POST` | `/v1/negotiate` | Agent-to-agent capability negotiation |
| `POST` | `/v1/jwt/exchange` | Exchange a JWT for a scoped cert (admin) |
| `POST` | `/v1/anchor` | Anchor a ZkChainCommitment on-chain |
| `GET`  | `/v1/passports/list` | List passport files |
| `POST` | `/v1/passports/issue` | Issue a new passport |
| `POST` | `/v1/passports/renew` | Re-issue a passport with a new TTL |
| `GET`  | `/v1/passports/read` | Read a passport by namespace |
| `POST` | `/v1/passports/restore` | Restore passports from a backup |
| `POST` | `/v1/passports/revoke-by-namespace` | Revoke all certs under a namespace |
| `GET`  | `/v1/tenant/info` | Active tenant context |
| `GET`  | `/v1/tenant/config` | Per-tenant capability allowlist |
| `GET`  | `/v1/webhook/status` | Webhook delivery status |
| `POST` | `/v1/webhook/test` | Send a test webhook event (admin) |
| `POST` | `/v1/debug/explain-error` | Translate an A1 error code to plain English |
| `GET`  | `/healthz` | Health check |
| `GET`  | `/.well-known/a1-configuration` | Service discovery document |
| `GET`  | `/.well-known/schema.json` | Wire format JSON schema |

**Production features:**
- Per-IP rate limiting (configurable via `A1_RATE_LIMIT_RPS`)
- Admin endpoint protection via `A1_ADMIN_SECRET` bearer token
- CORS via `A1_CORS_ALLOWED_ORIGIN`
- Idempotency keys on cert issuance (5-minute TTL)
- Structured tracing via `tower-http` + OpenTelemetry
- Graceful shutdown (SIGTERM + Ctrl-C)
- Storage: in-memory (dev), Redis (`A1_REDIS_URL`), Postgres (`A1_PG_URL`)

**Relevant files.** `a1-gateway/src/`

---

## Core Capability: Multi-Agent Swarm Coordination

**What it is.** A lightweight coordination layer that lets groups of A1-authorized agents register themselves, discover peers, and coordinate actions while keeping every action individually authorized.

**What it solves.** In swarm deployments, tens or hundreds of agents need to find each other and self-organize without a central coordinator. Each agent still carries its own passport and each action is still individually authorized — swarm membership does not grant additional capabilities.

**How it works.**
- `SwarmRegistry` maintains a list of `SwarmMember` records, each containing an agent's public key and passport fingerprint.
- `SwarmMember::join(registry, passport)` registers an agent.
- `SwarmRegistry::list()` returns active members with their last-seen timestamps.
- The gateway exposes swarm endpoints at `/v1/swarm/*`.

**Feature flag.** `features = ["swarm"]`

**Relevant files.** `src/swarm.rs`, `sdk/python/a1/swarm.py`, `sdk/typescript/src/swarm.ts`

---

## Core Capability: On-Chain Governance

**What it is.** A module for recording governance votes and proposals on-chain, with each vote cryptographically tied to the voter's A1 identity.

**What it solves.** DAO and on-chain governance systems need to verify that a vote came from an authorized agent identity, not just an anonymous wallet. A1 binds votes to the passport identity, giving governance systems cryptographic proof of who voted.

**Feature flag.** `features = ["governance"]`

**Relevant files.** `src/governance.rs`, `a1-gateway/src/routes/governance.rs`

---

## Core Capability: Algorithm Negotiation

**What it is.** A protocol-level handshake that selects the strongest signature algorithm both parties support.

**How it works.** `negotiate_algorithm(local_capabilities, remote_capabilities)` returns:
- `HybridMlDsa65Ed25519` if both sides support it (highest security).
- `HybridMlDsa44Ed25519` if both support this level.
- `Ed25519` if only classical support is present.

This allows a gradual rollout of post-quantum signing without requiring all agents to upgrade simultaneously.

**Feature flag.** `features = ["negotiate"]`

**Relevant files.** `src/negotiate.rs`

---

## Core Capability: Framework SDKs

### Python SDK

**Install:**

```bash
pip install a1                    # Base client
pip install "a1[langchain]"       # LangChain integration
pip install "a1[langgraph]"       # LangGraph integration
pip install "a1[llamaindex]"      # LlamaIndex integration
pip install "a1[autogen]"         # AutoGen integration
pip install "a1[crewai]"          # CrewAI integration
pip install "a1[semantic-kernel]" # Semantic Kernel integration
pip install "a1[openai]"          # OpenAI Agents SDK integration
pip install "a1[vault-aws]"       # AWS KMS signing
pip install "a1[vault-gcp]"       # GCP KMS signing
pip install "a1[vault-hashicorp]" # HashiCorp Vault signing
pip install "a1[vault-azure]"     # Azure Key Vault signing
pip install "a1[siem-datadog]"    # Datadog audit export
pip install "a1[siem-splunk]"     # Splunk audit export
pip install "a1[siem-otel]"       # OpenTelemetry audit export
pip install "a1[all]"             # All framework integrations
```

**Key classes:**
- `A1Client` — base HTTP client for the gateway.
- `AsyncA1Client` — async variant of the HTTP client.
- `PassportClient` — manages passport file, chain building, and gateway calls.
- `@a1_guard(client, capability)` — decorator for any sync or async function.
- `VaultSigner` — KMS signing backend base class.
- `A1Tracer` — wraps any `PassportClient` or function to emit OTEL spans (`instrument_passport_client`, `trace_capability` decorator). Requires `pip install "a1[siem-otel]"`.
- `protect` / `inject_passport` / `a1_context` — ASGI/WSGI middleware helpers. `A1Context` carries the verified passport through a request. `set_context` / `get_context` for manual propagation. `MiddlewareError` raised on unauthorized requests.
- Framework tools — `A1AuthorizationTool` (LangChain, CrewAI), `a1_node` (LangGraph), `a1_llamaindex_tool` (LlamaIndex), `build_a1_function_tool` (AutoGen), `a1_sk_function` (Semantic Kernel).

### TypeScript / Node.js SDK

**Install:**

```bash
npm install a1
```

**Key exports (`"a1"`):**
- `A1Client` — typed HTTP client.
- `PassportClient` — passport management with full TypeScript type safety.
- `withA1Passport(fn, options)` — higher-order function guard.
- `PassportGuard` — class decorator variant.
- `buildLangChainA1Tool`, `withDyoloLangGraphNode`, `withDyoloSkFunction` — framework integrations.

**Key exports (`"a1/middleware"`):**
- `A1Middleware` — class for Express/Hono/Fastify middleware integration.
- `exchangeJwt(options)` — exchange a JWKS-verified JWT for a scoped `DelegationCert`.
- `verifyWebhookSignature(event, secret)` — verify A1 gateway webhook payloads.
- `JwtExchangeOptions`, `JwtExchangeResult`, `WebhookEvent` — typed interfaces.

**Supports:** Node.js 18+, ESM and CJS, full TypeScript strict mode.

### Go SDK

**Install:**

```bash
go get github.com/dyologician/a1/sdk/go/a1/kya
```

**Key exports:**
- `A1Client` — HTTP client for the gateway.
- `WithPassport[T, R]` — generic guard function.
- `PassportClient` — passport lifecycle management.

**Supports:** Go 1.21+, generics, net/http and gRPC middleware patterns.

---

## Core Capability: CLI (`a1-cli`)

**Install:**

```bash
cargo install a1-cli
```

**All commands:**

```
a1 keygen                        Generate an Ed25519 keypair
a1 passport issue                Issue a new root passport
a1 passport inspect <file>       Inspect a passport file (namespace, caps, expiry, status)
a1 passport sub                  Issue a scoped sub-delegation cert
a1 issue                         Issue a delegation cert via the gateway
a1 revoke <fingerprint>          Revoke a cert by fingerprint
a1 revoke-batch <fp> [<fp>...]   Bulk revocation
a1 inspect <fingerprint>         Check revocation status of a cert
a1 verify <token-file>           Verify a VerifiedToken HMAC receipt
a1 decode <cert-file>            Print all fields of a cert for debugging
a1 policy -f policy.yaml         Apply a YAML policy to the gateway
a1 migrate                       Run Postgres schema migration
a1 completion <shell>            Generate shell completions (bash, fish, zsh, powershell)
```

**Relevant files.** `a1-cli/src/`

---

## Core Capability: C FFI

**What it is.** A C ABI export that lets you embed A1's core authorization engine in Python (ctypes/cffi), Go (cgo), Java (JNA/JNI), Node.js (N-API), or any language with C interop.

**When to use it.** Use the REST gateway for most use cases. Use the FFI when you need sub-millisecond authorization latency without a network hop and cannot use Rust natively.

**Key exported functions:**

```c
a1_context_t* a1_context_new(void);
void          a1_context_free(a1_context_t* ctx);
int           a1_authorize(a1_context_t* ctx,
                           const uint8_t* chain_json, size_t chain_json_len,
                           const uint8_t* intent_json, size_t intent_json_len,
                           uint8_t* receipt_out, size_t* receipt_len);
```

All returned buffers are heap-allocated and must be freed with `a1_free_buf`. Thread-safe: multiple calls with different `a1_context_t*` instances can run concurrently without locking.

**Feature flag.** `features = ["ffi"]`

**Relevant files.** `src/ffi.rs`, `cbindgen.toml`

---

## Core Capability: CBOR Wire Encoding

**What it is.** Binary CBOR serialization for `SignedChain` and related wire types.

**When to use it.** CBOR encoding reduces wire size by approximately 30–40% compared to JSON. Use it for IoT deployments, edge devices, or bandwidth-constrained environments.

**Feature flag.** `features = ["cbor"]`

---

## Core Capability: JSON Schema Export

**What it is.** Machine-generated JSON Schema for all wire types, served at `GET /.well-known/schema.json`.

**When to use it.** Use the schema to validate A1 payloads in languages without a native SDK, to generate client code with OpenAPI tooling, or to integrate A1 types into an API documentation system.

**Feature flag.** `features = ["schema"]`

---

## Core Capability: A1 Studio (Web Dashboard)

**What it is.** A browser-based visual dashboard built into the gateway. Available at `http://localhost:8080/studio` with zero configuration — no separate install, no extra dependencies.

**What it solves.** Not everyone on a team writes code. Compliance officers, security leads, and product managers need visibility into what agents are authorized to do, and the ability to issue or revoke access, without touching a terminal. Studio gives them that access safely.

**Who it's for.**

| Role | What they use it for |
|---|---|
| Non-technical team members | Issue passports, inspect delegation chains, revoke access |
| Developers | Quickstart wizard, MCP tester, direct connect, integration code snippets |
| Security / compliance | Trust layer visualization, audit log, governance review |
| Enterprise teams | Enterprise settings, tenant config, SIEM and webhook setup |

**Included components.**

| Tab | Component | Description |
|---|---|---|
| Get Started | Quickstart wizard | Step-by-step guided setup for first-time users. Generates a working passport and gateway config in minutes. |
| Get Started | How It Works | Plain-English visual explanation of the delegation model for non-technical stakeholders. |
| Developers | Developer Tabs | Code generator for Python, TypeScript, Go, and Rust. Produces ready-to-paste snippets for every A1 operation. |
| Developers | Direct Connect | Connect Studio to a running agent over HTTP or WebSocket. Live capability inspection and test-fire tool. |
| Developers | AI Integration | Framework-specific integration guides: LangChain, LangGraph, LlamaIndex, AutoGen, CrewAI, Semantic Kernel, OpenAI Agents. |
| Developers | MCP Tester | Live MCP tool tester. Fire `authorize`, `cert-issue`, `revoke`, and `passport-check` directly from the browser against the running gateway. |
| Developers | Error Explainer | Paste any A1 error code or JSON error body and get a plain-English explanation and fix suggestion. |
| Passports | My Passports | List, inspect, quick-renew, and revoke individual passports by namespace. |
| Passports | Passport Dashboard | **All passports across all namespaces in a single view.** Sorted by urgency: expired first, then expiring soon, then valid. Stats bar shows Total / Protected / Expiring / Expired. Search by namespace or capability. Inline quick-renew without expanding. |
| Passports | Passport Vault | Export, backup, and restore passport files. |
| Agents | Connect Agents | Register remote agents and view their live status. |
| Agents | Agent Lifecycle | Visualize the lifecycle of an agent: issuance → delegation → authorization → receipt → revocation. |
| Agents | Guided Next | Contextual next-step prompts based on current setup state. |
| Security | Trust Layer | Interactive visualization of the full delegation chain from human principal through every agent hop to the final receipt. |
| Security | Security Diagram | Cryptographic security model diagram with Ed25519 / Blake3 / ML-DSA annotations. |
| Security | Revoke (with confirmation) | Safe revoke flow with a recovery-path diagram before the final confirmation, reducing accidental revocation for non-technical users. |
| Advanced | Local AI | **Connect to locally-running AI models without a cloud key.** Auto-detects Ollama (port 11434), LM Studio (port 1234), and llama.cpp (port 8000). Lists running models. Generates integration code for Python, TypeScript, LangChain, and `.mcp.json` in one click. Fully offline. |
| Advanced | AI Assistant | Built-in AI assistant (requires `A1_AI_KEY` env var). Answers questions about A1, explains error messages, and generates code. Powered by `/v1/ai/chat` proxy. |
| Enterprise | Enterprise Settings | Multi-tenant config, webhook setup, SIEM endpoint, rate limit overrides. |

**Key properties.**
- Zero install — served directly by the gateway at `/studio`.
- Works without any environment variables in development mode.
- Mobile-friendly — responsive layout with 36 px minimum touch targets, iOS zoom-prevention, two-column stats grid on narrow screens.
- No auth required for read-only views; write operations (issue, revoke) require `A1_ADMIN_SECRET` when set.

**Relevant files.** `studio/src/`, `a1-gateway/src/routes/studio.rs`

---

## Performance Characteristics

| Operation | Throughput | Notes |
|---|---|---|
| `NarrowingMatrix::is_subset_of` | >1,000,000,000/s | 32-byte AND, no allocation |
| `NarrowingMatrix::from_capabilities` | ~5,000,000/s | Per capability, Blake3 |
| Single-hop chain authorization (memory store) | ~80,000/s | Single core, Ed25519 batch |
| 10-hop chain authorization (memory store) | ~25,000/s | Single core |
| `DelegationCert::fingerprint` | ~10,000,000/s | Blake3 over 64 bytes |
| `VerifiableCredential::verify` | ~50,000/s | Ed25519 verify + Blake3 |
| `ZkChainCommitment::verify_commitment` | ~200,000/s | Blake3 + Ed25519 verify |
| Gateway `/v1/authorize` (Redis) | ~15,000 req/s | 8-core, Redis on localhost |

All numbers are approximate. Run `cargo bench` for exact figures on your target hardware.

---

## Feature Flags Summary

| Flag | What It Unlocks | Required By |
|---|---|---|
| `serde` | Serialization for all types | `wire`, `did`, `zk` |
| `wire` | `SignedChain`, `VerifiedToken`, `CertExtensions` | Gateway, SDKs |
| `async` | Async storage traits, `AsyncA1Context`, `VaultSigner` | Gateway |
| `did` | `AgentDid`, `DidDocument`, `VerifiableCredential` | DID/VC endpoints |
| `zk` | `ZkChainCommitment`, `ZkProofMode`, `anchor_hash` | ZK endpoints |
| `anchor` | On-chain anchoring helpers | ZK + blockchain |
| `negotiate` | `negotiate_algorithm()` for hybrid deployments | PQ migration |
| `swarm` | `SwarmRegistry`, swarm gateway routes | Swarm deployments |
| `governance` | Governance proposal and vote recording | DAO deployments |
| `tracing` | `tracing` spans during authorization | Observability |
| `ffi` | C ABI exports | FFI consumers |
| `policy-yaml` | YAML policy file parsing | CLI policy command |
| `post-quantum` | Full ML-DSA signature verification | Quantum deployments |
| `schema` | JSON Schema export | API documentation |
| `cbor` | CBOR serialization | Bandwidth-constrained |
| `otel` | OpenTelemetry spans | Distributed tracing |
| `full` | All above except `post-quantum` | Full deployments |

---

## Environment Variables (Gateway)

| Variable | Default | Description |
|---|---|---|
| `A1_SIGNING_KEY_HEX` | *(generated)* | 32-byte hex Ed25519 seed for gateway signing identity |
| `A1_MAC_KEY_HEX` | *(generated)* | 32-byte hex key for `VerifiedToken` HMAC |
| `A1_ADMIN_SECRET` | *(none)* | Bearer token for admin endpoints. **Required in production.** |
| `A1_REDIS_URL` | *(none)* | Redis URL (e.g. `redis://127.0.0.1/`) |
| `A1_PG_URL` | *(none)* | Postgres URL (e.g. `postgres://user:pass@host/db`) |
| `A1_RATE_LIMIT_RPS` | `500` | Per-IP requests per second limit |
| `A1_CORS_ALLOWED_ORIGIN` | *(none)* | CORS origin (`*` for permissive) |
| `GATEWAY_ADDR` | `0.0.0.0:8080` | Bind address |
| `A1_PUBLIC_BASE_URL` | `http://localhost:8080` | Used in `.well-known` discovery document |
| `A1_TRUSTED_PROXY_MODE` | *(none)* | `x-forwarded-for`, `fly-client-ip`, or `cf-connecting-ip` |
| `RUST_LOG` | `a1_gateway=info` | Log filter (e.g. `a1_gateway=debug,a1=trace`) |

---

## Compliance Pack

A1 ships with pre-built compliance documentation for the two most common enterprise security frameworks.

### SOC 2 Type II

`docs/compliance/soc2-mapping.md` maps every SOC 2 Trust Service Criteria control to A1 code controls:

- **CC6.1 (Logical Access Controls)** → `NarrowingMatrix` + `DyoloPassport` capability enforcement.
- **CC6.6 (Unauthorized Access Prevention)** → `NonceStore` replay prevention + `RevocationStore`.
- **CC7.2 (System Monitoring)** → `AuditSink` → Datadog / Splunk / OTLP.
- **CC9.2 (Vendor Risk Management)** → Self-hostable gateway, no cloud dependency at authorization time.

### ISO/IEC 27001:2022

`docs/compliance/iso27001-mapping.md` maps Annex A controls:

- **A.5.15 (Access Control)** → `DyoloChain` chain verification.
- **A.8.2 (Privileged Access Rights)** → `NarrowingMatrix` + `PolicySet` TTL limits.
- **A.8.15 (Logging)** → `AuditSink` with tamper-evident `ProvableReceipt`.
- **A.8.24 (Use of Cryptography)** → Ed25519, Blake3, optional ML-DSA.

### Sample Audit Report

`docs/compliance/sample-audit-report.md` is a pre-filled audit report template. Replace the bracketed placeholders with your deployment details and submit to your auditor.