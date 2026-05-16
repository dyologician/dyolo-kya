# OpenAI Assistants / Agents SDK Integration Guide

Gate every OpenAI function tool call with a cryptographic authorization check.

## Prerequisites

```bash
# Python
pip install a1 openai

# TypeScript
npm install a1 openai

docker run -p 8080:8080 ghcr.io/dyologician/a1-gateway:2
```

## Python — `a1_function_guard` decorator

```python
import os, json
from a1 import A1Client
from a1.openai_tool import a1_function_guard

client = A1Client("http://localhost:8080")

AGENT_PK    = os.environ["AGENT_PK_HEX"]
AGENT_CHAIN = json.loads(os.environ["AGENT_SIGNED_CHAIN"])

@a1_function_guard(
    intent_name="trade.equity",
    client=client,
    chain=AGENT_CHAIN,
    executor_pk_hex=AGENT_PK,
)
def execute_trade(symbol: str, qty: int) -> dict:
    import your_broker
    your_broker.buy(symbol, qty)
    return {"status": "filled", "symbol": symbol, "qty": qty}
```

Register the tool schema with the Assistants API and dispatch tool calls to the
decorated function.  The decorator raises `A1Error` before your function body
runs if authorization fails.

## TypeScript — `buildOpenAIA1Function`

```ts
import { A1Client, SignedChain } from "a1";
import { buildOpenAIA1Function } from "a1/integrations";

const client = new A1Client("http://localhost:8080");
const agentChain  = JSON.parse(process.env.AGENT_SIGNED_CHAIN!) as SignedChain;
const agentPk     = process.env.AGENT_PK_HEX!;

const tradeTool = buildOpenAIA1Function({
  name: "execute_trade",
  description: "Execute an equity trade",
  parameters: {
    type: "object",
    properties: {
      symbol: { type: "string" },
      qty:    { type: "integer" },
    },
    required: ["symbol", "qty"],
  },
  intentName: "trade.equity",
  client: client,
  resolveContext: (args) => ({
    chain: agentChain,
    executorPkHex: agentPk,
    intentParams: { symbol: args.symbol },
  }),
  execute: async (args, auth) => {
    await broker.buy(args.symbol, args.qty);
    return { ok: true, chain_depth: auth.chainDepth };
  },
});
```

Pass `tradeTool.definition` to the Assistants API `tools` array and call
`tradeTool.handler(toolCall.function.arguments)` in your dispatch loop.

## Full examples

- Python: [`examples/integrations/openai_assistants_example.py`](../../examples/integrations/openai_assistants_example.py)
- TypeScript: [`examples/integrations/openai_agents_example.ts`](../../examples/integrations/openai_agents_example.ts)
