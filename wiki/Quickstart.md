# Quickstart Guide

**From zero to a guarded AI agent tool in 5 minutes.**

Pick your path based on what you need right now.

---

## Path A — Non-developer (CLI only, no code)

You want to issue a passport for your AI agent and see how it works.

### Install

```bash
cargo install a1-cli
```

### Issue a passport

```bash
a1 passport issue \
  --namespace my-trading-agent \
  --allow "trade.equity,portfolio.read" \
  --ttl 30d
```

This writes:
- `my-trading-agent-passport.json` — the agent's identity (safe to share)
- `my-trading-agent-key.hex` — the signing key (**keep this secret, put it in your vault**)

### Inspect a passport

```bash
a1 passport inspect my-trading-agent-passport.json
```

Output:
```
Passport: my-trading-agent-passport.json
  Namespace        : my-trading-agent
  Capability mask  : a3b2c1...
  Scope root       : 7f9c4e...
  Holder public key: 4a1b2c...
  Cert issued_at   : 1746547200
  Cert expires_at  : 1749139200
```

### Revoke a compromised agent

```bash
a1 revoke <fingerprint-hex>
```

---

## Path B — Python developer (one decorator)

You are building an AI agent tool in Python and want to add authorization in one line.

### Install

```bash
pip install a1identity
```

### Start the gateway

```bash
git clone https://github.com/dyologician/A1
cd A1
./setup.sh
```

### Issue a passport

```bash
a1 passport issue \
  --namespace my-bot \
  --allow "trade.equity,portfolio.read" \
  --ttl 30d
```

### Add the guard to your tool

```python
from a1identity.passport import PassportClient, a1_guard

client = PassportClient("http://localhost:8080")

@a1_guard(client=client, capability="trade.equity")
async def execute_trade(symbol: str, qty: int, signed_chain: dict, executor_pk_hex: str):
    # This only runs after the gateway confirms authorization.
    return await broker.place_order(symbol=symbol, qty=qty)
```

The decorator reads `signed_chain` and `executor_pk_hex` from the function's kwargs. Everything else happens automatically.

### What if authorization fails?

```python
from a1identity.passport import PassportError

try:
    result = await execute_trade(
        symbol="AAPL",
        qty=10,
        signed_chain=chain,
        executor_pk_hex=agent_pk,
    )
except PassportError as e:
    print(e.error_code)    # "PASSPORT_NARROWING_VIOLATION"
    print(e.http_status)   # 403
```

---

## Path C — TypeScript developer (one function)

```bash
npm install a1-ai
```

```typescript
import { withA1Passport, PassportClient } from "a1-ai/passport";

const client = new PassportClient("http://localhost:8080");

const guardedTrade = withA1Passport(executeTrade, {
  client,
  capability: "trade.equity",
});

// Callers pass signed_chain and executor_pk_hex alongside their normal args:
const result = await guardedTrade({
  symbol: "AAPL",
  qty: 10,
  signed_chain: chain,
  executor_pk_hex: agentPkHex,
});
```

---

## Path D — Go developer (one function)

```bash
go get github.com/dyologician/a1/sdk/go
```

```go
import "github.com/dyologician/a1/sdk/go/a1"

client, _ := a1.NewClient("http://localhost:8080")

guarded := a1.WithPassport(client, executeTrade, a1.PassportOptions{
    Capability: "trade.equity",
})

type TradeArgs struct {
    Symbol        string
    Qty           int
    SignedChain   any    // the delegation chain
    ExecutorPKHex string // agent's public key hex
}

receipt, err := guarded(ctx, TradeArgs{
    Symbol:        "AAPL",
    Qty:           10,
    SignedChain:   chain,
    ExecutorPKHex: agentPKHex,
})
```

---

## Path E — Rust developer (embedded, no gateway)

```toml
[dependencies]
a1-ai = { version = "2.8", features = ["full"] }
```

```rust
use a1::{DyoloIdentity, DyoloPassport, Intent, SystemClock};

fn main() -> Result<(), a1::A1Error> {
    let root  = DyoloIdentity::generate();
    let agent = DyoloIdentity::generate();
    let clock = SystemClock;

    // Issue a root passport (store this JSON in your vault)
    let passport = DyoloPassport::issue(
        "acme-trading-bot",
        &["trade.equity", "portfolio.read"],
        30 * 24 * 3600,
        &root,
        &clock,
    )?;
    passport.save("passport.json")?;

    // Issue a task-scoped sub-cert
    let sub_cert = passport.issue_sub(
        agent.verifying_key(),
        &["trade.equity"],
        3600,
        &root,
        &clock,
    )?;

    // Build the chain and guard
    let mut chain = passport.new_chain()?;
    chain.push(sub_cert);

    let intent  = Intent::new("trade.equity")?;
    let receipt = passport.guard_local(&chain, &agent.verifying_key(), &intent)?;

    assert!(receipt.verify_commitment());
    println!("{}", receipt);
    // ProvableReceipt { namespace=acme-trading-bot, depth=1, fingerprint=a3b2... }

    Ok(())
}
```

---

## What to do next

- Read the [Passport Guide](Passport-Guide) for multi-hop delegation and advanced patterns
- Set up a [persistent gateway](Enterprise-Deployment) for production
- Add [KMS signing](KMS-Integration) so your root key never touches application memory
- Connect to your [SIEM](SIEM-Integration) for audit log forwarding
- Review the [Security Model](Security-Model) before a compliance review
- See the [full capability list](../CAPABILITIES.md) for every available feature