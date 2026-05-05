# OpenAI Assistants / Agents SDK Integration Guide

Gate every OpenAI function tool call with a cryptographic authorization check.

## Prerequisites

```bash
# Python
pip install dyolo-kya openai

# TypeScript
npm install dyolo-kya openai

docker run -p 8080:8080 ghcr.io/dyologician/dyolo-kya-gateway:2
```

## Python — `kya_function_guard` decorator

```python
import os, json
from dyolo_kya import KyaClient
from dyolo_kya.openai_tool import kya_function_guard

kya = KyaClient("http://localhost:8080")

AGENT_PK    = os.environ["AGENT_PK_HEX"]
AGENT_CHAIN = json.loads(os.environ["AGENT_SIGNED_CHAIN"])

@kya_function_guard(
    intent_name="trade.equity",
    client=kya,
    chain=AGENT_CHAIN,
    executor_pk_hex=AGENT_PK,
)
def execute_trade(symbol: str, qty: int) -> dict:
    import your_broker
    your_broker.buy(symbol, qty)
    return {"status": "filled", "symbol": symbol, "qty": qty}
```

Register the tool schema with the Assistants API and dispatch tool calls to the
decorated function.  The decorator raises `KyaError` before your function body
runs if authorization fails.

## TypeScript — `buildOpenAIKyaFunction`

```ts
import { KyaClient, SignedChain } from "dyolo-kya";
import { buildOpenAIKyaFunction } from "dyolo-kya/integrations";

const kya         = new KyaClient("http://localhost:8080");
const agentChain  = JSON.parse(process.env.AGENT_SIGNED_CHAIN!) as SignedChain;
const agentPk     = process.env.AGENT_PK_HEX!;

const tradeTool = buildOpenAIKyaFunction({
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
  client: kya,
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
