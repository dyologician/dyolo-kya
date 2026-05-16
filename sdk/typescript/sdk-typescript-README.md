# A1 TypeScript SDK

[![npm](https://img.shields.io/npm/v/a1.svg)](https://www.npmjs.com/package/a1)
[![Node](https://img.shields.io/node/v/a1.svg)](https://www.npmjs.com/package/a1)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/dyologician/a1/blob/main/LICENSE-MIT)

TypeScript/Node.js SDK for [A1](https://github.com/dyologician/a1) — cryptographic chain-of-custody for recursive AI agent delegation.

A1 gives every AI agent a verifiable passport and produces an independently verifiable receipt for every authorized action. It closes the **Recursive Delegation Gap**: the inability to prove, in a multi-agent delegation chain, which human authorized the action at the end.

No Rust toolchain required — this SDK communicates with the self-hosted A1 gateway over HTTP/JSON.

---

## Requirements

- Node.js 18+
- An A1 gateway running locally or remotely (see [gateway setup](#running-the-gateway))

---

## Installation

```bash
npm install a1
```

---

## Quick Start

### 1. Start the gateway

```bash
git clone https://github.com/dyologician/a1.git
cd a1
docker compose up -d
```

The gateway listens on `http://localhost:8080`. Verify it:

```bash
curl http://localhost:8080/healthz
```

### 2. Issue a passport

```bash
cargo install a1-cli
a1 passport issue \
  --namespace my-agent \
  --allow "trade.equity,portfolio.read" \
  --ttl 30d \
  --out passport.json
```

### 3. Create a client and authorize an action

```typescript
import { A1Client } from "a1";

const client = new A1Client("http://localhost:8080");

const result = await client.authorize({
  chain: signedChain,
  intentName: "trade.equity",
  intentParams: { symbol: "AAPL", qty: 100 },
  executorPkHex: agentPkHex,
});

console.log(result.authorized);        // true
console.log(result.chainDepth);        // 2
console.log(result.chainFingerprint);  // hex string
```

### 4. Issue a delegation cert

```typescript
const cert = await client.issueCert({
  delegatePkHex: subAgentPkHex,
  intents: [{ name: "trade.equity", params: { symbol: "AAPL" } }],
  ttlSeconds: 3600,
  extensions: { "dyolo.cost_center": "ai-ops" },
});
```

### 5. Guard a function with `withA1Passport`

```typescript
import { withA1Passport, PassportClient } from "a1/passport";

const client = new PassportClient({
  gatewayUrl: "http://localhost:8080",
  passportPath: "./passport.json",
});

async function executeTrade(
  symbol: string,
  qty: number,
  signedChain: SignedChain,
  executorPkHex: string,
): Promise<{ status: string }> {
  return broker.placeOrder(symbol, qty);
}

const guardedTrade = withA1Passport(executeTrade, {
  client,
  capability: "trade.equity",
});

const result = await guardedTrade("AAPL", 100, chain, agentPk);
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

Import from `"a1/integrations"`:

### LangChain.js

```typescript
import { buildLangChainA1Tool } from "a1/integrations";
import { A1Client } from "a1";

const client = new A1Client("http://localhost:8080");

const tradeTool = buildLangChainA1Tool({
  name: "execute_trade",
  description: "Execute an equity trade. Input: JSON with symbol and qty.",
  intentName: "trade.equity",
  client,
  resolveContext: (rawInput: string) => {
    const { symbol } = JSON.parse(rawInput);
    return { chain: agentChain, executorPkHex: agentPk, intentParams: { symbol } };
  },
  fn: async ({ symbol, qty }: { symbol: string; qty: number }) => {
    return broker.placeOrder(symbol, qty);
  },
});
```

### LangGraph.js

```typescript
import { withDyoloLangGraphNode } from "a1/integrations";
import { A1Client } from "a1";

const client = new A1Client("http://localhost:8080");

const guardedNode = withDyoloLangGraphNode(
  async (state: AgentState) => {
    await broker.placeOrder(state.symbol, state.qty);
    return state;
  },
  {
    intentName: "trade.equity",
    client,
    resolveContext: (state: AgentState) => ({
      chain: state.chain,
      executorPkHex: state.agentPk,
    }),
  },
);
```

### Semantic Kernel (JS)

```typescript
import { withDyoloSkFunction } from "a1/integrations";
import { A1Client } from "a1";

const client = new A1Client("http://localhost:8080");

const guardedFunction = withDyoloSkFunction(executeTrade, {
  intentName: "trade.equity",
  client,
  resolveContext: (args) => ({ chain: agentChain, executorPkHex: agentPk }),
});
```

### OpenAI Agents SDK

```typescript
import { buildOpenAiAgentA1Tool } from "a1/integrations";
import { A1Client } from "a1";

const client = new A1Client("http://localhost:8080");

const tool = buildOpenAiAgentA1Tool({
  name: "execute_trade",
  description: "Execute an equity trade.",
  intentName: "trade.equity",
  client,
  resolveContext: (input) => ({ chain: agentChain, executorPkHex: agentPk }),
  fn: executeTrade,
});
```

---

## Passport Client

For full passport lifecycle management:

```typescript
import { PassportClient } from "a1/passport";

const passport = new PassportClient({
  gatewayUrl: "http://localhost:8080",
  passportPath: "./passport.json",
  adminSecret: process.env.A1_ADMIN_SECRET,
});

// Authorize with automatic chain building
const receipt = await passport.authorize({
  intentName: "trade.equity",
  executorPkHex: agentPkHex,
});

// Issue a sub-delegation cert
const subCert = await passport.issueSub({
  delegatePkHex: subAgentPkHex,
  capabilities: ["trade.equity"],
  ttlSeconds: 3600,
});

// Inspect the passport
const info = await passport.inspect();
console.log(info.namespace, info.capabilities, info.expiresAt);
```

---

## JWT Bridge (OIDC / SSO)

Exchange an existing OIDC or SAML JWT token for a scoped A1 `DelegationCert` — no manual key ceremony required. Useful for enterprises already running SSO.

```typescript
import { A1Client } from "a1";

const client = new A1Client("http://localhost:8080");

const cert = await client.exchangeJwt({
  token: jwtBearerToken,            // your OIDC access token
  capabilities: ["files.read"],
  ttlSeconds: 3600,
  delegatePkHex: agentPkHex,
});

// Use the returned cert to build a delegation chain for /v1/authorize
```

Configure the gateway with `A1_JWT_JWKS_URL` pointing at your identity provider's JWKS endpoint and `A1_JWT_ALLOWED_CAPS` listing permitted capabilities.

---

## Delegation Negotiation

An agent can request a scoped cert from the gateway without a pre-shared chain:

```typescript
import { A1Client } from "a1";

const client = new A1Client("http://localhost:8080");

const offer = await client.negotiate({
  intentName: "files.read",
  agentPkHex: myPublicKeyHex,
  requestedTtlSeconds: 3600,
});

// offer.cert is a DelegationCert signed by the gateway
```

---

## Swarm Management

```typescript
import { A1Client } from "a1";

const client = new A1Client("http://localhost:8080", {
  adminSecret: process.env.A1_ADMIN_SECRET,
});

// Create a swarm
const swarm = await client.createSwarm({
  swarmName: "trading-fleet",
  capabilities: ["trade.equity"],
  ttlDays: 30,
  signingKeyHex: rootKeyHex,
});

// Add a member
await client.addSwarmMember({
  swarmId: swarm.swarmId,
  agentPkHex: workerPkHex,
  role: "worker",
  capabilities: ["trade.equity"],
  ttlSeconds: 3600,
  signingKeyHex: rootKeyHex,
});

// Remove a member
await client.removeSwarmMember({
  swarmId: swarm.swarmId,
  agentDid: workerDid,
});

// List active members
const { members } = await client.listSwarmMembers(swarm.swarmId);
```

---

## On-chain Anchoring (ZK)

Anchor a zero-knowledge chain commitment on-chain for immutable audit trails:

```typescript
import { A1Client } from "a1";

const client = new A1Client("http://localhost:8080");

const anchor = await client.anchor({
  commitment: zkChainCommitment,
  passportDid: "did:a1:...",
  network: "ethereum",             // ethereum | polygon | base | arbitrum | solana
});

console.log(anchor.anchorHashHex);
// For EVM chains:
console.log(anchor.evmCalldata);   // submit via eth_sendRawTransaction
// For Solana:
console.log(anchor.solanaInstructionData);
```

---

## Multi-Tenant

Send `X-A1-Tenant-ID` on every request to scope all revocation and nonce operations to that tenant:

```typescript
import { A1Client } from "a1";

const client = new A1Client("http://localhost:8080", {
  defaultHeaders: { "X-A1-Tenant-ID": "acme" },
});

// All authorize, revoke, and nonce calls are now scoped to tenant "acme"
const result = await client.authorize({ ... });
```

Enable on the gateway with `A1_MULTI_TENANT=true`.

---

## Batch Authorization

```typescript
const results = await client.authorizeBatch({
  chain: signedChain,
  intents: [
    { name: "trade.equity", params: { symbol: "AAPL" } },
    { name: "portfolio.read" },
  ],
  executorPkHex: agentPkHex,
});

// All-or-nothing: if any intent fails, no nonces are consumed
results.forEach((r) => console.log(r.intentName, r.authorized));
```

---

## API Reference

### `A1Client`

```typescript
import { A1Client, AuthorizeRequest, AuthorizeResult } from "a1";

const client = new A1Client(
  gatewayUrl: string,
  options?: {
    timeout?: number;                          // ms, default 10000
    adminSecret?: string;                      // required for admin endpoints
    defaultHeaders?: Record<string, string>;   // e.g. tenant header
  }
);

client.authorize(req: AuthorizeRequest): Promise<AuthorizeResult>
client.authorizeBatch(req: AuthorizeBatchRequest): Promise<AuthorizeBatchResult>
client.issueCert(req: IssueCertRequest): Promise<IssueCertResult>
client.revokeCert(fingerprint: string): Promise<void>
client.revokeCertsBatch(fingerprints: string[]): Promise<void>
client.exchangeJwt(req: JwtExchangeRequest): Promise<IssueCertResult>
client.negotiate(req: NegotiateRequest): Promise<DelegationOffer>
client.anchor(req: AnchorRequest): Promise<AnchorResult>
client.createSwarm(req: CreateSwarmRequest): Promise<SwarmResult>
client.addSwarmMember(req: AddSwarmMemberRequest): Promise<void>
client.removeSwarmMember(req: RemoveSwarmMemberRequest): Promise<void>
client.listSwarmMembers(swarmId: string): Promise<SwarmMembersResult>
client.health(): Promise<HealthResult>
client.wellKnown(): Promise<A1Configuration>
```

### Key types

```typescript
interface AuthorizeRequest {
  chain: SignedChain;
  intentName: string;
  intentParams?: Record<string, unknown>;
  executorPkHex: string;
  returnToken?: boolean;
  requestId?: string;
}

interface AuthorizeResult {
  authorized: boolean;
  chainDepth: number;
  chainFingerprint: string;
  verifiedAtUnix: number;
  receipt: {
    chainDepth: number;
    fingerprintHex: string;
    verifiedAtUnix: number;
    passportNamespace: string;
    capabilityMaskHex: string;
    narrowingCommitmentHex: string;
  };
  token?: VerifiedToken;
}

interface SignedChain {
  certs: DelegationCert[];
  principalPkHex: string;
}
```

---

## Error Handling

```typescript
import { A1AuthorizationError, A1GatewayError } from "a1";

try {
  const result = await client.authorize(req);
} catch (e) {
  if (e instanceof A1AuthorizationError) {
    // Authorization denied: expired cert, scope violation, replay, etc.
    console.error("Denied:", e.reason, "code:", e.errorCode);
  } else if (e instanceof A1GatewayError) {
    // Network error or unexpected gateway response
    console.error("Gateway error:", e.message, "status:", e.status);
  }
}
```

---

## Module Exports

```typescript
// Main entry point
import { A1Client, A1AuthorizationError, A1GatewayError } from "a1";

// Passport management + guards
import { PassportClient, withA1Passport, PassportGuard } from "a1/passport";

// Framework integrations
import {
  buildLangChainA1Tool,
  withDyoloLangGraphNode,
  withDyoloSkFunction,
  buildOpenAiAgentA1Tool,
} from "a1/integrations";

// Middleware helpers (Express, Hono, Fastify, etc.)
import { A1Middleware, exchangeJwt, verifyWebhookSignature } from "a1/middleware";
```

The package ships both ESM and CJS builds and is fully compatible with TypeScript strict mode.

---

## Middleware

`a1/middleware` provides drop-in middleware for Node.js HTTP frameworks and webhook verification.

### `A1Middleware`

```typescript
import { A1Middleware } from "a1/middleware";
import { A1Client } from "a1";

const middleware = new A1Middleware(new A1Client("http://localhost:8080"), {
  capability: "trade.equity",
});

// Express
app.use("/trade", middleware.express(), (req, res) => {
  console.log(res.locals.a1Receipt);  // ProvableReceipt
  res.json({ ok: true });
});

// Hono
app.use("/trade", middleware.hono());
```

### `exchangeJwt` — JWT bootstrap (OIDC / SSO)

Exchange an existing OIDC/SAML JWT for a scoped `DelegationCert` without a manual key ceremony:

```typescript
import { exchangeJwt } from "a1/middleware";

const result = await exchangeJwt({
  gatewayUrl: "http://localhost:8080",
  token: jwtBearerToken,
  capabilities: ["files.read"],
  ttlSeconds: 3600,
  delegatePkHex: agentPublicKeyHex,
  adminSecret: process.env.A1_ADMIN_SECRET,
});

console.log(result.cert);  // DelegationCert ready to use
```

### `verifyWebhookSignature` — Webhook security

Verify A1 gateway event payloads before processing them:

```typescript
import { verifyWebhookSignature } from "a1/middleware";

app.post("/a1-events", (req, res) => {
  const event = verifyWebhookSignature(req.body, process.env.A1_WEBHOOK_SECRET);
  // `event` is typed as WebhookEvent; throws if signature invalid
  console.log(event.type, event.payload);
  res.sendStatus(200);
});
```

---

## Running the Gateway

```bash
git clone https://github.com/dyologician/a1.git
cd a1
cp .env.example .env        # fill in at minimum A1_PG_PASSWORD
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

## Development

```bash
cd sdk/typescript
npm install

# Type check
npx tsc --noEmit

# Run tests
npx jest

# Build (ESM + CJS)
npm run build
```

---

## License

MIT OR Apache-2.0. See [LICENSE-MIT](https://github.com/dyologician/a1/blob/main/LICENSE-MIT) and [LICENSE-APACHE](https://github.com/dyologician/a1/blob/main/LICENSE-APACHE).

---

*Part of the [A1](https://github.com/dyologician/a1) ecosystem. Built and maintained by dyolo ([@dyologician](https://github.com/dyologician)).*