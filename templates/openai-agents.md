# Connecting A1 to OpenAI Agents SDK

---

## Install

```bash
pip install "a1[openai]"
```

---

## Quick integration

```python
from a1.passport import a1_guard, PassportClient
from openai import OpenAI

# Set up A1
client = PassportClient(
    gateway_url="http://localhost:8080",
    passport_path="./passport.json",
)

# Protect your tool with one decorator
@a1_guard(client=client, capability="files.read")
async def read_file(path: str, signed_chain: dict, executor_pk_hex: str) -> str:
    """Read a file — protected by A1 authorization."""
    with open(path) as f:
        return f.read()

# Use with OpenAI Agents normally
openai_client = OpenAI()
# ... rest of your agent setup
```

---

## Using the OpenAI tool helper

```python
from a1.openai_tool import a1_openai_function
from a1.passport import PassportClient

client = PassportClient(
    gateway_url="http://localhost:8080",
    passport_path="./passport.json",
)

@a1_openai_function(intent_name="trade.equity", client=client)
async def execute_trade(symbol: str, qty: int) -> str:
    """Execute an equity trade."""
    return f"Filled {qty} shares of {symbol}"
```

---

## How authorization works

When the decorated function is called:

1. A1 receives the `signed_chain` and `executor_pk_hex`
2. A1 verifies the delegation chain cryptographically
3. A1 checks that `trade.equity` is in the authorized capability set
4. If all checks pass → your function runs and returns a `ProvableReceipt`
5. If any check fails → `A1AuthorizationError` is raised and your function does NOT run

---

## Error handling

```python
from a1.client import A1AuthorizationError, A1GatewayError

try:
    result = await execute_trade("AAPL", 100, chain, agent_pk)
except A1AuthorizationError as e:
    print(f"Not authorized: {e.reason}")  # e.g. "CapabilityNotGranted"
except A1GatewayError as e:
    print(f"Gateway error: {e}")          # Network or config issue
```

---

## Full example

See [examples/integrations/openai_agents_example.py](../examples/integrations/openai_agents_example.py) for a complete working example including passport setup, chain building, and authorization.

---

## Need help?

- A1 Studio: http://localhost:8080/studio
- Full docs: [CAPABILITIES.md](../CAPABILITIES.md)
- GitHub: https://github.com/dyologician/a1
