# Python SDK

The A1 Python SDK (`a1identity` on PyPI, `a1` as the import module) gives any Python AI agent framework a one-decorator authorization gate backed by cryptographic chain-of-custody.

---

## Installation

```bash
pip install a1identity                    # Base client + passport guard
pip install "a1identity[langchain]"       # LangChain integration
pip install "a1identity[langgraph]"       # LangGraph integration
pip install "a1identity[llamaindex]"      # LlamaIndex integration
pip install "a1identity[autogen]"         # AutoGen / AutoGen Studio
pip install "a1identity[crewai]"          # CrewAI
pip install "a1identity[semantic-kernel]" # Microsoft Semantic Kernel
pip install "a1identity[openai]"          # OpenAI Agents SDK
pip install "a1identity[vault-aws]"       # AWS KMS signing
pip install "a1identity[vault-gcp]"       # Google Cloud KMS signing
pip install "a1identity[vault-hashicorp]" # HashiCorp Vault Transit
pip install "a1identity[vault-azure]"     # Azure Key Vault
pip install "a1identity[siem-datadog]"    # Datadog audit export
pip install "a1identity[siem-splunk]"     # Splunk HEC audit export
pip install "a1identity[siem-otel]"       # OpenTelemetry audit export
pip install "a1identity[all]"             # All framework integrations
```

---

## Start the gateway

```bash
git clone https://github.com/dyologician/A1
cd A1
./setup.sh
```

Studio and the REST API are available at `http://localhost:8080`.

---

## Core client

```python
from a1 import A1Client, AsyncA1Client, IntentSpec

# Synchronous
client = A1Client("http://localhost:8080")

result = client.authorize(
    chain=signed_chain,
    intent_name="trade.equity",
    intent_params={"symbol": "AAPL"},
    executor_pk_hex=agent_pk_hex,
)
print(result.authorized)       # True
print(result.chain_depth)      # 2
print(result.chain_fingerprint) # "a3b2c1..."

# Async
async_client = AsyncA1Client("http://localhost:8080")
result = await async_client.authorize(...)
```

---

## Passport guard (`@a1_guard`)

The one-decorator pattern. Works on any sync or async Python function.

```python
from a1.passport import PassportClient, a1_guard

client = PassportClient("http://localhost:8080")

@a1_guard(client=client, capability="trade.equity")
async def execute_trade(symbol: str, qty: int, signed_chain: dict, executor_pk_hex: str) -> dict:
    # Only runs after the gateway confirms the chain is valid.
    return await broker.place_order(symbol=symbol, qty=qty)

# Call normally — a1_guard reads signed_chain and executor_pk_hex from kwargs:
result = await execute_trade(
    symbol="AAPL",
    qty=10,
    signed_chain=chain,
    executor_pk_hex=agent_pk_hex,
)
```

### Handling authorization failures

```python
from a1.passport import PassportError

try:
    result = await execute_trade(...)
except PassportError as e:
    print(e.error_code)   # "PASSPORT_NARROWING_VIOLATION"
    print(e.http_status)  # 403
    print(e.message)      # "agent does not hold trade.equity"
```

---

## Middleware (`protect`, `a1_context`)

For ASGI/WSGI frameworks that need to propagate the verified passport through a request context.

```python
from a1.middleware import protect, a1_context, get_context, A1Context

# FastAPI example
from fastapi import FastAPI, Depends
app = FastAPI()

@app.post("/trade")
@protect(client=client, capability="trade.equity")
async def trade_endpoint(request: Request):
    ctx: A1Context = get_context()
    print(ctx.namespace)         # "trading-bot"
    print(ctx.chain_depth)       # 2
    print(ctx.capability_mask)   # "a3b2..."
    return {"status": "ok"}
```

---

## Framework integrations

### LangChain

```python
from langchain_core.tools import tool
from a1.langchain_tool import a1_tool

@tool
def execute_trade(symbol: str, qty: int) -> str:
    return f"Bought {qty} shares of {symbol}"

secured = a1_tool(
    execute_trade,
    chain=CHAIN_JSON,
    executor_pk_hex=AGENT_PUBLIC_KEY,
    intent_name="trade.equity",
    intent_params={"symbol": "AAPL"},
)
```

### LangGraph

```python
from a1.langgraph_tool import a1_node

@a1_node(client=client, capability="trade.equity")
async def trade_node(state: dict) -> dict:
    # state["signed_chain"] and state["executor_pk_hex"] are verified before this runs
    return {"result": "filled"}
```

### CrewAI

```python
from crewai import Agent, Task
from a1.crewai_tool import A1CrewAITool

tool = A1CrewAITool(
    name="execute_trade",
    description="Execute an equity trade",
    capability="trade.equity",
    gateway_url="http://localhost:8080",
)

trading_agent = Agent(role="Trader", tools=[tool])
```

