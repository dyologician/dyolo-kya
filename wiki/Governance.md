# Governance

A1's governance module lets DAO and on-chain governance systems verify that a vote or proposal came from an authorized agent identity — cryptographically, not just by wallet address.

---

## Enable

```toml
[dependencies]
a1-ai = { version = "2.8", features = ["governance"] }
```

---

## The problem it solves

On-chain governance systems know which wallet voted. They don't know:
- Which AI agent, if any, cast the vote on the wallet's behalf
- Whether that agent was authorized by the wallet holder to vote
- What capabilities the agent held at the time of the vote

A1 binds every governance action to a passport identity, giving governance systems cryptographic proof of the authorization chain behind every vote.

---

## How it works

Every governance action (vote, proposal, approval) is signed by the executing agent's `DelegationCert`. The signature proves:

1. The agent holds a valid, unexpired cert
2. The cert was issued by the human principal (wallet holder)
3. The cert explicitly includes the `governance.vote` capability
4. The action is timestamped and cannot be replayed

---

## Gateway endpoints

### Get active policy

```bash
curl http://localhost:8080/v1/governance/policy
```

Returns the active delegation policy constraints (capability requirements, TTL limits, chain depth requirements).

### Verify a governance approval

```bash
curl -X POST http://localhost:8080/v1/governance/approval/verify \
  -H "Content-Type: application/json" \
  -d '{
    "approval_record": { /* governance approval JSON */ },
    "on_chain_hash": "0xabc123..."
  }'
```

Verifies that the approval record's hash matches what was recorded on-chain.

### Generate an audit report (admin)

```bash
curl -X POST http://localhost:8080/v1/governance/audit-report \
  -H "Authorization: Bearer $A1_ADMIN_SECRET" \
  -H "Content-Type: application/json" \
  -d '{
    "from": "2025-01-01T00:00:00Z",
    "to": "2025-06-01T00:00:00Z",
    "namespace": "dao-voting-agent"
  }'
```

Returns a structured audit report of all governance actions in the time window, with full chain fingerprints for each action.

---

## Governance capability

Issue a passport with the `governance.vote` capability to authorize an agent to vote on governance proposals:

```bash
a1 passport issue \
  --namespace dao-voting-agent \
  --allow "governance.vote,governance.propose" \
  --ttl 7d
```

---

## On-chain integration

The `anchor_hash` function produces a 32-byte Blake3 hash suitable for submission to any EVM-compatible blockchain:

```rust
use a1::zk::anchor_hash;

let hash = anchor_hash(&chain_commitment);
// Submit to Ethereum, Base, Arbitrum, etc.
// bytes32 a1Hash = 0xabc123...;
```

This creates a permanent, tamper-evident on-chain record that the authorization occurred, without revealing the full delegation chain.

---

## Compliance

Governance audit trails produced by A1 map to:

- **SOC 2 CC7.2** — Security events monitored: every vote is a structured `AuditEvent`
- **ISO 27001 A.5.33** — Protection of records: `AuditRecord` is append-only and Blake3-hashed
- **DAO legal requirements** — Cryptographic proof of who authorized each vote, suitable for legal proceedings

---

*Source: `src/governance.rs`, `a1-gateway/src/routes/governance.rs` · [Back to wiki home](Home)*
