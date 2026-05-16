# A1 Integration Guide

**Connecting A1 to AI frameworks, IDEs, cloud services, and enterprise infrastructure.**

This guide is organized by who you are. Jump to your section.

---

## Contents

- [For everyone — the 30-second mental model](#for-everyone)
- [MCP integration (Claude Code, Cursor, any MCP client)](#mcp-integration)
- [Python frameworks](#python-frameworks)
- [TypeScript / Node.js frameworks](#typescript--nodejs-frameworks)
- [Go](#go)
- [Rust (direct embedding)](#rust-direct-embedding)
- [Enterprise infrastructure](#enterprise-infrastructure)
- [What A1 does NOT replace](#what-a1-does-not-replace)

---

## For everyone

A1 sits between your agent and the action it wants to take. It checks three things:

1. **Is this agent allowed to do this?** (capability check)
2. **Was this actually authorized by a human?** (chain verification)
3. **Is the authorization still valid?** (expiry + revocation check)

If any check fails, the action is blocked — cryptographically, not just by a policy rule.

You integrate A1 by:
1. Starting the gateway (`docker compose up -d` or `a1 start`)
2. Issuing a passport for your agent (`a1 passport issue`)
3. Adding one decorator / wrapper to each protected function

That's it. Everything else in this guide is optional depth.

---

## MCP Integration

**For: Claude Code, Cursor, and any MCP-compatible agent or IDE extension.**

A1 ships a built-in [Model Context Protocol](https://spec.modelcontextprotocol.io/) server. MCP-compatible tools can authorize through A1 **without any code changes**.

**Step 1 — Start the gateway**

```bash
docker compose up -d
```

**Step 2 — Add to your MCP config**

In your `.mcp.json` file or Claude Code settings:

```json
{
  "mcpServers": {
    "a1": {
      "url": "http://localhost:8080/mcp"
    }
  }
}
```

**MCP endpoints:**

| Method | Path | Description |
|---|---|---|
| `GET` | `/mcp` | SSE stream — persistent MCP session |
| `POST` | `/mcp` | JSON-RPC 2.0 — single request/response |
| `GET` | `/mcp/tools` | List all available A1 tools |

The MCP server exposes A1's `authorize`, `cert-issue`, `revoke`, and `passport-check` tools directly to the agent. No decorators. No imports. One config entry.

---

## Python Frameworks

### Any Python function (one decorator)

```python
from a1.passport import PassportClient, a1_guard

client = PassportClient("http://localhost:8080")

@a1_guard(client=client, capability="trade.equity")
async def execute_trade(symbol: str, qty: int, signed_chain: dict, executor_pk_hex: str) -> dict:
    return await broker.place_order(symbol, qty)
```

The decorator reads `signed_chain` and `executor_pk_hex` from keyword arguments, calls the gateway, and either runs your function or raises `PassportError`.

### ASGI / FastAPI middleware

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

### LangChain

```bash
pip install "a1[langchain]"
```

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

```bash
pip install "a1[langgraph]"
```

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

```bash
pip install "a1[llamaindex]"
```

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

```bash
pip install "a1[autogen]"
```

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

```bash
pip install "a1[crewai]"
```

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

```bash
pip install "a1[semantic-kernel]"
```

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

```bash
pip install "a1[openai]"
```

```python
from a1.openai_tool import a1_openai_function

@a1_openai_function(intent_name="trade.equity", client=client)
async def execute_trade(symbol: str, qty: int) -> str:
    return await broker.place_order(symbol, qty)
```

### Swarm management (Python)

Coordinate a fleet of agents with role-based scoped certs:

```python
from a1.swarm import SwarmClient, SwarmRole

client = SwarmClient(
    "http://localhost:8080",
    admin_secret=os.environ["A1_ADMIN_SECRET"],
)

# Create a swarm and issue a root swarm passport
swarm_id = client.create_swarm(
    name="acme-trading-swarm",
    capabilities=["trade.equity", "portfolio.read"],
    ttl_days=30,
    signing_key_hex=ROOT_SK_HEX,
)

# Add a scoped worker agent
cert = client.add_member(
    swarm_id=swarm_id,
    agent_pk_hex=WORKER_PK_HEX,
    role=SwarmRole.WORKER,
    capabilities=["trade.equity"],   # subset of swarm capabilities
    ttl_seconds=3600,
    signing_key_hex=ROOT_SK_HEX,
)

# Remove a member when done
client.remove_member(swarm_id=swarm_id, agent_pk_hex=WORKER_PK_HEX, signing_key_hex=ROOT_SK_HEX)

# List active members
members = client.list_members(swarm_id=swarm_id)
```

**Roles:** `SwarmRole.ORCHESTRATOR`, `SwarmRole.SUPERVISOR`, `SwarmRole.AUDITOR`, `SwarmRole.WORKER`.

### Install all framework integrations at once

```bash
pip install "a1[all]"
```

---

## TypeScript / Node.js Frameworks

```bash
npm install a1
```

### Any async function (one wrapper)

```typescript
import { withA1Passport, PassportClient } from "a1/passport";

const client = new PassportClient("http://localhost:8080");

const guardedTrade = withA1Passport(executeTrade, {
  client,
  capability: "trade.equity",
});
```

### Class decorator

```typescript
import { PassportGuard } from "a1/passport";

class TradingAgent {
  @PassportGuard({ client, capability: "trade.equity" })
  async executeTrade(args: TradeArgs): Promise<TradeResult> {
    return broker.place_order(args);
  }
}
```

### Express / Next.js / Hono middleware

```typescript
import { A1Middleware } from "a1/middleware";

app.use(A1Middleware({
  client,
  capability: "api.write",
}));
```

### JWT bootstrap (SSO / OIDC)

Exchange an existing OIDC JWT for an A1 delegation cert:

```typescript
import { exchangeJwt } from "a1";

const cert = await exchangeJwt({
  gatewayUrl: "http://localhost:8080",
  token: oidcJwt,
  capabilities: ["files.read"],
  ttlSeconds: 3600,
  delegatePkHex: agentPublicKey,
});
```

### Webhook signature verification

```typescript
import { verifyWebhookSignature } from "a1";

app.post("/a1-webhook", (req, res) => {
  const valid = verifyWebhookSignature(req.body, req.headers["x-a1-signature"], webhookSecret);
  if (!valid) return res.status(401).end();
  // process event
});
```

### LangGraph (TypeScript)

```typescript
import { withDyoloLangGraphNode } from "a1/integrations";

const guardedNode = withDyoloLangGraphNode(executeTradeNode, {
  client,
  capability: "trade.equity",
});
```

### Semantic Kernel (TypeScript)

```typescript
import { withDyoloSkFunction } from "a1/integrations";

const guardedFn = withDyoloSkFunction(executeTradeSkFn, {
  client,
  capability: "trade.equity",
});
```

### Swarm management (TypeScript)

Coordinate a fleet of agents with role-based scoped certs:

```typescript
import { SwarmClient, SwarmRole } from "a1/swarm";

const swarm = new SwarmClient("http://localhost:8080", {
  adminSecret: process.env.A1_ADMIN_SECRET,
});

// Create a swarm and issue a root swarm passport
const swarmId = await swarm.createSwarm({
  name: "acme-trading-swarm",
  capabilities: ["trade.equity", "portfolio.read"],
  ttlDays: 30,
  signingKeyHex: ROOT_SK_HEX,
});

// Add a scoped worker
const member = await swarm.addMember({
  swarmId,
  agentPkHex: WORKER_PK_HEX,
  role: SwarmRole.Worker,
  capabilities: ["trade.equity"],   // subset of swarm capabilities
  ttlSeconds: 3600,
  signingKeyHex: ROOT_SK_HEX,
});

// Remove a member when a task is done
await swarm.removeMember({ swarmId, agentPkHex: WORKER_PK_HEX, signingKeyHex: ROOT_SK_HEX });

// List all active members
const members = await swarm.listMembers(swarmId);
```

**Roles:** `SwarmRole.Orchestrator`, `SwarmRole.Supervisor`, `SwarmRole.Auditor`, `SwarmRole.Worker`.

---

## Go

```bash
go get github.com/dyologician/a1/sdk/go/a1/kya
```

```go
import a1 "github.com/dyologician/a1/sdk/go/a1/kya"

guarded := a1.WithPassport(executeTrade, passport)
result, err := guarded(ctx, args)
```

---

## Rust (direct embedding)

No gateway required — authorize locally with zero network hops.

```toml
# Cargo.toml
[dependencies]
a1 = { version = "2.8", features = ["full"] }
```

```rust
use a1::{DyoloPassport, DyoloIdentity, Intent, SystemClock};

// Issue a root passport (do this once; save to disk or KMS)
let root     = DyoloIdentity::generate();
let passport = DyoloPassport::issue(
    "acme-trading-bot",
    &["trade.equity", "portfolio.read"],
    30 * 24 * 3600,
    &root,
    &SystemClock,
)?;
passport.save("passport.json")?;

// At task time: issue a sub-cert for the executing agent
let agent    = DyoloIdentity::generate();
let sub_cert = passport.issue_sub(
    agent.verifying_key(),
    &["trade.equity"],
    3600,
    &root,
    &SystemClock,
)?;

// Build the chain and guard the action
let mut chain = passport.new_chain()?;
chain.push(sub_cert);

let intent  = Intent::new("trade.equity")?;
let receipt = passport.guard_local(&chain, &agent.verifying_key(), &intent)?;

// Archive the receipt for audit
println!("{}", receipt);
assert!(receipt.verify_commitment());
```

### Embedding via C FFI

If you're embedding A1 in Python, Go, Java, or Node.js without an HTTP hop:

```c
// Feature flag: features = ["ffi"]
a1_context_t* ctx = a1_context_new();
int ok = a1_authorize(ctx, chain_json, chain_len, intent_json, intent_len, receipt_out, &receipt_len);
a1_context_free(ctx);
```

See `src/ffi.rs` and `cbindgen.toml` for the full exported ABI.

---

## Enterprise Infrastructure

### KMS signing backends

Replace the local key file with your organization's KMS. The verifying key is embedded in every cert — zero KMS calls at authorization time.

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
    project="my-project", location="global",
    key_ring="a1-keys", key="passport-root",
)

# Azure Key Vault
signer = AzureKeyVaultSigner(
    vault_url="https://my-vault.vault.azure.net",
    key_name="a1-passport-root",
)
```

Full KMS setup guide: [`wiki/KMS-Integration.md`](wiki/KMS-Integration.md)

### SIEM / audit log export

```python
from a1.siem import DatadogLogExporter, SplunkHecExporter, CompositeExporter, OpenTelemetryExporter

# Multiple SIEM destinations simultaneously
exporter = CompositeExporter([
    DatadogLogExporter(api_key=os.environ["DD_API_KEY"], service="trading-agents"),
    SplunkHecExporter(url="https://splunk.corp.com:8088", token=os.environ["SPLUNK_HEC_TOKEN"]),
    OpenTelemetryExporter(endpoint="http://otel-collector:4318", service_name="agents"),
])

exporter.export_dict(audit_event)
```

### OpenTelemetry tracing (Python)

```python
from a1.otel import A1Tracer

tracer = A1Tracer(client=PassportClient("http://localhost:8080"), service_name="trading-agents")

with tracer.start_span("execute_trade"):
    result = await execute_trade(symbol="AAPL", qty=10, signed_chain=chain, executor_pk_hex=pk)
```

### Multi-tenant isolation

```rust
// Rust
let ctx = A1Context::builder()
    .namespace("tenant-acme")
    .build();
let action = ctx.authorize(&chain, &agent_pk, &intent, &proof)?;
```

```bash
# REST gateway
curl -H "X-A1-Tenant-ID: acme" http://localhost:8080/v1/authorize ...
```

Set `A1_MULTI_TENANT=true` on the gateway to enforce the header. Set `A1_TENANT_REQUIRED=true` to reject requests without it.

### PostgreSQL and Redis backends

```bash
# .env or deployment secrets
A1_REDIS_URL=redis://localhost:6379
# OR
A1_PG_URL=postgres://user:password@localhost/a1
```

```toml
# Cargo.toml — if embedding in Rust
[dependencies]
a1-pg    = "2.8"   # Postgres nonce + revocation stores
a1-redis = "2.8"   # Redis nonce + revocation stores
```

Run the schema migration once after setting `A1_PG_URL`:

```bash
a1 migrate
```

### Kubernetes deployment

See [`wiki/Enterprise-Deployment.md`](wiki/Enterprise-Deployment.md) for a complete Kubernetes manifest, including Deployment, Service, Ingress, and Secret references for `A1_SIGNING_KEY_HEX` and `A1_ADMIN_SECRET`.

---

## What A1 does NOT replace

A1 is the **delegation accountability layer**. It is not a drop-in for:

| What you already have | A1 relationship |
|---|---|
| Authentication (Auth0, Okta, SAML, OIDC) | A1 can **bootstrap** from your OIDC JWT via `/v1/jwt/exchange` |
| API keys / service accounts | A1 wraps these with a provable chain on top |
| Audit logs (Splunk, Datadog) | A1 **feeds** these via `AuditSink` — it's additive |
| Secrets management (Vault, AWS Secrets Manager) | A1 **uses** these for signing keys via `VaultSigner` |
| Network policy (Istio, Envoy) | A1 operates at the authorization layer above network policy |

---

*Full API reference: [`README.md`](README.md) · Full capability reference: [`CAPABILITIES.md`](CAPABILITIES.md) · Enterprise deployment: [`wiki/Enterprise-Deployment.md`](wiki/Enterprise-Deployment.md)*