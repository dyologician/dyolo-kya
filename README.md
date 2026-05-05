# dyolo-kya — Know Your Agent

**Cryptographic chain-of-custody for recursive AI agent delegation.**

[![Crates.io](https://img.shields.io/crates/v/dyolo-kya)](https://crates.io/crates/dyolo-kya)
[![npm](https://img.shields.io/npm/v/dyolo-kya)](https://www.npmjs.com/package/dyolo-kya)
[![PyPI](https://img.shields.io/pypi/v/dyolo-kya)](https://pypi.org/project/dyolo-kya/)
[![docs.rs](https://img.shields.io/docsrs/dyolo-kya)](https://docs.rs/dyolo-kya)
[![CI](https://github.com/dyologician/dyolo-kya/actions/workflows/ci.yml/badge.svg)](https://github.com/dyologician/dyolo-kya/actions)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](LICENSE)

---

## The Problem

When a human authorizes an AI agent — which may delegate to another agent, which
may delegate again — the authorization chain breaks down at the first hop.
There is no irrefutable proof that an action traces back to a human authorization,
no guarantee that the delegated scope is a strict subset of the parent's scope,
and no replay or revocation enforcement across hops.

This is the **Recursive Delegation Gap**. `dyolo-kya` closes it with a
cryptographic chain-of-custody protocol using Ed25519 signatures and Blake3
Merkle trees over intent hashes.

---

## What is new in v2.0.0

| Gap from v1 | v2.0 fix |
|---|---|
| Rust-only — no SDK for other languages | REST gateway + Go SDK + Python SDK + TypeScript SDK |
| No ops tooling | `dyolo-kya-cli`: keygen, issue, revoke, inspect, verify, decode |
| No identity bridge to existing IAM | `dyolo-kya-identity`: JWT/OIDC binding + YAML policy-as-code |
| Redis-only storage | `dyolo-kya-pg`: PostgreSQL adapter with multi-tenant namespacing |
| No typed extension fields | `CertExtensions` committed into cert signatures |
| No AI framework examples | LangChain, OpenAI Assistants, AutoGen, Go SDK |
| Global rate limiting | Per-client-IP bucketing with `governor::keyed` |
| Two-step nonce protocol (TOCTOU) | Atomic `try_consume` — single-roundtrip at DB level |
| No batch cert issuance | `POST /v1/cert/issue-batch` + `CertBundle` type |
| No discovery document | `GET /.well-known/kya-configuration` (OIDC-style) |
| No batch authorization | `POST /v1/authorize/batch` + `authorizeBatch` in all SDKs |
| No policy-as-code | `PolicyDocument` YAML/JSON compiled into `DelegationPolicy` |

---

## Architecture

```
┌───────────────────────────────────────────────────────────────────────┐
│  Human (Ed25519 root)                                                 │
│    │ signs DelegationCert (scope: trade.equity, max_depth: 8)         │
│    ▼                                                                  │
│  Orchestrator agent (Ed25519)                                         │
│    │ narrows scope → signs sub-cert (scope: trade.equity/NYSE)        │
│    ▼                                                                  │
│  Tool agent (Ed25519)                                                 │
│    │ calls execute_trade("AAPL", 10)                                  │
│    ▼                                                                  │
│  dyolo-kya-gateway  POST /v1/authorize                                │
│    verifies: signatures → scope ⊆ parent → expiry → nonce → revoked  │
│    returns: AuthorizeResponse { authorized: true, chain_depth: 2 }    │
└───────────────────────────────────────────────────────────────────────┘
```

---

## Five-minute start (Docker + Python)

```bash
# 1. Run the gateway
docker run -p 8080:8080 ghcr.io/dyologician/dyolo-kya-gateway:2

# 2. Install the Python SDK
pip install dyolo-kya

# 3. Issue a cert and authorize an action
python - <<'EOF'
from dyolo_kya import KyaClient, IntentSpec

kya  = KyaClient("http://localhost:8080")
cert = kya.issue_cert(
    delegate_pk_hex="<agent-pk-hex>",
    intents=[IntentSpec("trade.equity", {"exchange": "NYSE"})],
    ttl_seconds=3600,
    extensions={"dyolo.cost_center": "ai-ops"},
)
print("Cert fingerprint:", cert.fingerprint_hex)
EOF
```

---

## SDK quick-reference

### Go

```go
import "github.com/dyologician/dyolo-kya/sdk/go/kya"

client := kya.NewClient("http://localhost:8080")

cert, err := client.IssueCert(ctx, kya.IssueCertRequest{
    DelegatePkHex: agentPkHex,
    Intents:       []kya.IntentSpec{{Name: "trade.equity"}},
    TtlSeconds:    3600,
})

result, err := client.Authorize(ctx, kya.AuthorizeRequest{
    Chain:         signedChain,
    IntentName:    "trade.equity",
    ExecutorPkHex: agentPkHex,
})

// Batch: authorize multiple intents atomically
batch, err := client.AuthorizeBatch(ctx, kya.BatchAuthorizeRequest{
    Chain:         signedChain,
    ExecutorPkHex: agentPkHex,
    Intents:       []kya.BatchIntentSpec{{Name: "query.portfolio"}, {Name: "trade.equity"}},
})
```

### Python

```python
from dyolo_kya import KyaClient, IntentSpec

kya  = KyaClient("http://localhost:8080")
cert = kya.issue_cert(delegate_pk_hex="...", intents=[IntentSpec("trade.equity")])
res  = kya.authorize(chain=signed_chain, intent_name="trade.equity", executor_pk_hex="...")

# Batch
batch = kya.authorize_batch(
    chain=signed_chain,
    executor_pk_hex="...",
    intents=[IntentSpec("query.portfolio"), IntentSpec("trade.equity")],
)
assert batch.all_authorized
```

### TypeScript / Node.js

```ts
import { KyaClient } from "dyolo-kya";
import { buildLangChainKyaBatchTool } from "dyolo-kya/integrations";

const kya  = new KyaClient("http://localhost:8080");
const cert = await kya.issueCert({ delegatePkHex: "...", intents: [{ name: "trade.equity" }] });

// Single-intent
const res = await kya.authorize({ chain, intentName: "trade.equity", executorPkHex: "..." });

// Multi-intent batch (atomic single round-trip)
const batch = await kya.authorizeBatch({
  chain,
  executorPkHex: "...",
  intents: [{ name: "query.portfolio" }, { name: "trade.equity" }],
});
if (!batch.allAuthorized) throw new Error("Batch denied");

// LangChain multi-intent tool guard
const tool = buildLangChainKyaBatchTool({
  name: "portfolio_rebalance",
  description: "Query and rebalance a portfolio",
  intentNames: ["query.portfolio", "trade.equity"],
  client: kya,
  resolveContext: (input) => ({ chain: agentChain, executorPkHex: agentPk }),
  run: async (input, auth) => `Rebalanced (${auth.authorizedCount} intents authorized)`,
});
```

### Rust (native, no gateway)

```rust
use dyolo_kya::{
    DyoloIdentity, DyoloChain, Intent, CertBuilder, IntentTree,
    MerkleProof, MemoryRevocationStore, MemoryNonceStore, SystemClock,
    policy::{DelegationPolicy, CapabilitySet, PolicySet},
    NoopAuditSink,
};

let human  = DyoloIdentity::generate();
let agent  = DyoloIdentity::generate();
let intent = Intent::new("trade.equity").unwrap();
let tree   = IntentTree::build(vec![intent.hash()]).unwrap();
let cert   = CertBuilder::new(agent.verifying_key(), tree.root(), now, now + 3600)
                 .sign(&human);
let mut chain = DyoloChain::new(human.verifying_key(), tree.root());
chain.push(cert);

// Plain authorize
let action = chain.authorize(
    &agent.verifying_key(), &intent.hash(),
    &MerkleProof::default(), &SystemClock,
    &MemoryRevocationStore::new(), &MemoryNonceStore::new(),
).unwrap();

// Authorize with policy enforcement + audit log
let policy = PolicySet::new().add(
    DelegationPolicy::new("fintech-prod")
        .max_chain_depth(4)
        .max_ttl_secs(3600)
        .capabilities(CapabilitySet::new().allow("trade."))
        .forbid_sub_delegation(),
);
let action = chain.authorize_with_options(
    &agent.verifying_key(), &intent.hash(),
    &MerkleProof::default(), &SystemClock,
    &MemoryRevocationStore::new(), &MemoryNonceStore::new(),
    Some(&policy), &NoopAuditSink,
).unwrap();
assert_eq!(action.receipt.chain_depth, 1);
```

---

## Policy-as-code (YAML)

Store your policies in source control and load them at startup:

```yaml
# policies/fintech-trading.yaml
name: fintech-trading
max_chain_depth: 4
max_ttl_secs: 3600
forbid_sub_delegation: true
capabilities:
  - "trade.equity"
  - "query.portfolio"
required_extensions:
  - "dyolo.cost_center"
```

```rust
use dyolo_kya_identity::policy::PolicyDocument;

let doc    = PolicyDocument::from_yaml(include_str!("policies/fintech-trading.yaml"))?;
let policy = doc.into_policy(); // → DelegationPolicy
```

---

## Batch cert issuance

Issue multiple delegation certificates in a single authenticated request:

```bash
POST /v1/cert/issue-batch
{
  "requests": [
    { "delegate_pk_hex": "<pk1>", "intents": [{"name":"trade.equity"}], "ttl_seconds": 3600 },
    { "delegate_pk_hex": "<pk2>", "intents": [{"name":"query.portfolio"}], "ttl_seconds": 7200 }
  ]
}
```

Returns a `CertBundle` alongside the individual issuance results.

---

## Discovery document

```bash
GET /.well-known/kya-configuration
```

Returns an OIDC-style JSON document with the gateway's signing public key,
all endpoint URLs, supported algorithms, and protocol version. Enterprise
clients can bootstrap trust by fetching this document and pinning the
`gateway_signing_pk_hex` field.

---

## CLI

```bash
cargo install dyolo-kya-cli

dyolo-kya keygen
dyolo-kya issue --policy examples/trading_chain.yaml
dyolo-kya revoke <fingerprint-hex>
dyolo-kya inspect <fingerprint-hex>
dyolo-kya verify token.json
dyolo-kya decode cert.json

# PostgreSQL migration (one-time setup for dyolo-kya-pg)
dyolo-kya migrate --database-url postgres://user:pass@localhost/mydb
# Print the raw DDL for manual or CI-managed application
dyolo-kya migrate --print | psql "$DATABASE_URL"

# Generate shell completions
dyolo-kya completion bash   >> ~/.bash_completion
dyolo-kya completion zsh    >> ~/.zshrc
dyolo-kya completion fish   > ~/.config/fish/completions/dyolo-kya.fish
```

---

## Gateway REST API

| Method | Path | Description |
|---|---|---|
| `GET`  | `/health`                     | Liveness + signing public key |
| `GET`  | `/.well-known/kya-configuration` | OIDC-style discovery document |
| `POST` | `/v1/cert/issue`              | Issue a single cert |
| `POST` | `/v1/cert/issue-batch`        | Issue multiple certs (CertBundle) |
| `POST` | `/v1/cert/revoke`             | Revoke a cert by fingerprint |
| `POST` | `/v1/cert/revoke-batch`       | Revoke multiple certs in one call |
| `GET`  | `/v1/cert/:fp`                | Inspect revocation status |
| `POST` | `/v1/authorize`               | Authorize a single agent intent |
| `POST` | `/v1/authorize/batch`         | Authorize multiple intents atomically |
| `POST` | `/v1/token/verify`            | Verify a VerifiedToken MAC |

### Environment variables

| Variable | Default | Description |
|---|---|---|
| `DYOLO_SIGNING_KEY_HEX` | ephemeral | 32-byte Ed25519 signing key (hex). Set in production. |
| `DYOLO_MAC_KEY_HEX`     | ephemeral | 32-byte BLAKE3 MAC key (hex). Set in production. |
| `DYOLO_RATE_LIMIT_RPS`  | `500` | Per-IP request rate limit (requests/second). |
| `GATEWAY_ADDR`          | `0.0.0.0:8080` | Bind address |
| `RUST_LOG`              | `info` | Log filter |

---

## Framework integrations

| Framework | Language | Reference |
|---|---|---|
| LangChain | Python | [docs/integrations/langchain.md](docs/integrations/langchain.md) |
| OpenAI Assistants | Python | [docs/integrations/openai-agents.md](docs/integrations/openai-agents.md) |
| AutoGen | Python | [examples/integrations/autogen_example.py](examples/integrations/autogen_example.py) |
| OpenAI Agents SDK | TypeScript | [examples/integrations/openai_agents_example.ts](examples/integrations/openai_agents_example.ts) |
| LangChain.js (batch) | TypeScript | `buildLangChainKyaBatchTool` in `dyolo-kya/integrations` |
| OpenAI SDK (batch) | TypeScript | `buildOpenAIKyaBatchFunction` in `dyolo-kya/integrations` |
| AutoGen (batch) | TypeScript | `withKyaBatchGuard` in `dyolo-kya/integrations` |

---

## Crate / package layout

| Package | Description |
|---|---|
| `dyolo-kya` (Rust) | Core library — certs, chains, intents, identity, policy, audit |
| `dyolo-kya-gateway` | Axum REST sidecar with per-IP rate limiting |
| `dyolo-kya-cli` | Operations CLI |
| `dyolo-kya-redis` | Redis `AsyncRevocationStore` + `AsyncNonceStore` (atomic `SET NX PX`) |
| `dyolo-kya-pg` | PostgreSQL adapter (atomic `INSERT ON CONFLICT DO NOTHING`) |
| `dyolo-kya-identity` | JWT/OIDC binding + YAML policy-as-code (`PolicyDocument`) |
| `dyolo-kya` (npm) | TypeScript/Node.js SDK |
| `dyolo-kya` (PyPI) | Python SDK |
| `dyolo-kya` (Go module) | Go SDK |

---

## How dyolo-kya compares to JWT delegation and SPIFFE

See [docs/vs-jwt-spiffe.md](docs/vs-jwt-spiffe.md).  
SPIFFE answers "which workload is this?", JWT answers "which user triggered this?",
and dyolo-kya answers "did the user authorize this exact action through this exact
delegation chain, and is the scope still valid?"

---

## Security

Primitives: `ed25519-dalek 2` (signatures), `blake3 1` (hashing + MAC),
`subtle 2` (constant-time comparisons). Nonce stores use single-operation
atomic commits — `SET NX PX` for Redis, `INSERT ON CONFLICT DO NOTHING` for
Postgres — eliminating the check-then-set TOCTOU race.

Report vulnerabilities to the address in [SECURITY.md](SECURITY.md).

---

## License

MIT OR Apache-2.0.
MD_EOF
echo "done"