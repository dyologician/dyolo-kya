# LangChain Integration Guide

Adds A1 authorization to any LangChain tool in three steps.

## Prerequisites

```bash
pip install a1 langchain langchain-openai
docker run -p 8080:8080 ghcr.io/dyologician/a1-gateway:2
```

## Step 1 — Issue a delegation cert for your agent

```python
from a1 import A1Client, IntentSpec

client = A1Client("http://localhost:8080")

cert = client.issue_cert(
    delegate_pk_hex="<agent-ed25519-pk-hex>",
    intents=[
        IntentSpec("trade.equity", {"exchange": "NYSE"}),
        IntentSpec("portfolio.read"),
    ],
    ttl_seconds=3600,
    extensions={"a1.cost_center": "ai-ops"},
)

# Serialize the chain to pass through your session context
signed_chain = client.build_chain(cert)
```

## Step 2 — Decorate your tool function

```python
from a1.langchain_tool import a1_tool

@a1_tool(
    name="execute_trade",
    description="Execute an equity trade. Input: JSON {symbol, qty}.",
    intent_name="trade.equity",
    client=client,
    chain=signed_chain,
    executor_pk_hex="<agent-ed25519-pk-hex>",
)
def execute_trade(tool_input: str) -> str:
    import json, your_broker
    args = json.loads(tool_input)
    your_broker.buy(args["symbol"], args["qty"])
    return f"Filled: {args['qty']} × {args['symbol']}"
```

## Step 3 — Pass the tool to your agent

```python
from langchain.agents import AgentExecutor, create_openai_tools_agent
from langchain_openai import ChatOpenAI
from langchain_core.prompts import ChatPromptTemplate, MessagesPlaceholder

llm    = ChatOpenAI(model="gpt-4o")
prompt = ChatPromptTemplate.from_messages([
    ("system", "You are a trading assistant."),
    ("human", "{input}"),
    MessagesPlaceholder("agent_scratchpad"),
])

agent    = create_openai_tools_agent(llm, [execute_trade], prompt)
executor = AgentExecutor(agent=agent, tools=[execute_trade])
executor.invoke({"input": "Buy 10 shares of AAPL."})
```

## What happens on each tool call

1. The LLM produces a `tool_call` for `execute_trade`.
2. The `a1_tool` wrapper calls `POST /v1/authorize` on the gateway.
3. The gateway verifies the full cert chain: signature validity → scope
   commitment → expiry → nonce replay → revocation.
4. If verification passes, your tool function runs and returns.
5. If verification fails, `A1Error` is raised and the LLM receives an
   authorization denied message.

The authorization result includes `chain_depth` and `chain_fingerprint` which
your tool can include in its audit log for compliance purposes.

## Full example

See [`examples/integrations/langchain_example.py`](../../examples/integrations/langchain_example.py).
