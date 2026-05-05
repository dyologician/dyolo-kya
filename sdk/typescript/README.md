# dyolo-kya — TypeScript SDK

TypeScript/Node.js client for the [dyolo-kya](https://github.com/dyologician/dyolo-kya) cryptographic
agent delegation protocol. No Rust toolchain required — wraps the REST gateway.

## Install

```bash
npm install dyolo-kya
```

## Quick start

```ts
import { KyaClient } from "dyolo-kya";

const kya = new KyaClient("http://localhost:8080");

// Issue a cert
const cert = await kya.issueCert({
  delegatePkHex: "<agent-ed25519-pk-hex>",
  intents: [{ name: "trade.equity", params: { symbol: "AAPL" } }],
  ttlSeconds: 3600,
  extensions: { "dyolo.cost_center": "ai-ops" },
});

// Authorize an action
const result = await kya.authorize({
  chain: signedChain,
  intentName: "trade.equity",
  intentParams: { symbol: "AAPL" },
  executorPkHex: "<agent-pk-hex>",
});

console.log(result.authorized, result.chainDepth);
```

## LangChain.js

```ts
import { buildLangChainKyaTool } from "dyolo-kya/integrations";

const tradeTool = buildLangChainKyaTool({
  name: "execute_trade",
  description: "Execute an equity trade. Input: JSON with symbol and qty.",
  intentName: "trade.equity",
  client: kya,
  resolveContext: (rawInput) => {
    const { symbol } = JSON.parse(rawInput);
    return { chain: agentChain, executorPkHex: agentPk, intentParams: { symbol } };
  },
  run: async (rawInput, auth) => {
    const { symbol, qty } = JSON.parse(rawInput);
    await broker.executeTrade(symbol, qty);
    return `Trade placed. Authorized at depth ${auth.chainDepth}.`;
  },
});
```

## OpenAI Assistants / Agents SDK

```ts
import { buildOpenAIKyaFunction } from "dyolo-kya/integrations";

const tradeFn = buildOpenAIKyaFunction({
  name: "execute_trade",
  description: "Execute an equity trade on behalf of the authorized user",
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
  execute: async (args, auth) => ({
    ok: true,
    chain_depth: auth.chainDepth,
    fingerprint: auth.chainFingerprint,
  }),
});

// In your tool_calls dispatch loop:
const output = await tradeFn.handler(toolCall.function.arguments);
```

## AutoGen

```ts
import { withKyaGuard } from "dyolo-kya/integrations";

const guardedTrade = withKyaGuard({
  intentName: "trade.equity",
  client: kya,
  resolveContext: (args) => ({ chain: agentChain, executorPkHex: agentPk }),
  fn: async (args, auth) => broker.executeTrade(args.symbol, args.qty),
});
```

## Run the gateway

```bash
docker run -p 8080:8080 \
  -e DYOLO_SIGNING_KEY_HEX=<64-char-hex> \
  -e DYOLO_MAC_KEY_HEX=<64-char-hex> \
  ghcr.io/dyologician/dyolo-kya-gateway:2
```

See the [root README](../../README.md) for the full protocol specification.