### AutoGen

```python
from a1.autogen_tool import build_a1_function_tool

fn_tool = build_a1_function_tool(
    capability="trade.equity",
    gateway_url="http://localhost:8080",
    fn=execute_trade,
)
```

### OpenAI Agents SDK

```python
from a1.openai_tool import a1_openai_tool

tool = a1_openai_tool(
    fn=execute_trade,
    capability="trade.equity",
    gateway_url="http://localhost:8080",
)
```

---

## KMS signing (VaultSigner)

Root keys never touch application memory.

```python
from a1.vault import AwsKmsSigner, HashiCorpVaultSigner, GcpKmsSigner, AzureKeyVaultSigner

# AWS KMS
signer = AwsKmsSigner(key_id="alias/a1-passport-root", region="us-east-1")

# HashiCorp Vault Transit
signer = HashiCorpVaultSigner(
    vault_addr="https://vault.corp.example.com",
    key_name="a1-root",
    token=os.environ["VAULT_TOKEN"],
)

# GCP KMS
signer = GcpKmsSigner(
    project="my-project",
    location="global",
    key_ring="a1-keys",
    key_name="root",
)

# Azure Key Vault
signer = AzureKeyVaultSigner(
    vault_url="https://mykeyvault.vault.azure.net",
    key_name="a1-passport-root",
)
```

---

## SIEM audit export

```python
from a1.siem import DatadogLogExporter, SplunkHecExporter, CompositeExporter, NdjsonFileExporter

# Single exporter
dd = DatadogLogExporter(api_key=os.environ["DD_API_KEY"], service="trading-agents")

# Fan-out to multiple destinations
exporter = CompositeExporter([
    DatadogLogExporter(api_key=os.environ["DD_API_KEY"]),
    SplunkHecExporter(url="https://splunk.corp.example.com:8088", token=os.environ["SPLUNK_TOKEN"]),
    NdjsonFileExporter(path="/var/log/a1-audit.jsonl"),
])

# Export an authorization event
exporter.export(auth_event_dict)
```

---

## OpenTelemetry tracing

```python
from a1.otel import A1Tracer

tracer = A1Tracer(service_name="trading-service")

# Wrap any PassportClient call with a trace span
with tracer.trace_capability("trade.equity") as span:
    result = await client.authorize(...)
    span.set_attribute("chain_depth", result.chain_depth)
```

Requires: `pip install "a1identity[siem-otel]"`

---

## Swarm coordination

```python
from a1.swarm import SwarmClient, SwarmRole

client = SwarmClient("http://localhost:8080")

swarm_id = client.create_swarm(
    name="trading-swarm",
    capabilities=["trade.equity", "portfolio.read"],
    ttl_days=30,
    signing_key_hex=ROOT_SK_HEX,
)

cert = client.add_member(
    swarm_id=swarm_id,
    agent_pk_hex=WORKER_PK_HEX,
    role=SwarmRole.WORKER,
    capabilities=["trade.equity"],
    ttl_seconds=3600,
    signing_key_hex=ROOT_SK_HEX,
)

members = client.list_members(swarm_id)
```

---

## Key exports

| Import | Class / Function | Description |
|---|---|---|
| `from a1 import` | `A1Client` | Synchronous HTTP client |
| `from a1 import` | `AsyncA1Client` | Async HTTP client |
| `from a1 import` | `A1Error` | Base exception for gateway errors |
| `from a1.passport import` | `PassportClient` | Passport lifecycle + chain building |
| `from a1.passport import` | `a1_guard` | One-decorator authorization gate |
| `from a1.passport import` | `PassportError` | Authorization failure exception |
| `from a1.middleware import` | `protect` | ASGI/WSGI request guard |
| `from a1.middleware import` | `a1_context` / `get_context` | Request context propagation |
| `from a1.vault import` | `AwsKmsSigner` etc. | KMS signing backends |
| `from a1.siem import` | `DatadogLogExporter` etc. | SIEM audit exporters |
| `from a1.otel import` | `A1Tracer` | OpenTelemetry spans |
| `from a1.swarm import` | `SwarmClient` | Swarm management |
| `from a1.langchain_tool import` | `a1_tool` | LangChain guard |
| `from a1.langgraph_tool import` | `a1_node` | LangGraph node guard |
| `from a1.crewai_tool import` | `A1CrewAITool` | CrewAI tool |
| `from a1.autogen_tool import` | `build_a1_function_tool` | AutoGen function tool |
| `from a1.openai_tool import` | `a1_openai_tool` | OpenAI Agents SDK tool |

---

*Source: `sdk/python/a1/` · [Back to wiki home](Home)*
