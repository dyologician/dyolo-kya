# A1 — Know Your Agent

**Cryptographic chain-of-custody for recursive AI agent delegation.**

[![Crates.io](https://img.shields.io/crates/v/a1-ai.svg)](https://crates.io/crates/a1-ai)
[![npm](https://img.shields.io/npm/v/a1-ai.svg)](https://www.npmjs.com/package/a1-ai)
[![PyPI](https://img.shields.io/pypi/v/a1identity.svg)](https://pypi.org/project/a1identity/)
[![Go Reference](https://img.shields.io/badge/go-reference-blue)](https://pkg.go.dev/github.com/dyologician/a1/sdk/go/a1/kya)
[![CI](https://github.com/dyologician/a1/actions/workflows/ci.yml/badge.svg)](https://github.com/dyologician/a1/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

---

## The problem this solves

When an AI agent delegates a task to another agent — and that agent delegates further — the original authorization completely breaks down. There is no way to prove which human approved the action at the end of the chain. This is called the **Recursive Delegation Gap**, and it is the reason enterprises cannot safely deploy multi-agent AI systems at scale.

**A1 closes this gap.**

Every action executed by any agent in any delegation tree carries an irrefutable, cryptographically verified chain proving exactly which human authorized it and what boundaries were enforced. This holds offline, without a trust server, without any shared secrets, and without changing your existing agent code.

---

## What you get

| Capability | Description |
|---|---|
| **A1 Passport** | Issue a long-lived agent identity once. Every sub-agent it creates is cryptographically bound to the same chain of custody. |
| **NarrowingMatrix** | A 256-bit capability bitmask that enforces strict subset delegation in O(1). No agent can request a capability its parent did not explicitly grant. No escalation is possible. |
| **ProvableReceipt** | Every authorized action produces a tamper-evident receipt your compliance team can verify independently, without any secrets. |
| **One-line middleware** | `@a1_guard`, `withA1Passport`, `a1.WithPassport`. Drop into any AI framework in one line. |
| **MCP server built-in** | Any MCP-compatible agent (Claude Code, Cursor, etc.) can use A1 authorization with zero code changes — just point at the gateway. |
| **Works everywhere** | LangChain, LangGraph, LlamaIndex, AutoGen, CrewAI, Semantic Kernel, OpenAI Agents, plain Python, TypeScript, Go, and any C FFI target. |
| **Air-gap compatible** | All verification is local. No network call, no trust server, no cloud dependency at authorization time. |
| **Enterprise-ready** | AWS KMS, GCP KMS, HashiCorp Vault, Azure Key Vault signing backends. SOC 2 and ISO 27001 compliance mapping included. |
| **Post-quantum ready** | Wire format supports ML-DSA (CRYSTALS-Dilithium) hybrid signatures alongside Ed25519 — no migration required when you upgrade. |

---

## Install

**Rust**

```toml
# Cargo.toml
[dependencies]
a1-ai = { version = "2.8", features = ["full"] }
```

**Python**

```bash
pip install a1identity
```

**TypeScript / Node.js**

```bash
npm install a1-ai
```

**Go**

```bash
go get github.com/dyologician/a1/sdk/go/a1/kya
```

**CLI**

```bash
# From crates.io (after publish):
cargo install a1-cli

# From source (run from the repo root):
cargo install --path a1-cli
```

> **Common mistake:** `cargo install --path . --bin a1-cli` will fail — the root package is the Rust library, not the CLI. Always use `--path a1-cli` (points to the CLI sub-crate).

---

## Quickstart (5 minutes)

### 1. Start the gateway

```bash
git clone https://github.com/dyologician/a1.git
cd a1
docker compose up -d
```

This starts three services: **gateway** (port 8080), **Redis** (nonce + revocation cache), and **Postgres** (persistent storage). The gateway is ready when the health check passes:

> **First run:** Docker must compile the Rust gateway from source. This takes **3–10 minutes** — subsequent starts are instant because the image is cached.

```bash
curl http://localhost:8080/healthz
```

### 2. Generate a passport

```bash
a1 passport issue \
  --namespace my-trading-agent \
  --allow "trade.equity,portfolio.read" \
  --ttl 30d \
  --out passport.json
```

This creates `passport.json` — your agent's root identity. Store it in your vault or secrets manager.

### 3. Add one line to your agent tool

**Python (any framework)**

```python
from a1.passport import a1_guard, PassportClient

client = PassportClient("http://localhost:8080")

@a1_guard(client=client, capability="trade.equity")
async def execute_trade(symbol: str, qty: int, signed_chain: dict, executor_pk_hex: str) -> dict:
    return await broker.place_order(symbol, qty)
```

**TypeScript**

```typescript
import { withA1Passport, PassportClient } from "a1/passport";

const client = new PassportClient("http://localhost:8080");

const guardedTrade = withA1Passport(executeTrade, {
  client,
  capability: "trade.equity",
});
```

**Go**

```go
import a1 "github.com/dyologician/a1/sdk/go/a1/kya"

guarded := a1.WithPassport(executeTrade, passport)
```

**Rust (direct / embedded)**

```rust
use a1::{DyoloPassport, DyoloIdentity, Intent, SystemClock};

let passport = DyoloPassport::load("passport.json")?;
let sub_cert = passport.issue_sub(agent_pk, &["trade.equity"], 3600, &root_id, &SystemClock)?;

let mut chain = passport.new_chain()?;
chain.push(sub_cert);

let intent  = Intent::new("trade.equity")?;
let receipt = passport.guard_local(&chain, &agent_pk, &intent)?;

println!("{}", receipt);
// ProvableReceipt { namespace=my-trading-agent, depth=1, fingerprint=... }
```

### 4. Verify an action happened (audit)

```bash
a1 passport inspect passport.json
# Namespace:    my-trading-agent
# Capabilities: trade.equity, portfolio.read
# Expires:      2026-06-05T00:00:00Z
# Mask:         0000000000000003...
# Status:       VALID
```

---

## MCP — Claude Code and Cursor support

A1 ships a built-in [Model Context Protocol](https://spec.modelcontextprotocol.io/) server. Any MCP-compatible agent or IDE extension can authorize through A1 **without any code changes** to the agent itself.

**Add to your MCP config (`.mcp.json` or Claude Code settings):**

```json
{
  "mcpServers": {
    "a1-ai": {
      "url": "http://localhost:8080/mcp"
    }
  }
}
```

**Endpoints:**

| Method | Path | Description |
|---|---|---|
| `GET` | `/mcp` | SSE stream — persistent MCP session (server-sent events) |
| `POST` | `/mcp` | JSON-RPC 2.0 — single MCP request/response |
| `GET` | `/mcp/tools` | Tool manifest — list all available A1 tools (non-MCP convenience) |

The MCP server exposes A1's authorize, cert-issue, revoke, and passport-check tools directly to the agent. No decorators. No imports. One config entry.

---

## A1 Studio

A1 ships a built-in web dashboard at `http://localhost:8080/studio`. Open it in your browser after starting the gateway. No separate install required.

**What you can do without writing any code:**

- **Issue and manage passports** — create, inspect, renew, and revoke agent passports from the browser. The **Passport Dashboard** shows every passport across all namespaces in one sorted, searchable view, with urgent/expiring passports surfaced at the top.
- **Connect and test agents** — register running agents, fire live capability checks, and view real-time delegation chain inspection.
- **Test MCP tools** — the built-in MCP tester lets you call `authorize`, `cert-issue`, `revoke`, and `passport-check` directly against the gateway without touching a terminal.
- **Visualize the trust layer** — an interactive delegation chain diagram shows the full path from human principal through every agent hop to the final receipt.
- **Connect local AI models** — the **Local AI** tab auto-detects Ollama, LM Studio, and llama.cpp running on your machine. Select a model, pick a language (Python, TypeScript, LangChain, or `.mcp.json`), and get ready-to-paste integration code in one click. Fully offline, no cloud key needed.
- **Explain errors** — paste any A1 error code or JSON error body into the Error Explainer and get a plain-English fix suggestion.
- **Enterprise config** — multi-tenant setup, webhook configuration, SIEM endpoint registration.

Studio is designed for non-technical team members, compliance officers, and anyone who needs visibility into what agents are authorized to do — alongside a full developer toolset for engineers.

---

## Framework integrations

### LangChain

```python
from a1.langchain_tool import A1AuthorizationTool

tool = A1AuthorizationTool(
    name="execute_trade",
    intent_name="trade.equity",
    client=client,
    func=execute_trade_fn,
    chain=agent_chain,
    executor_pk_hex=agent_pk,
)
```

### LangGraph

```python
from a1.langgraph_tool import a1_node, A1StateSchema

class AgentState(A1StateSchema):
    messages: list
    symbol: str

@a1_node(intent_name="trade.equity", client=client, propagate_receipt=True)
async def execute_trade(state: AgentState) -> AgentState:
    await broker.place_order(state["symbol"])
    return state
```

### LlamaIndex

```python
from a1.llamaindex_tool import a1_llamaindex_tool

tool = a1_llamaindex_tool(
    fn=read_portfolio_fn,
    intent_name="portfolio.read",
    client=client,
    resolve_context=lambda kwargs: {"chain": agent_chain, "executor_pk_hex": agent_pk},
    name="read_portfolio",
    description="Read portfolio holdings.",
)
```

### AutoGen v0.4

```python
from a1.autogen_tool import build_a1_function_tool

tool = build_a1_function_tool(
    fn=execute_trade,
    intent_name="trade.equity",
    client=client,
    chain=agent_chain,
    executor_pk_hex=agent_pk,
)
```

### CrewAI

```python
from a1.crewai_tool import A1AuthorizationTool

tool = A1AuthorizationTool(
    func=execute_trade,
    intent_name="trade.equity",
    gateway_url="http://localhost:8080",
    chain=agent_chain,
    executor_pk_hex=agent_pk,
)
```

### Semantic Kernel

```python
from a1.semantic_kernel_tool import a1_sk_function
from semantic_kernel import Kernel

class TradingPlugin:
    @a1_sk_function(intent_name="trade.equity", client=client, description="Execute equity trade.")
    async def execute_trade(self, symbol: str, signed_chain: dict, executor_pk_hex: str) -> str:
        return f"Traded {symbol}"

kernel = Kernel()
kernel.add_plugin(TradingPlugin(), plugin_name="trading")
```

### OpenAI Agents SDK

```python
from a1.openai_tool import a1_openai_function

@a1_openai_function(intent_name="trade.equity", client=client)
async def execute_trade(symbol: str, qty: int) -> str:
    return await broker.place_order(symbol, qty)
```

### Python ASGI / FastAPI middleware

Protect an entire API application with one call:

```python
from a1.middleware import A1Middleware
from fastapi import FastAPI

app = FastAPI()
app.add_middleware(
    A1Middleware,
    client=PassportClient("http://localhost:8080"),
    capability="api.write",
)
```

All requests must carry a valid `signed_chain` and `executor_pk_hex`. Any request that fails authorization receives a `403` with a machine-readable `error_code`.

### Swarm management (Python)

Coordinate fleets of agents with role-based access:

```python
from a1.swarm import SwarmClient, SwarmRole

client = SwarmClient("http://localhost:8080", admin_secret=os.environ["A1_ADMIN_SECRET"])

# Create a swarm passport
swarm_id = client.create_swarm(
    name="acme-trading-swarm",
    capabilities=["trade.equity", "portfolio.read"],
    ttl_days=30,
    signing_key_hex=ROOT_SK_HEX,
)

# Add a worker agent with a scoped capability subset
cert = client.add_member(
    swarm_id=swarm_id,
    agent_pk_hex=WORKER_PK_HEX,
    role=SwarmRole.WORKER,
    capabilities=["trade.equity"],
    ttl_seconds=3600,
    signing_key_hex=ROOT_SK_HEX,
)

# List all active members
members = client.list_members(swarm_id=swarm_id)
```

**Roles:** `SwarmRole.ORCHESTRATOR`, `SwarmRole.SUPERVISOR`, `SwarmRole.AUDITOR`, `SwarmRole.WORKER`.

### Swarm management (TypeScript)

```typescript
import { SwarmClient, SwarmRole } from "a1/swarm";

const swarm = new SwarmClient("http://localhost:8080", {
  adminSecret: process.env.A1_ADMIN_SECRET,
});

const swarmId = await swarm.createSwarm({
  name: "acme-trading-swarm",
  capabilities: ["trade.equity", "portfolio.read"],
  ttlDays: 30,
  signingKeyHex: ROOT_SK_HEX,
});

const member = await swarm.addMember({
  swarmId,
  agentPkHex: WORKER_PK_HEX,
  role: SwarmRole.Worker,
  capabilities: ["trade.equity"],
  ttlSeconds: 3600,
  signingKeyHex: ROOT_SK_HEX,
});

const members = await swarm.listMembers(swarmId);
```

### TypeScript middleware utilities

```typescript
import { A1Middleware, exchangeJwt, verifyWebhookSignature } from "a1-ai";

// Express middleware — protect all routes
app.use(A1Middleware({ client, capability: "api.write" }));

// Bootstrap a delegation cert from an existing OIDC/SSO JWT
const cert = await exchangeJwt({
  token: req.headers.authorization.replace("Bearer ", ""),
  capabilities: ["files.read"],
  ttlSeconds: 3600,
  delegatePkHex: agentPk,
  gatewayUrl: "http://localhost:8080",
});

// Verify an incoming webhook payload from the A1 gateway
app.post("/a1-webhook", (req, res) => {
  const valid = verifyWebhookSignature(
    req.body,
    req.headers["x-a1-signature"] as string,
    process.env.A1_WEBHOOK_SECRET!,
  );
  if (!valid) return res.status(401).end();
  // process audit event
});
```

---

## Gateway — REST API reference

All endpoints are served on `http://localhost:8080` by default.

### Authorization

| Method | Path | Auth required | Description |
|---|---|---|---|
| `POST` | `/v1/authorize` | No | Verify a signed delegation chain against an intent |
| `POST` | `/v1/authorize/batch` | No | Verify multiple intents in a single chain traversal |
| `POST` | `/v1/passport/authorize` | No | Same as `/v1/authorize` but resolves passport extensions |
| `POST` | `/v1/token/verify` | No | Verify a `VerifiedToken` (HMAC-signed receipt) |

**`POST /v1/authorize` — request body:**

```json
{
  "chain": { /* SignedChain */ },
  "intent_name": "trade.equity",
  "intent_params": { "symbol": "AAPL", "qty": "100" },
  "executor_pk_hex": "<32-byte hex>",
  "return_token": false,
  "request_id": "optional-trace-id"
}
```

**Response:**

```json
{
  "authorized": true,
  "chain_depth": 2,
  "chain_fingerprint": "<hex>",
  "verified_at_unix": 1748000000,
  "receipt": {
    "chain_depth": 2,
    "fingerprint_hex": "<hex>",
    "verified_at_unix": 1748000000,
    "passport_namespace": "my-trading-agent",
    "capability_mask_hex": "<64-char hex>",
    "narrowing_commitment_hex": "<hex>"
  },
  "token": null
}
```

### Certificates (admin-protected)

| Method | Path | Auth required | Description |
|---|---|---|---|
| `POST` | `/v1/cert/issue` | Yes | Issue a single `DelegationCert` |
| `POST` | `/v1/cert/issue-batch` | Yes | Issue multiple certs in one call |
| `POST` | `/v1/cert/revoke` | No | Revoke a cert by fingerprint |
| `POST` | `/v1/cert/revoke-batch` | No | Revoke multiple certs at once |
| `GET` | `/v1/cert/:fingerprint` | No | Inspect a cert by its Blake3 fingerprint |

### Passports

| Method | Path | Auth required | Description |
|---|---|---|---|
| `POST` | `/v1/passports/issue` | No | Issue a new `DyoloPassport` and save to disk |
| `GET` | `/v1/passports/list` | No | List passport files under `~/.a1/passports/` |
| `POST` | `/v1/passports/renew` | No | Re-issue a passport with a new TTL |
| `GET` | `/v1/passports/read` | No | Read a passport file by namespace |
| `POST` | `/v1/passports/restore` | No | Restore passport files from a backup payload |
| `POST` | `/v1/passports/revoke-by-namespace` | No | Revoke all certs issued under a namespace |

**`POST /v1/passports/issue` — request body:**

```json
{
  "namespace": "my-agent",
  "capabilities": ["files.read", "web.search"],
  "ttl": "30d",
  "output_path": null
}
```

### DID / W3C Verifiable Credentials

| Method | Path | Auth required | Description |
|---|---|---|---|
| `GET` | `/v1/did/gateway` | No | Resolve the gateway's own DID document |
| `GET` | `/v1/did/:pk_hex` | No | Resolve a DID document by public key hex |
| `POST` | `/v1/vc/issue` | Yes | Issue a W3C Verifiable Credential tied to an agent identity |
| `POST` | `/v1/vc/verify` | No | Verify a Verifiable Credential |

### On-chain anchoring (ZK)

| Method | Path | Auth required | Description |
|---|---|---|---|
| `POST` | `/v1/anchor` | No | Anchor a `ZkChainCommitment` on-chain |

Supported networks: `ethereum`, `polygon`, `base`, `arbitrum`, `solana`, `ethereum-sepolia`, or a custom EVM chain via `{"custom": {"chain_id": N, "name": "..."}}`.

**Request body:**

```json
{
  "commitment": { /* ZkChainCommitment */ },
  "passport_did": "did:a1:...",
  "network": "ethereum"
}
```

**Response** includes `anchored_receipt`, `anchor_hash_hex`, and for EVM chains `evm_calldata` (submit via `eth_sendRawTransaction`), and for Solana `solana_instruction_data`.

### Agent-to-agent delegation negotiation

| Method | Path | Auth required | Description |
|---|---|---|---|
| `POST` | `/v1/negotiate` | No | Exchange a `CapabilityRequest` for a scoped `DelegationCert` |

An agent sends a signed `CapabilityRequest`; the gateway issues a cert scoped to the requested capability and returns a `DelegationOffer`. Configure allowed capabilities with `A1_NEGOTIATE_CAPABILITIES` or set `A1_NEGOTIATE_ALLOW_ALL=1` for dev environments.

### JWT bridge (SSO / OIDC)

| Method | Path | Auth required | Description |
|---|---|---|---|
| `POST` | `/v1/jwt/exchange` | Yes | Exchange a JWKS-verified JWT for a scoped `DelegationCert` |

Enterprises running OIDC or SAML SSO use this to bootstrap delegation chains from their existing IAM infrastructure — no manual key ceremony required.

- Gateway fetches the issuer's JWKS and verifies the JWT signature.
- Cert TTL is capped at `min(requested_ttl, jwt_exp - now)`.
- JWT `sub` claim is recorded in the cert's `dyolo.jwt.subject` extension for audit.
- Configure with `A1_JWT_JWKS_URL` and `A1_JWT_ALLOWED_CAPS`.

**Request body:**

```json
{
  "token": "<JWT bearer token>",
  "capabilities": ["files.read"],
  "ttl_seconds": 3600,
  "delegate_pk_hex": "<agent public key hex>"
}
```

### Swarm management (admin-protected)

| Method | Path | Auth required | Description |
|---|---|---|---|
| `POST` | `/v1/swarm/create` | Yes | Create a new agent swarm |
| `POST` | `/v1/swarm/member/add` | Yes | Add an agent to a swarm with a role and TTL |
| `POST` | `/v1/swarm/member/remove` | Yes | Remove an agent from a swarm |
| `GET` | `/v1/swarm/:swarm_id/members` | No | List active members of a swarm |

**Swarm roles:** `orchestrator`, `supervisor`, `auditor`, `worker`.

### Governance

| Method | Path | Auth required | Description |
|---|---|---|---|
| `GET` | `/v1/governance/policy` | No | Return the active delegation policy |
| `POST` | `/v1/governance/approval/verify` | No | Verify a governance approval record |
| `POST` | `/v1/governance/audit-report` | Yes | Generate a structured audit report |

### Agent management

| Method | Path | Auth required | Description |
|---|---|---|---|
| `GET` | `/v1/agents/scan` | No | Scan for locally running agents |
| `POST` | `/v1/agents/connect` | No | Register a remote agent connection |
| `POST` | `/v1/agents/restart` | No | Restart a registered agent |
| `POST` | `/v1/agents/probe` | No | Probe an agent's health and capabilities |
| `POST` | `/v1/agents/relay` | No | Relay a message to an agent |
| `GET` | `/v1/agents/integration-check` | No | Verify SDK integration on a running agent |
| `POST` | `/v1/agents/read-file` | No | Read a file from an agent's working directory |
| `POST` | `/v1/agents/write-file` | No | Write a file to an agent's working directory |
| `GET` | `/v1/agents/list-files` | No | List files in an agent's working directory |

### Tenant management

| Method | Path | Auth required | Description |
|---|---|---|---|
| `GET` | `/v1/tenant/info` | No | Return the active tenant context for the caller |
| `GET` | `/v1/tenant/config` | No | Return per-tenant capability allowlist |

Send `X-A1-Tenant-ID: <tenant>` on any request to scope all revocation and nonce operations to that tenant. Enable with `A1_MULTI_TENANT=true`.

### System / utility

| Method | Path | Auth required | Description |
|---|---|---|---|
| `GET` | `/health` or `/healthz` | No | Liveness check — returns `{"status":"ok"}` |
| `GET` | `/studio` | No | A1 Studio web dashboard |
| `GET` | `/.well-known/a1-configuration` | No | Discovery document — all endpoint URLs, gateway DID, signing key |
| `POST` | `/v1/webhook/test` | Yes | Send a test webhook event |
| `GET` | `/v1/webhook/status` | No | Check webhook delivery status |
| `GET` | `/v1/ai/status` | No | Check if the AI proxy is configured |
| `POST` | `/v1/ai/chat` | No | Proxy a chat message through the gateway's AI key |
| `POST` | `/v1/system/autostart` | No | Install gateway as a system service (launchd/systemd) |
| `DELETE` | `/v1/system/autostart` | No | Remove the autostart service |
| `GET` | `/v1/system/status` | No | System and gateway status |
| `POST` | `/v1/system/install-docker` | No | Install Docker on the host machine |
| `POST` | `/v1/system/gitignore-add` | No | Add A1 entries to `.gitignore` |
| `POST` | `/v1/debug/explain-error` | No | Translate a raw A1 error code into plain English |

---

## Enterprise features

### KMS / Vault signing backends

A1 never locks you into a key storage format. Your root passport key lives in your KMS:

```python
from a1.vault import AwsKmsSigner, HashiCorpVaultSigner, GcpKmsSigner, AzureKeyVaultSigner

# AWS KMS
signer = AwsKmsSigner(key_id="alias/a1-passport-root", region="us-east-1")

# HashiCorp Vault (Transit engine, ed25519 key)
signer = HashiCorpVaultSigner(
    vault_addr="https://vault.corp.example.com",
    key_name="a1-passport-root",
)

# GCP KMS
signer = GcpKmsSigner(
    project="my-project",
    location="global",
    key_ring="a1-keys",
    key="passport-root",
)

# Azure Key Vault
signer = AzureKeyVaultSigner(
    vault_url="https://my-vault.vault.azure.net",
    key_name="a1-passport-root",
)
```

At verification time: **zero KMS calls**. The verifying key is embedded in the cert. Authorization is fully local.

### SIEM / audit log export

Every authorization event feeds your existing SIEM with zero configuration:

```python
from a1.siem import DatadogLogExporter, SplunkHecExporter, CompositeExporter

exporter = CompositeExporter([
    DatadogLogExporter(api_key=os.environ["DD_API_KEY"], service="trading-agents"),
    SplunkHecExporter(url="https://splunk.corp.com:8088", token=os.environ["SPLUNK_HEC_TOKEN"]),
])

exporter.export_dict(audit_event)
```

OpenTelemetry (OTLP) is also supported:

```python
from a1.siem import OpenTelemetryExporter
exporter = OpenTelemetryExporter(endpoint="http://otel-collector:4318", service_name="agents")
```

### OpenTelemetry distributed tracing (Python)

Every authorization event emits a structured OTEL span, compatible with Datadog APM, Jaeger, Honeycomb, and any OTLP backend:

```python
from a1.otel import A1Tracer
from a1 import PassportClient

tracer = A1Tracer(
    client=PassportClient("http://localhost:8080"),
    service_name="acme-trading-bot",
)

# Option 1 — wrap an existing client
guarded_client = tracer.instrument_passport_client(client)

# Option 2 — decorator
@tracer.trace_capability("trade.equity")
async def execute_trade(symbol: str, qty: int) -> dict:
    return await broker.place_order(symbol, qty)
```

Install the optional dependency: `pip install "a1identity[siem-otel]"`. All spans use the `dyolo.a1.*` attribute namespace. If `opentelemetry-sdk` is absent, the module silently degrades to a no-op so the rest of your code compiles unchanged.

### Namespace isolation (multi-tenant)

```rust
let ctx = A1Context::builder()
    .namespace("tenant-acme")
    .build();
let action = ctx.authorize(&chain, &agent_pk, &intent, &proof)?;
```

A cert issued for `tenant-acme` cannot authorize under `tenant-beta`. Hard separation, zero config overhead.

For the REST gateway, send `X-A1-Tenant-ID: acme` on every request and set `A1_MULTI_TENANT=true`.

### PostgreSQL and Redis storage backends

```toml
# Cargo.toml
[dependencies]
a1-pg    = "2.8"   # Postgres nonce + revocation stores
a1-redis = "2.8"   # Redis nonce + revocation stores
```

### Self-hosted gateway

```bash
docker compose up -d
# Gateway: http://localhost:8080
# Health:  http://localhost:8080/healthz
# Studio:  http://localhost:8080/studio
# Schema:  http://localhost:8080/.well-known/a1-configuration
```

---

## Production setup

> **Important:** The gateway starts safely in development mode with no configuration. Before going to production, set the environment variables below. Without them, keys are ephemeral (lost on restart), tokens are invalidated on every restart, admin endpoints are unprotected, and revocation state is not persisted.

Create a `.env` file (see `.env.example` in the repo) or export these in your deployment environment:

```bash
# Generate these once and store them in your secrets manager
A1_SIGNING_KEY_HEX=$(openssl rand -hex 32)   # Ed25519 seed for gateway signing key
A1_MAC_KEY_HEX=$(openssl rand -hex 32)        # HMAC key for VerifiedToken

# Protect admin endpoints (cert issuance, batch revocation, swarm management, etc.)
A1_ADMIN_SECRET=your-strong-secret-here

# Pick one persistent backend for revocation + nonce state
A1_REDIS_URL=redis://localhost:6379
# OR
A1_PG_URL=postgres://user:password@localhost/a1
```

Without a persistent backend, revoking a certificate has no effect after a restart — the revoked cert becomes valid again. This is a security issue in production.

---

## Gateway environment variables

| Variable | Default | Description |
|---|---|---|
| `A1_SIGNING_KEY_HEX` | *(generated)* | 32-byte hex Ed25519 seed for gateway signing identity. **Set in production.** |
| `A1_MAC_KEY_HEX` | *(generated)* | 32-byte hex key for `VerifiedToken` HMAC. **Set in production.** |
| `A1_ADMIN_SECRET` | *(none)* | Bearer token for admin endpoints. **Required in production.** |
| `A1_REDIS_URL` | *(none)* | Redis URL, e.g. `redis://127.0.0.1/` |
| `A1_PG_URL` | *(none)* | Postgres URL, e.g. `postgres://user:pass@host/db` |
| `A1_RATE_LIMIT_RPS` | `500` | Per-IP requests per second limit |
| `A1_CORS_ALLOWED_ORIGIN` | *(none)* | CORS origin (`*` for permissive) |
| `GATEWAY_ADDR` | `0.0.0.0:8080` | Bind address |
| `A1_PUBLIC_BASE_URL` | `http://localhost:8080` | Used in `.well-known` discovery document |
| `A1_TRUSTED_PROXY_MODE` | *(none)* | `x-forwarded-for`, `fly-client-ip`, or `cf-connecting-ip` |
| `A1_NEGOTIATE_CAPABILITIES` | *(none)* | Comma-separated list of capabilities the negotiate endpoint may issue |
| `A1_NEGOTIATE_ALLOW_ALL` | *(none)* | Set to `1` to allow any capability (dev/staging only) |
| `A1_JWT_JWKS_URL` | *(none)* | JWKS endpoint URL for JWT bridge (`/v1/jwt/exchange`) |
| `A1_JWT_ALLOWED_CAPS` | *(none)* | Comma-separated capability allowlist for JWT-exchanged certs |
| `A1_MULTI_TENANT` | *(none)* | Set to `true` to enable `X-A1-Tenant-ID` header enforcement |
| `A1_TENANT_REQUIRED` | *(none)* | Set to `true` to reject requests missing the tenant header |
| `A1_TENANT_ALLOWLIST` | *(none)* | Comma-separated list of permitted tenant IDs |
| `A1_AI_KEY` | *(none)* | Anthropic API key — enables the `/v1/ai/chat` proxy for Studio |
| `A1_WEBHOOK_URL` | *(none)* | URL to receive A1 audit webhook events |
| `A1_WEBHOOK_SECRET` | *(none)* | Secret for signing webhook payloads (verified via `verifyWebhookSignature`) |
| `RUST_LOG` | `a1_gateway=info` | Log filter |

---

## How it works

### The delegation model

```
Human principal
  └─ issues DyoloPassport (capabilities: [trade.equity, portfolio.read])
       └─ issues DelegationCert → Orchestrator agent (same caps or subset)
            └─ issues DelegationCert → Executor agent (trade.equity only)
                 └─ authorizes Intent("trade.equity")
                      → ProvableReceipt (fingerprint, mask commitment, depth=2)
```

Every arrow in this chain is an Ed25519 signature. Every scope reduction is enforced by NarrowingMatrix. The final receipt is independently verifiable without any secrets.

### The NarrowingMatrix (O(1) capability enforcement)

256-bit bitmask. Each capability name maps deterministically to a bit position via Blake3. Narrowing check:

```
child_mask & parent_mask == child_mask
```

This is eight 64-bit AND operations. It runs in nanoseconds regardless of how many named capabilities exist. No registry lookup. No network call. No configuration.

### Benchmark results

Run `cargo bench` to reproduce locally. Median wall-clock times across 10,000 iterations (Criterion.rs, Apple M-series / AWS c7g.large). Full methodology and comparison tables: [`docs/performance-benchmarks.md`](docs/performance-benchmarks.md).

| Operation | Latency |
|---|---|
| `NarrowingMatrix::is_subset_of` | ~150 ns |
| `NarrowingMatrix::from_capabilities` (4 caps) | ~1.1 µs |
| `NarrowingMatrix::from_capabilities` (16 caps) | ~4.2 µs |
| `NarrowingMatrix::commitment` (Blake3 over 32 bytes) | ~280 ns |
| Single-hop chain authorization | ~5 µs |
| Two-hop scoped chain authorization | ~9 µs |
| `DyoloPassport::guard_local` (end-to-end) | ~12 µs |
| `authorize_batch` (256 intents) | ~820 µs |
| `authorize_batch` (1024 intents) | ~3.3 ms |
| Single gateway process, CPU-bound ceiling | ~200,000 req/s |

> `is_subset_of` is O(1) regardless of capability count — only `from_capabilities` scales with the number of names, and it runs once at issuance time, not on every authorization call.

---

## Compliance

- [SOC 2 Type II control mapping](docs/compliance/soc2-mapping.md)
- [ISO/IEC 27001:2022 Annex A mapping](docs/compliance/iso27001-mapping.md)
- [Sample audit report template](docs/compliance/sample-audit-report.md)

---

## Security model

- **Ed25519 everywhere** — 128-bit security, fast, no weak parameter choices.
- **Blake3 for all hashing** — domain-separated, collision-resistant, hardware-accelerated.
- **Zero unsafe Rust** — `#![deny(unsafe_code)]` enforced at the crate level (isolated `ffi` module documents all contracts explicitly).
- **No global state** — all stores are passed explicitly; no process-level singletons.
- **Offline-first** — authorization never requires a network call.
- **Capability names are hashed, not plaintext** — `DelegationCert` contains only Blake3 hashes of intent values.

See [SECURITY.md](SECURITY.md) for the full threat model and responsible disclosure policy.

---

## Capability listing

| Capability | Description |
|---|---|
| `DyoloPassport` | Long-lived agent identity. Issue once, delegate per task. Save/load as JSON. |
| `NarrowingMatrix` | 256-bit O(1) capability bitmask. Subset enforcement. Blake3 commitment. |
| `CapabilityRegistry` | Collision-free explicit name-to-bit assignment for deployments with 100+ distinct capability names. |
| `ProvableReceipt` | Tamper-evident authorization receipt with verifiable commitment over the enforced scope. |
| `DyoloChain` | Delegation chain builder and verifier. Supports 1-to-N hops with Merkle scope proofs. |
| `DelegationCert` | Wire-format signed credential. Ed25519 signature over all fields. |
| `A1Context` | Builder-pattern entry point. Wires all stores and clock in three lines. |
| `DyoloIdentity` | Ed25519 key pair generator and signer. In-memory; swap for a `VaultSigner` in production. |
| `RevocationStore` | Deny-list for cert fingerprints. Memory, Redis, and Postgres backends included. |
| `NonceStore` | Replay-attack prevention via intent nonce tracking. |
| `RateLimitStore` | Per-principal intent execution rate limiting. |
| `AuditSink` | Composable audit event destination. NDJSON, Datadog, Splunk, OTLP. |
| `PolicySet` | YAML-driven delegation policy with capability restrictions and TTL limits. |
| `VaultSigner` | Abstract signing backend. AWS KMS, GCP KMS, HashiCorp Vault, Azure Key Vault, local file. |
| `ZkChainCommitment` | Zero-knowledge commitment over a delegation chain. Prove authorization without revealing it. |
| `AnchoredReceipt` | On-chain anchor record for Ethereum, Polygon, Base, Arbitrum, and Solana. |
| `SwarmPassport` | Multi-agent swarm coordinator. Issue, add/remove members with scoped roles and TTLs. |
| Middleware (Python) | `@a1_guard` decorator (sync + async). `protect`, `inject_passport`, `a1_context` context helpers. `A1Middleware` class for ASGI frameworks. |
| OTel Tracing (Python) | `A1Tracer` — wraps any `PassportClient` or function to emit structured OTEL spans. Compatible with Datadog APM, Jaeger, Honeycomb, and any OTLP backend. |
| Middleware (TypeScript) | `withA1Passport` higher-order function. `@PassportGuard` class decorator. `A1Middleware` class. `exchangeJwt` for JWT bootstrap. `verifyWebhookSignature` for webhook security. |
| Middleware (Go) | `WithPassport[T, R]` generic guard function. |
| Framework integrations | LangChain, LangGraph, LlamaIndex, AutoGen v0.4, CrewAI, Semantic Kernel, OpenAI Agents. |
| MCP server | Built-in Model Context Protocol server at `/mcp`. Works with Claude Code, Cursor, and any MCP client. |
| CLI (`a1`) | `passport issue`, `passport inspect`, `passport sub`, `keygen`, `verify`, `revoke`, `decode`, `migrate`, `policy`, `completion`. |
| Gateway | Self-hostable REST API. Docker Compose included. Full endpoint reference above. |
| A1 Studio | Web dashboard at `/studio`. Issue, inspect, and manage from a browser. |
| Namespace isolation | `DyoloChain::with_namespace` — hard multi-tenant separation. `X-A1-Tenant-ID` header for REST. |
| Multi-hop batch authorization | `authorize_batch` verifies N intents in a single chain traversal. |
| JWT bridge | `/v1/jwt/exchange` — bootstrap delegation chains from existing OIDC/SAML JWT tokens. |
| Agent negotiation | `/v1/negotiate` — agent-to-agent capability negotiation with freshness enforcement. |
| Wire schema | `wire/schema.json` — stable, versioned JSON schema for all wire types. |
| Discovery document | `/.well-known/a1-configuration` — all endpoint URLs, gateway DID, signing public key. |
| C FFI | `ffi` feature flag exports a C ABI for embedding in Python, Go, Java, Node.js. |
| CBOR serialization | `cbor` feature flag adds binary wire encoding for constrained environments. |
| DID / W3C VC | `did` feature flag: issue and verify W3C Verifiable Credentials tied to agent identities. |
| ZK commitments | `zk` feature flag: `ZkChainCommitment` proves authorization without revealing the chain. |
| Post-quantum hybrid | ML-DSA-44 and ML-DSA-65 hybrid signatures. Zero migration cost. |
| SOC 2 mapping | Annex-level mapping of all Trust Service Criteria to A1 controls. |
| ISO 27001 mapping | Annex A control mapping for certification preparation. |
| Sample audit report | Structured audit report template populated with A1 evidence fields. |

---

## CLI reference

```bash
# Generate an Ed25519 keypair
a1 keygen --out key.json

# Issue a root passport
a1 passport issue --namespace my-agent --allow "trade.equity,portfolio.read" --ttl 30d

# Inspect a passport file
a1 passport inspect passport.json

# Issue a sub-delegation cert
a1 passport sub --passport passport.json --allow trade.equity --ttl 1h --agent-pk <hex>

# Verify a signed chain JSON
a1 verify chain.json --principal-pk <hex>

# Revoke a cert by fingerprint
a1 revoke <fingerprint> --store redis://localhost:6379

# Bulk revoke
a1 revoke-batch <fp1> <fp2> --store redis://localhost:6379

# Decode a raw cert for debugging
a1 decode cert.json

# Apply a YAML policy file
a1 policy -f policy.yaml

# Run Postgres schema migration
a1 migrate

# Generate shell completions
a1 completion bash   # or fish, zsh, powershell
```

---

## Feature flags

| Flag | What it unlocks |
|---|---|
| `serde` | Serialization for all types |
| `wire` | `SignedChain`, `VerifiedToken`, `CertExtensions` |
| `async` | Async storage traits, `AsyncA1Context`, `VaultSigner` |
| `did` | `AgentDid`, `DidDocument`, `VerifiableCredential` |
| `zk` | `ZkChainCommitment`, `ZkProofMode`, `anchor_hash` |
| `anchor` | On-chain anchoring via `anchor_hash` |
| `negotiate` | Algorithm negotiation for hybrid deployments |
| `swarm` | Swarm coordination primitives |
| `governance` | On-chain governance vote recording |
| `tracing` | `tracing` spans during authorization |
| `ffi` | C ABI for Python, Go, Java, Node.js |
| `policy-yaml` | YAML policy file parsing |
| `post-quantum` | Full ML-DSA signature verification |
| `schema` | JSON Schema export for `SignedChain` |
| `cbor` | CBOR serialization for bandwidth-sensitive deployments |
| `otel` | OpenTelemetry spans on every authorization event |
| `full` | All features above (except `post-quantum`) |

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). All contributions require a signed Contributor License Agreement.

---

## License

**MIT OR Apache-2.0.** See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).

---

*A1 is built and maintained by dyolo ([@dyologician](https://github.com/dyologician)).*