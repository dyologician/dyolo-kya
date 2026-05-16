# Connecting A1 to LangChain

---

## Install

```bash
pip install "a1[langchain]"
```

---

## Quick integration — tool wrapper

```python
from a1.langchain_tool import A1AuthorizationTool
from a1.passport import PassportClient

client = PassportClient(
    gateway_url="http://localhost:8080",
    passport_path="./passport.json",
)

# Wrap your existing function as an A1-protected LangChain tool
tool = A1AuthorizationTool(
    name="execute_trade",
    description="Execute an equity trade. Input: JSON with symbol and qty.",
    intent_name="trade.equity",
    client=client,
    func=execute_trade_fn,          # your existing function
    chain=agent_chain,              # signed delegation chain
    executor_pk_hex=agent_pk,       # agent's public key hex
)

# Use like any LangChain tool
from langchain.agents import initialize_agent, AgentType
agent = initialize_agent([tool], llm, agent=AgentType.ZERO_SHOT_REACT_DESCRIPTION)
```

---

## Quick integration — decorator

```python
from a1.passport import a1_guard, PassportClient
from langchain.tools import tool

client = PassportClient(
    gateway_url="http://localhost:8080",
    passport_path="./passport.json",
)

@tool
@a1_guard(client=client, capability="web.search")
async def search_web(query: str, signed_chain: dict, executor_pk_hex: str) -> str:
    """Search the web for information."""
    return await my_search_fn(query)
```

---

## Full example

See [examples/integrations/langchain_example.py](../examples/integrations/langchain_example.py).

---

## Need help?

- A1 Studio: http://localhost:8080/studio
- Full docs: [CAPABILITIES.md](../CAPABILITIES.md)
- GitHub: https://github.com/dyologician/a1
