# Connecting A1 to Any AI Agent

This guide works for any AI agent, any framework, or any language. If your agent can make HTTP calls, it can use A1.

---

## What you need first

1. **A1 gateway running** — `./setup.sh` (Mac/Linux) or `.\setup.ps1` (Windows)
2. **A passport file** — created from the A1 Studio wizard

---

## The three ways to connect

### Option 1 — Use the Python SDK (recommended for Python agents)

```bash
pip install a1
```

```python
from a1.passport import a1_guard, PassportClient

client = PassportClient(
    gateway_url="http://localhost:8080",
    passport_path="./passport.json",
)

@a1_guard(client=client, capability="your.capability")
async def my_tool(input: str, signed_chain: dict, executor_pk_hex: str) -> str:
    # Your tool logic — only runs if authorized
    return f"Done: {input}"
```

### Option 2 — Use the TypeScript SDK (for Node.js / TypeScript agents)

```bash
npm install a1
```

```typescript
import { withA1Passport, PassportClient } from "a1/passport";

const client = new PassportClient({
    gatewayUrl: "http://localhost:8080",
    passportPath: "./passport.json",
});

const guardedTool = withA1Passport(myTool, {
    client,
    capability: "your.capability",
});
```

### Option 3 — Use the REST API (any language)

No SDK needed. Just make HTTP calls.

**Zero-boilerplate helper (copy-paste for any custom agent)**

If your agent isn't using a supported framework, this helper loads the chain once and injects `signed_chain` and `executor_pk_hex` automatically — you never have to pass them manually in every call:

```python
# a1_helper.py — drop this file next to your agent code
import json, pathlib, requests

class A1Guard:
    """Wraps any callable so it checks A1 authorization first.
    
    Usage:
        guard = A1Guard("passport.json", "chain.json", "http://localhost:8080")
        
        @guard("trade.equity")
        def execute_trade(symbol, qty):
            ...  # only runs if authorized
    """
    def __init__(self, passport_path: str, chain_path: str, gateway: str = "http://localhost:8080"):
        self.chain = json.loads(pathlib.Path(chain_path).read_text())
        passport   = json.loads(pathlib.Path(passport_path).read_text())
        self.pk    = passport["holder_pk_hex"]
        self.gateway = gateway.rstrip("/")

    def __call__(self, capability: str):
        def decorator(fn):
            def wrapper(*args, **kwargs):
                resp = requests.post(
                    f"{self.gateway}/v1/authorize",
                    json={"chain": self.chain, "intent_name": capability,
                          "executor_pk_hex": self.pk},
                    timeout=5,
                )
                result = resp.json()
                if not result.get("authorized"):
                    raise PermissionError(
                        f"A1 denied '{capability}': {result.get('error', 'unknown')}"
                    )
                return fn(*args, **kwargs)
            return wrapper
        return decorator
```

```python
# your_agent.py — no signed_chain or executor_pk_hex needed anywhere
from a1_helper import A1Guard

guard = A1Guard("passport.json", "chain.json")

@guard("trade.equity")
def execute_trade(symbol: str, qty: int):
    broker.buy(symbol=symbol, qty=qty)

@guard("files.read")
def read_report(path: str) -> str:
    return open(path).read()
```

The equivalent TypeScript helper:

```typescript
// a1Helper.ts
import fs from "fs";

export class A1Guard {
  private chain: object;
  private pk: string;
  private gateway: string;

  constructor(passportPath: string, chainPath: string, gateway = "http://localhost:8080") {
    this.chain   = JSON.parse(fs.readFileSync(chainPath, "utf8"));
    const pp     = JSON.parse(fs.readFileSync(passportPath, "utf8"));
    this.pk      = pp.holder_pk_hex;
    this.gateway = gateway.replace(/\/$/, "");
  }

  wrap<T extends (...args: unknown[]) => unknown>(capability: string, fn: T): T {
    return (async (...args: unknown[]) => {
      const res = await fetch(`${this.gateway}/v1/authorize`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ chain: this.chain, intent_name: capability,
                               executor_pk_hex: this.pk }),
      });
      const data = await res.json();
      if (!data.authorized) throw new Error(`A1 denied '${capability}': ${data.error}`);
      return fn(...args);
    }) as T;
  }
}
```

