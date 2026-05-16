# Passport Guide

A `DyoloPassport` is the primary entry point for A1 v2.8. This guide walks through every stage of the passport lifecycle: issuance, sub-delegation, chain construction, guarding, and audit archival.

---

## Concepts

### What a passport is

A passport is a self-signed Ed25519 certificate that declares:
- **Who** holds it (the `delegator_pk` — the human principal's key)
- **What** it can do (a set of named capabilities encoded in a `NarrowingMatrix`)
- **Until when** it is valid (the `expires_at` field)

Every sub-delegation cert derived from a passport must carry a `SubScopeProof` proving it is a strict Merkle subset of the passport's scope. Any attempt to escalate beyond the passport's capabilities is rejected at issuance time.

### The Recursive Delegation Gap

Without a passport system, the following chain has no provable scope bounds:

```
Human → Orchestrator Agent → Executor Agent → (action)
```

With a1 passports, the same chain becomes:

```
Human (DyoloPassport: [trade.equity, portfolio.read])
  └─ Orchestrator cert (SubScopeProof: [trade.equity, portfolio.read])
       └─ Executor cert (SubScopeProof: [trade.equity])
            └─ Intent("trade.equity") → ProvableReceipt
```

Every arrow is an Ed25519 signature. Every scope reduction is enforced by `NarrowingMatrix`. The final receipt is independently verifiable without retaining any secrets.

---

## Lifecycle

### Step 1 — Issue a root passport

A root passport is issued once and stored securely. It anchors all downstream delegation.

**CLI**
```bash
a1 passport issue \
  --namespace acme-trading-bot \
  --allow "trade.equity,portfolio.read" \
  --ttl 30d \
  --out acme-trading-bot-passport.json
```

This writes two files:
- `acme-trading-bot-passport.json` — the passport (share with agents)
- `acme-trading-bot-key.hex` — the 32-byte signing key seed (**store in your vault**)

**Rust**
```rust
use a1::{DyoloIdentity, DyoloPassport, SystemClock};

let root  = DyoloIdentity::generate();
let clock = SystemClock;

let passport = DyoloPassport::issue(
    "acme-trading-bot",
    &["trade.equity", "portfolio.read"],
    30 * 24 * 3600,  // 30 days
    &root,
    &clock,
)?;
passport.save("passport.json")?;
```

**Python (via gateway)**
```python
import subprocess, json
result = subprocess.run(
    ["a1", "passport", "issue",
     "--namespace", "acme-trading-bot",
     "--allow", "trade.equity,portfolio.read",
     "--ttl", "30d"],
    capture_output=True, text=True
)
```

---

### Step 2 — Issue a sub-delegation cert (per task)

For every task, issue a time-limited sub-cert scoped to only the capabilities that task needs. The sub-cert's capabilities must be a strict subset of the passport's.

**CLI**
```bash
# First, get the agent's public key
AGENT_PK=$(a1 keygen | grep "verifying_key_hex" | awk '{print $NF}')

a1 passport sub \
  --passport passport.json \
  --key acme-trading-bot-key.hex \
  --delegate $AGENT_PK \
  --allow "trade.equity" \
  --ttl 1h \
  --out sub-cert.json
```

**Rust**
```rust
let agent = DyoloIdentity::generate();

let sub_cert = passport.issue_sub(
    agent.verifying_key(),
    &["trade.equity"],   // must be subset of passport's capabilities
    3600,                // 1 hour
    &root,               // passport holder's signing key
    &clock,
)?;
```

**Attempted escalation (rejected)**
```rust
// This returns Err(A1Error::PassportNarrowingViolation)
let invalid = passport.issue_sub(
    agent.verifying_key(),
    &["admin.delete"],   // not in passport's capability set
    3600,
    &root,
    &clock,
);
assert!(invalid.is_err());
```

---

### Step 3 — Build the delegation chain

```rust
let mut chain = passport.new_chain()?;
chain.push(sub_cert);
// For multi-hop: push additional sub-certs in order from outermost to innermost
```

---

### Step 4 — Guard the action

**In-process (no gateway)**
```rust
let intent  = Intent::new("trade.equity")?;
let receipt = passport.guard_local(&chain, &agent.verifying_key(), &intent)?;

// Archive this receipt in your audit log
assert!(receipt.verify_commitment());
println!("{}", receipt);
// ProvableReceipt { namespace=acme-trading-bot, depth=1, fingerprint=a3b2... }
```

**Via gateway (Python)**
```python
from a1.passport import PassportClient, a1_guard

client = PassportClient("http://localhost:8080")

@a1_guard(client=client, capability="trade.equity")
async def execute_trade(symbol: str, qty: int, signed_chain: dict, executor_pk_hex: str):
    return await broker.place_order(symbol, qty)
```

**Via gateway (TypeScript)**
```typescript
import { withA1Passport, PassportClient } from "a1/passport";

const client = new PassportClient("http://localhost:8080");
const guarded = withA1Passport(executeTrade, { client, capability: "trade.equity" });
```

---

### Step 5 — Archive the receipt

The `ProvableReceipt` contains everything needed for audit replay. No secrets required.

```rust
// Fields available for archival
receipt.passport_namespace     // "acme-trading-bot"
receipt.capability_mask_hex    // hex of the enforced NarrowingMatrix
receipt.narrowing_commitment   // Blake3 commitment over the mask
receipt.inner.chain_depth      // 1
receipt.inner.chain_fingerprint // unique ID for this delegation chain
receipt.inner.verified_at_unix // Unix timestamp of authorization

// Independent verification (auditor does this, no secrets needed)
assert!(receipt.verify_commitment());
```

---

## Multi-hop delegation

```rust
let orchestrator = DyoloIdentity::generate();
let executor     = DyoloIdentity::generate();

// Passport: [trade.equity, portfolio.read, audit.read]
let passport = DyoloPassport::issue(
    "trading-swarm",
    &["trade.equity", "portfolio.read", "audit.read"],
    86400,
    &root,
    &clock,
)?;

// Orchestrator cert: [trade.equity, portfolio.read] — subset
let orch_cert = passport.issue_sub(
    orchestrator.verifying_key(),
    &["trade.equity", "portfolio.read"],
    7200,
    &root,
    &clock,
)?;

// Executor cert: [trade.equity] — strict subset of orchestrator's scope
// The executor signs this cert because the orchestrator is delegating further
let exec_cert = passport.issue_sub(
    executor.verifying_key(),
    &["trade.equity"],
    3600,
    &orchestrator,  // signed by orchestrator, not root
    &clock,
)?;

let mut chain = passport.new_chain()?;
chain.push(orch_cert).push(exec_cert);

let intent  = Intent::new("trade.equity")?;
let receipt = passport.guard_local(&chain, &executor.verifying_key(), &intent)?;
// receipt.inner.chain_depth == 2
```

---

## Save and load

Passports are serialized as JSON and can be stored in any secret management system.

```rust
// Save
passport.save("passport.json")?;

// Load
let passport = DyoloPassport::load("passport.json")?;
```

The on-disk format is stable and versioned. It is identified by the `a1_passport` field set to `1`.

```json
{
  "a1_passport": 1,
  "namespace": "acme-trading-bot",
  "capability_mask_hex": "a3b2...",
  "capabilities": ["trade.equity", "portfolio.read"],
  "cert": { ... }
}
```

---

## KMS-backed issuance

For production, replace `DyoloIdentity` with a `VaultSigner`:

```python
from a1.vault import HashiCorpVaultSigner

signer = HashiCorpVaultSigner(
    vault_addr="https://vault.corp.example.com",
    key_name="a1-passport-root",
)
# Pass signer to the gateway's /issue endpoint or to the Rust core
```

At authorization time: **zero KMS calls**. The verifying key is embedded in the cert. All verification is local.

---

## Capability naming conventions

Capability names are arbitrary strings. By convention, use `domain.action` format:

```
trade.equity          trade.options         trade.fx
portfolio.read        portfolio.write
audit.read            audit.export
risk.compute          risk.approve
settlement.initiate   settlement.confirm
admin.read            admin.write           admin.delete
```

The `NarrowingMatrix` maps each name to a bit position via Blake3. Naming conventions are advisory — the enforcement is on the bits, not the strings.

---

## FAQ

**Q: Can I issue a passport with all capabilities?**  
Yes. Use `NarrowingMatrix::FULL` or list all 256 possible named capabilities. In practice, use the minimum set your agent needs.

**Q: How many capabilities can one passport hold?**  
Up to 256 unique capability slots. The NarrowingMatrix is 256 bits. Most deployments use fewer than 20.

**Q: Can a sub-cert have the same capabilities as the passport?**  
Yes. `enforce_narrowing` checks `child_mask ⊆ parent_mask`, which includes equality.

**Q: What happens when a passport expires?**  
Re-issue with the same key file. Old authorized receipts remain valid for audit replay — they carry their own timestamp.

**Q: Can I revoke a specific sub-cert without revoking the whole passport?**  
Yes. Revoke the sub-cert's fingerprint via `a1 revoke <fingerprint>`. The passport and other sub-certs remain valid.

**Q: Is the passport compatible with v2.0.0 chains?**  
Fully. `DyoloPassport` wraps the same `DelegationCert` type used in v2.0. Existing chains need no migration.
