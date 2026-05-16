# A1 Python SDK

[![PyPI](https://img.shields.io/pypi/v/a1.svg)](https://pypi.org/project/a1/)
[![Python](https://img.shields.io/pypi/pyversions/a1.svg)](https://pypi.org/project/a1/)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/dyologician/a1/blob/main/LICENSE-MIT)

Python SDK for [A1](https://github.com/dyologician/a1) — cryptographic chain-of-custody for recursive AI agent delegation.

A1 gives every AI agent a verifiable passport and produces an independently verifiable receipt for every authorized action. It closes the **Recursive Delegation Gap**: the inability to prove, in a multi-agent delegation chain, which human authorized the action at the end.

---

## Requirements

- Python 3.9+
- An A1 gateway running locally or remotely (see [gateway setup](#running-the-gateway))

---

## Installation

**Base client:**

```bash
pip install a1
```

**With framework integrations:**

```bash
pip install "a1[langchain]"        # LangChain
pip install "a1[langgraph]"        # LangGraph
pip install "a1[llamaindex]"       # LlamaIndex
pip install "a1[autogen]"          # AutoGen v0.4
pip install "a1[crewai]"           # CrewAI
pip install "a1[semantic-kernel]"  # Microsoft Semantic Kernel
pip install "a1[openai]"           # OpenAI Agents SDK
pip install "a1[all]"              # All framework integrations
```

**With KMS backends:**

```bash
pip install "a1[vault-aws]"        # AWS KMS
pip install "a1[vault-gcp]"        # GCP Cloud KMS
pip install "a1[vault-hashicorp]"  # HashiCorp Vault Transit
pip install "a1[vault-azure]"      # Azure Key Vault
```

**With SIEM exporters:**

```bash
pip install "a1[siem-datadog]"     # Datadog Logs
pip install "a1[siem-splunk]"      # Splunk HEC
pip install "a1[siem-otel]"        # OpenTelemetry (OTLP)
```

---

## Quick Start

### 1. Start the gateway

```bash
git clone https://github.com/dyologician/a1.git
cd a1
cp .env.example .env      # fill in at minimum A1_PG_PASSWORD
docker compose up -d
```

The gateway listens on `http://localhost:8080`.

### 2. Issue a passport

```bash
cargo install a1-cli
a1 passport issue \
  --namespace my-agent \
  --allow "trade.equity,portfolio.read" \
  --ttl 30d \
  --out passport.json
```

### 3. Guard a function

```python
from a1.passport import a1_guard, PassportClient

client = PassportClient(
    gateway_url="http://localhost:8080",
    passport_path="passport.json",
)

@a1_guard(client=client, capability="trade.equity")
async def execute_trade(symbol: str, qty: int, signed_chain: dict, executor_pk_hex: str) -> dict:
    return {"status": "filled", "symbol": symbol, "qty": qty}
```

The `@a1_guard` decorator calls the gateway's `/v1/passport/authorize` endpoint, verifies authorization, and raises `A1AuthorizationError` if the agent is not authorized.

### 4. Use the base client directly

```python
from a1 import A1Client

client = A1Client(base_url="http://localhost:8080")

result = await client.authorize(
    chain=signed_chain,
    intent_name="trade.equity",
    intent_params={"symbol": "AAPL", "qty": 100},
    executor_pk_hex=agent_pk_hex,
)

print(result.authorized)           # True
print(result.chain_depth)          # 2
print(result.chain_fingerprint)    # hex string
```

---

## MCP — Claude Code and Cursor support

A1 ships a built-in [Model Context Protocol](https://spec.modelcontextprotocol.io/) server. Any MCP-compatible agent or IDE can authorize through A1 **without any code changes**.

Add to your `.mcp.json` (or Claude Code / Cursor settings):

```json
{
  "mcpServers": {
    "a1": {
      "type": "http",
      "url": "http://localhost:8080/mcp"
    }
  }
}
```

The MCP server exposes A1's authorize, cert-issue, revoke, and passport-check tools directly to the agent. No decorators, no imports, one config entry.

---

## A1 Studio

Open `http://localhost:8080/studio` in your browser after starting the gateway. Studio lets you issue passports, inspect delegation chains, manage agents, test MCP tools, and view audit logs — no CLI or code required.

---

## Framework Integrations

### LangChain

```python
from a1.langchain_tool import A1AuthorizationTool
from a1.passport import PassportClient

client = PassportClient(gateway_url="http://localhost:8080", passport_path="passport.json")

tool = A1AuthorizationTool(
    name="execute_trade",
    description="Execute an equity trade. Input: JSON with symbol and qty.",
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
from a1.passport import PassportClient

client = PassportClient(gateway_url="http://localhost:8080", passport_path="passport.json")

class AgentState(A1StateSchema):
    messages: list
    symbol: str
    receipt: dict | None

@a1_node(intent_name="trade.equity", client=client, propagate_receipt=True)
async def execute_trade(state: AgentState) -> AgentState:
    await broker.place_order(state["symbol"])
    return state  # receipt is automatically added to state["receipt"]
```

### LlamaIndex

```python
from a1.llamaindex_tool import a1_llamaindex_tool
from a1.passport import PassportClient

client = PassportClient(gateway_url="http://localhost:8080", passport_path="passport.json")

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
from a1.passport import PassportClient

client = PassportClient(gateway_url="http://localhost:8080", passport_path="passport.json")

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
from a1.passport import PassportClient
from semantic_kernel import Kernel

client = PassportClient(gateway_url="http://localhost:8080", passport_path="passport.json")

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
from a1.passport import PassportClient

client = PassportClient(gateway_url="http://localhost:8080", passport_path="passport.json")

@a1_openai_function(intent_name="trade.equity", client=client)
async def execute_trade(symbol: str, qty: int) -> str:
    return f"Filled {qty} shares of {symbol}"
```

---

## JWT Bridge (OIDC / SSO)

Exchange an existing OIDC or SAML JWT token for a scoped A1 `DelegationCert` — no manual key ceremony required. Useful for enterprises already running SSO.

```python
from a1 import A1Client

client = A1Client(base_url="http://localhost:8080")

cert = await client.exchange_jwt(
    token=jwt_bearer_token,         # your OIDC access token
    capabilities=["files.read"],
    ttl_seconds=3600,
    delegate_pk_hex=agent_pk_hex,
)

# Use cert to build a delegation chain for /v1/authorize
```

Configure the gateway with `A1_JWT_JWKS_URL` and `A1_JWT_ALLOWED_CAPS`.

---

## Delegation Negotiation

An agent can request a scoped cert from the gateway without a pre-shared chain:

```python
from a1 import A1Client

client = A1Client(base_url="http://localhost:8080")

offer = await client.negotiate(
    intent_name="files.read",
    agent_pk_hex=my_public_key_hex,
    requested_ttl_seconds=3600,
)

# offer.cert is a DelegationCert signed by the gateway
```

---

## Swarm Management

```python
from a1 import A1Client

client = A1Client(
    base_url="http://localhost:8080",
    admin_secret=os.environ["A1_ADMIN_SECRET"],
)

# Create a swarm
swarm = await client.create_swarm(
    swarm_name="trading-fleet",
    capabilities=["trade.equity"],
    ttl_days=30,
    signing_key_hex=root_key_hex,
)

# Add a member
await client.add_swarm_member(
    swarm_id=swarm["swarm_id"],
    agent_pk_hex=worker_pk_hex,
    role="worker",
    capabilities=["trade.equity"],
    ttl_seconds=3600,
    signing_key_hex=root_key_hex,
)

# Remove a member
await client.remove_swarm_member(
    swarm_id=swarm["swarm_id"],
    agent_did=worker_did,
)

# List active members
members = await client.list_swarm_members(swarm["swarm_id"])
```

---

## On-chain Anchoring (ZK)

Anchor a zero-knowledge chain commitment on-chain for immutable audit trails:

```python
from a1 import A1Client

client = A1Client(base_url="http://localhost:8080")

anchor = await client.anchor(
    commitment=zk_chain_commitment,
    passport_did="did:a1:...",
    network="ethereum",    # ethereum | polygon | base | arbitrum | solana
)

print(anchor["anchor_hash_hex"])
print(anchor.get("evm_calldata"))          # for EVM chains
print(anchor.get("solana_instruction_data"))  # for Solana
```

---

## Multi-Tenant

Send `X-A1-Tenant-ID` on every request to scope all revocation and nonce operations to that tenant:

```python
from a1 import A1Client

client = A1Client(
    base_url="http://localhost:8080",
    default_headers={"X-A1-Tenant-ID": "acme"},
)

# All authorize, revoke, and nonce calls are now scoped to tenant "acme"
result = await client.authorize(...)
```

Enable on the gateway with `A1_MULTI_TENANT=true`.

---

## Batch Authorization

```python
from a1 import A1Client

client = A1Client(base_url="http://localhost:8080")

results = await client.authorize_batch(
    chain=signed_chain,
    intents=[
        {"name": "trade.equity", "params": {"symbol": "AAPL"}},
        {"name": "portfolio.read"},
    ],
    executor_pk_hex=agent_pk_hex,
)

# All-or-nothing: if any intent fails, no nonces are consumed
for r in results:
    print(r.intent_name, r.authorized)
```

---

## KMS Signing Backends

For production deployments, the root passport key should live in a KMS — not in a file.

```python
from a1.vault import AwsKmsSigner, HashiCorpVaultSigner, GcpKmsSigner, AzureKeyVaultSigner

# AWS KMS (requires: pip install "a1[vault-aws]")
signer = AwsKmsSigner(key_id="alias/a1-passport-root", region="us-east-1")

# HashiCorp Vault Transit (requires: pip install "a1[vault-hashicorp]")
signer = HashiCorpVaultSigner(
    vault_addr="https://vault.corp.example.com",
    key_name="a1-passport-root",
    token=os.environ["VAULT_TOKEN"],
)

# GCP Cloud KMS (requires: pip install "a1[vault-gcp]")
signer = GcpKmsSigner(
    project="my-project",
    location="global",
    key_ring="a1-keys",
    key="passport-root",
    key_version="1",
)

# Azure Key Vault (requires: pip install "a1[vault-azure]")
signer = AzureKeyVaultSigner(
    vault_url="https://my-vault.vault.azure.net",
    key_name="a1-passport-root",
)
```

Verification is always local — zero KMS calls at authorization time.

---

## SIEM Integration

```python
from a1.siem import DatadogLogExporter, SplunkHecExporter, OpenTelemetryExporter, CompositeExporter

exporter = CompositeExporter([
    DatadogLogExporter(
        api_key=os.environ["DD_API_KEY"],
        service="trading-agents",
        env="production",
    ),
    SplunkHecExporter(
        url="https://splunk.corp.com:8088",
        token=os.environ["SPLUNK_HEC_TOKEN"],
        index="ai-audit",
    ),
])

exporter.export_dict(audit_event)
```

OpenTelemetry:

```python
from a1.siem import OpenTelemetryExporter

exporter = OpenTelemetryExporter(
    endpoint="http://otel-collector:4318",
    service_name="trading-agents",
)
```

---

## Middleware (ASGI / WSGI)

A1 includes request-level middleware helpers so you can protect entire routes or inject verified passport context into any async framework (FastAPI, Starlette, Litestar, etc.).

```python
from a1 import protect, inject_passport, a1_context, get_context, A1Context

# FastAPI example — protect a whole router
from fastapi import FastAPI, Depends

app = FastAPI()

@app.middleware("http")
async def a1_middleware(request, call_next):
    return await inject_passport(request, call_next, client=passport_client)

@app.get("/trade")
async def trade(ctx: A1Context = Depends(get_context)):
    print(ctx.namespace)         # "my-trading-agent"
    print(ctx.capability_mask)   # hex bitmask
    return await broker.place_order(...)

# Manual context propagation
set_context(A1Context(namespace="tenant-acme", capability_mask="..."))
ctx = get_context()
```

`MiddlewareError` is raised (and returns HTTP 403) when the passport is missing, expired, or the capability check fails.

---

## OpenTelemetry Tracing

Every authorization attempt can emit a structured OTEL span so your APM (Datadog, Jaeger, Honeycomb, any OTLP backend) has full distributed trace context through agent delegation chains.

```bash
pip install "a1[siem-otel]"
```

```python
from a1.otel import A1Tracer
from a1 import PassportClient

tracer = A1Tracer(service_name="acme-trading-bot")

# Option 1 — wrap an existing PassportClient
client = tracer.instrument_passport_client(PassportClient("http://localhost:8080"))
# All authorize() calls on `client` now emit spans automatically.

# Option 2 — decorator on any async function
@tracer.trace_capability("trade.equity")
async def execute_trade(symbol: str, qty: int) -> dict:
    return await broker.place_order(symbol, qty)
```

All spans use the `dyolo.a1.*` attribute namespace. The `trace_id` and `span_id` are attached to the A1 `AuditEvent` if the gateway's OTEL exporter is configured, giving end-to-end correlation from the HTTP request to the on-disk audit log.

**Noop tracer** (no-op for testing / non-OTEL environments):

```python
from a1.otel import noop_tracer
tracer = noop_tracer()
```

---

## API Reference

### `A1Client`

```python
from a1 import A1Client

client = A1Client(
    base_url="http://localhost:8080",
    timeout=10.0,               # seconds, default 10
    admin_secret=None,          # required for admin endpoints
    default_headers=None,       # e.g. {"X-A1-Tenant-ID": "acme"}
)

# Authorization
await client.authorize(chain, intent_name, intent_params, executor_pk_hex)
await client.authorize_batch(chain, intents, executor_pk_hex)

# Certificates (admin)
await client.issue_cert(delegate_pk_hex, intents, ttl_seconds, extensions)
await client.revoke_cert(fingerprint)
await client.revoke_certs_batch(fingerprints)

# JWT bridge
await client.exchange_jwt(token, capabilities, ttl_seconds, delegate_pk_hex)

# Negotiation
await client.negotiate(intent_name, agent_pk_hex, requested_ttl_seconds)

# Anchoring
await client.anchor(commitment, passport_did, network)

# Swarm (admin)
await client.create_swarm(swarm_name, capabilities, ttl_days, signing_key_hex)
await client.add_swarm_member(swarm_id, agent_pk_hex, role, capabilities, ttl_seconds, signing_key_hex)
await client.remove_swarm_member(swarm_id, agent_did)
await client.list_swarm_members(swarm_id)

# Utility
await client.health()
await client.well_known()
```

### `PassportClient`

```python
from a1.passport import PassportClient

client = PassportClient(
    gateway_url="http://localhost:8080",
    passport_path="passport.json",
    signer=None,           # optional VaultSigner for KMS-backed keys
    admin_secret=None,
)

await client.authorize(intent_name, executor_pk_hex)
await client.issue_sub(delegate_pk_hex, capabilities, ttl_seconds)
await client.inspect()
```

### `@a1_guard`

```python
from a1.passport import a1_guard, PassportClient

@a1_guard(client=client, capability="trade.equity")
async def my_tool(param: str, signed_chain: dict, executor_pk_hex: str) -> str:
    ...
```

The decorator injects `signed_chain` and `executor_pk_hex` if not provided by the caller. Raises `A1AuthorizationError` if the gateway rejects the authorization.

---

## Error Handling

```python
from a1.client import A1AuthorizationError, A1GatewayError

try:
    result = await client.authorize(...)
except A1AuthorizationError as e:
    # Denied: expired cert, scope violation, replay attack, etc.
    print(f"Denied: {e.reason}, error_code={e.error_code}")
except A1GatewayError as e:
    # Network error or unexpected gateway status
    print(f"Gateway error: {e}")
```

---

## Running the Gateway

```bash
git clone https://github.com/dyologician/a1.git
cd a1
cp .env.example .env      # fill in at minimum A1_PG_PASSWORD
docker compose up -d
curl http://localhost:8080/healthz
```

Key environment variables:

| Variable | Description |
|---|---|
| `A1_SIGNING_KEY_HEX` | 32-byte hex Ed25519 seed — **generate and set in production** |
| `A1_MAC_KEY_HEX` | 32-byte hex HMAC key — **generate and set in production** |
| `A1_ADMIN_SECRET` | Bearer token for admin endpoints — **required in production** |
| `A1_REDIS_URL` | Redis URL for production nonce/revocation stores |
| `A1_PG_URL` | Postgres URL for production nonce/revocation stores |
| `A1_JWT_JWKS_URL` | JWKS endpoint for JWT bridge (`/v1/jwt/exchange`) |
| `A1_NEGOTIATE_CAPABILITIES` | Comma-separated caps the negotiate endpoint may issue |
| `A1_MULTI_TENANT` | Set `true` to enable `X-A1-Tenant-ID` header enforcement |
| `A1_AI_KEY` | Anthropic API key — enables AI proxy for Studio |
| `GATEWAY_ADDR` | Bind address (default: `0.0.0.0:8080`) |

See `.env.example` in the repository root for the full list.

---

## Testing

```bash
pip install -e ".[dev]"
pytest
pytest --cov=a1 --cov-report=term-missing
```

---

## License

MIT OR Apache-2.0. See [LICENSE-MIT](https://github.com/dyologician/a1/blob/main/LICENSE-MIT) and [LICENSE-APACHE](https://github.com/dyologician/a1/blob/main/LICENSE-APACHE).

---

*Part of the [A1](https://github.com/dyologician/a1) ecosystem. Built and maintained by dyolo ([@dyologician](https://github.com/dyologician)).*