**Authorize an action (raw REST):**
```bash
curl -X POST http://localhost:8080/v1/authorize \
  -H "Content-Type: application/json" \
  -d '{
    "chain": <your-signed-chain-json>,
    "intent_name": "your.capability",
    "executor_pk_hex": "<agent-public-key-hex>"
  }'
```

**Response (authorized):**
```json
{
  "authorized": true,
  "chain_depth": 1,
  "chain_fingerprint": "a3b2c1...",
  "namespace": "my-agent",
  "capability_mask": "0000000000000003..."
}
```

**Response (denied):**
```json
{
  "authorized": false,
  "error": "CapabilityNotGranted",
  "error_code": "E4003"
}
```

---

## How the signed chain works

The `signed_chain` is a JSON object that represents the delegation from the root passport to the executing agent. It is created when you issue the passport and sub-delegation certs.

For simple (single-agent) setups, you generate it once with the CLI:

```bash
# Issue a passport (root identity)
a1 passport issue --namespace my-agent --allow "your.capability" --ttl 30d --out passport.json

# Generate a signed chain for an agent's public key
a1 passport sub \
  --passport passport.json \
  --allow your.capability \
  --ttl 1h \
  --agent-pk <agent-pk-hex> \
  --out chain.json
```

The `chain.json` is your `signed_chain`. Pass it with every authorization request.

---

## Framework-specific quick starts

| Framework | Install | Guide |
|---|---|---|
| LangChain | `pip install "a1[langchain]"` | [templates/langchain.md](langchain.md) |
| CrewAI | `pip install "a1[crewai]"` | [templates/crewai.md](crewai.md) |
| OpenAI Agents | `pip install "a1[openai]"` | [templates/openai-agents.md](openai-agents.md) |
| Claude Code | Prompt-based | [templates/claude-code.md](claude-code.md) |
| TypeScript | `npm install a1` | [sdk/typescript/README.md](../sdk/typescript/README.md) |
| Go | `go get github.com/dyologician/a1/sdk/go/a1` | [sdk/go/README.md](../sdk/go/README.md) |

---

## Capability reference

When you issue a passport, you specify what capabilities the agent has. Use these exact strings:

| String | Meaning |
|---|---|
| `files.read` | Read files and documents |
| `files.write` | Write and create files |
| `code.execute` | Run code or scripts |
| `web.search` | Search the internet |
| `email.send` | Send emails |
| `email.read` | Read emails |
| `database.read` | Query databases |
| `database.write` | Write to databases |
| `trade.equity` | Execute equity trades |
| `portfolio.read` | Read portfolio/account data |
| `api.call` | Call external APIs |
| `agent.delegate` | Delegate tasks to other agents |
| `memory.write` | Write to agent memory |
| `memory.read` | Read from agent memory |

You can also define custom capabilities: any dot-separated string like `payments.send`, `calendar.write`, `crm.update`.

---

## Testing the connection

From A1 Studio (http://localhost:8080/studio), use the **Authorize** tab to test an authorization without running any code.

From the command line:
```bash
curl http://localhost:8080/healthz
```

Expected output:
```json
{"status":"ok","signing_key_hex":"...","version":"2.8.0"}
```

---

## Stopping and restarting

```bash
# Stop A1
docker compose -f docker/docker-compose.yml down

# Start again
./setup.sh
```

Your passport files and configuration are preserved between restarts.

---

## Getting help

- **A1 Studio:** http://localhost:8080/studio (when running)
- **Full docs:** [CAPABILITIES.md](../CAPABILITIES.md)
- **GitHub:** https://github.com/dyologician/a1
- **Issues:** https://github.com/dyologician/a1/issues
