# TypeScript SDK

The A1 TypeScript SDK (`a1-ai` on npm) provides a fully-typed client for the A1 gateway. Works with Node.js 18+, ESM and CJS, and any TypeScript-based AI agent framework.

---

## Installation

```bash
npm install a1-ai
```

---

## Start the gateway

```bash
git clone https://github.com/dyologician/A1
cd A1
./setup.sh
```

---

## Core client

```typescript
import { A1Client } from "a1-ai";

const client = new A1Client("http://localhost:8080");

const result = await client.authorize({
  chain: signedChain,
  intentName: "trade.equity",
  intentParams: { symbol: "AAPL", qty: "100" },
  executorPkHex: agentPkHex,
});

console.log(result.authorized);        // true
console.log(result.chainDepth);        // 2
console.log(result.chainFingerprint);  // "a3b2c1..."
console.log(result.namespace);         // "trading-bot"
```

### Batch authorization

```typescript
const results = await client.authorizeBatch({
  chain: signedChain,
  executorPkHex: agentPkHex,
  intents: [
    { intentName: "trade.equity", intentParams: { symbol: "AAPL" } },
    { intentName: "portfolio.read" },
  ],
});
```

---

## Passport guard (`withA1Passport`)

One higher-order function. Wraps any async function.

```typescript
import { withA1Passport, PassportClient } from "a1-ai/passport";

const client = new PassportClient("http://localhost:8080");

async function executeTrade(args: {
  symbol: string;
  qty: number;
  signed_chain: SignedChain;
  executor_pk_hex: string;
}) {
  return broker.placeOrder(args.symbol, args.qty);
}

const guardedTrade = withA1Passport(executeTrade, {
  client,
  capability: "trade.equity",
});

// Authorization is checked before executeTrade runs:
const result = await guardedTrade({
  symbol: "AAPL",
  qty: 10,
  signed_chain: chain,
  executor_pk_hex: agentPkHex,
});
```

### Class decorator variant

```typescript
import { PassportGuard } from "a1-ai/passport";

class TradingService {
  @PassportGuard({ client, capability: "trade.equity" })
  async executeTrade(args: TradeArgs) {
    return broker.placeOrder(args.symbol, args.qty);
  }
}
```

---

## Middleware

For Express, Hono, Fastify, and any Node.js HTTP framework.

```typescript
import { A1Middleware, exchangeJwt, verifyWebhookSignature } from "a1-ai/middleware";

const a1 = new A1Middleware({
  gatewayUrl: "http://localhost:8080",
  adminSecret: process.env.A1_ADMIN_SECRET,
});

// Express middleware
app.post("/trade", a1.guard("trade.equity"), async (req, res) => {
  const auth = req.a1;                 // AuthorizeResult attached by middleware
  console.log(auth.chainDepth);        // 2
  res.json({ status: "ok" });
});

// JWT exchange — swap a JWKS-verified JWT for a scoped DelegationCert
const cert = await exchangeJwt({
  gatewayUrl: "http://localhost:8080",
  jwtToken: bearerToken,
  capability: "trade.equity",
  adminSecret: process.env.A1_ADMIN_SECRET,
});

// Webhook signature verification
const event = verifyWebhookSignature(req.body, req.headers["x-a1-signature"], webhookSecret);
console.log(event.type);   // "AUTHORIZED" | "DENIED" | "REVOKED"
```

---

## LangChain.js integration

```typescript
import { buildLangChainA1Tool } from "a1-ai/integrations";

const tradeTool = buildLangChainA1Tool({
  name: "execute_trade",
  description: "Execute an equity trade after authorization",
  intentName: "trade.equity",
  client: a1Client,
  resolveContext: (rawInput) => ({
    chain: agentChain,
    executorPkHex: agentPk,
    intentParams: { symbol: JSON.parse(rawInput).symbol },
  }),
  run: async (input, auth) => {
    const { symbol, qty } = JSON.parse(input);
    await broker.trade(symbol, qty);
    return `Executed. Chain depth: ${auth.chainDepth}`;
  },
});
```

### Batch LangChain tool (multiple intents)

```typescript
import { buildBatchLangChainA1Tool } from "a1-ai/integrations";

const multiTool = buildBatchLangChainA1Tool({
  name: "portfolio_ops",
  description: "Execute a trade and read portfolio in one authorized call",
  intents: ["trade.equity", "portfolio.read"],
  client: a1Client,
  resolveContext: (input) => ({ chain: agentChain, executorPkHex: agentPk }),
  run: async (input, authResults) => {
    const tradeAuth = authResults[0];
    const readAuth  = authResults[1];
    // Both authorized — execute:
    await broker.trade(...);
    const portfolio = await broker.readPortfolio();
    return JSON.stringify({ trade: "filled", portfolio });
  },
});
```

---

## LangGraph integration

```typescript
import { withDyoloLangGraphNode } from "a1-ai/integrations";

const tradeNode = withDyoloLangGraphNode({
  intentName: "trade.equity",
  client: a1Client,
  resolveContext: (state) => ({
    chain: state.agentChain,
    executorPkHex: state.agentPk,
  }),
  run: async (state, auth) => {
    await broker.trade(state.symbol, state.qty);
    return { ...state, result: "filled", chainDepth: auth.chainDepth };
  },
});
```

---

## Error handling

```typescript
import { A1Error } from "a1-ai";

try {
  const result = await guardedTrade(args);
} catch (err) {
  if (err instanceof A1Error) {
    console.error(err.errorCode);   // "PASSPORT_NARROWING_VIOLATION"
    console.error(err.httpStatus);  // 403
    console.error(err.message);     // human-readable reason
  }
}
```

---

## Full type reference

| Import path | Export | Description |
|---|---|---|
| `"a1-ai"` | `A1Client` | HTTP client for the gateway |
| `"a1-ai"` | `A1Error` | Base error class |
| `"a1-ai"` | `SignedChain` | Wire type for delegation chains |
| `"a1-ai"` | `AuthorizeResult` | Result of a single authorization |
| `"a1-ai"` | `BatchAuthorizeResult` | Result of a batch authorization |
| `"a1-ai/passport"` | `PassportClient` | Passport lifecycle management |
| `"a1-ai/passport"` | `withA1Passport` | Higher-order guard function |
| `"a1-ai/passport"` | `PassportGuard` | Class decorator variant |
| `"a1-ai/middleware"` | `A1Middleware` | Express/Hono/Fastify middleware |
| `"a1-ai/middleware"` | `exchangeJwt` | JWT → DelegationCert exchange |
| `"a1-ai/middleware"` | `verifyWebhookSignature` | Webhook payload verification |
| `"a1-ai/integrations"` | `buildLangChainA1Tool` | LangChain.js tool guard |
| `"a1-ai/integrations"` | `buildBatchLangChainA1Tool` | LangChain.js batch guard |
| `"a1-ai/integrations"` | `withDyoloLangGraphNode` | LangGraph node guard |

---

*Source: `sdk/typescript/src/` · [Back to wiki home](Home)*
