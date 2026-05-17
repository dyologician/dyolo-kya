# Swarm Coordination

A1 swarms let groups of AI agents register themselves, discover peers, and coordinate actions — while keeping every individual action cryptographically authorized. Swarm membership does not grant any additional capabilities.

---

## Enable

```toml
[dependencies]
a1-ai = { version = "2.8", features = ["swarm"] }
```

---

## Concepts

| Term | Description |
|---|---|
| **Swarm** | A named group of A1-authorized agents sharing a capability set |
| **SwarmPassport** | The root identity for the swarm — issued once, anchors all member certs |
| **SwarmMember** | An individual agent registered in the swarm |
| **SwarmRole** | `orchestrator`, `worker`, `supervisor`, or `auditor` |

Swarm membership is recorded cryptographically — each member holds a `DelegationCert` derived from the swarm passport. Individual authorization still happens per-action via `DyoloChain::authorize`. Swarms add discovery and coordination, not authorization bypass.

---

## Python SDK

```python
from a1.swarm import SwarmClient, SwarmRole

client = SwarmClient("http://localhost:8080",
                     admin_secret=os.environ["A1_ADMIN_SECRET"])

# Create a swarm
swarm_id = client.create_swarm(
    name="acme-trading-swarm",
    capabilities=["trade.equity", "portfolio.read"],
    ttl_days=30,
    signing_key_hex=ROOT_SK_HEX,
)

# Add members with role-based capabilities
orchestrator_cert = client.add_member(
    swarm_id=swarm_id,
    agent_pk_hex=ORCHESTRATOR_PK,
    role=SwarmRole.ORCHESTRATOR,
    capabilities=["trade.equity", "portfolio.read"],
    ttl_seconds=86400,
    signing_key_hex=ROOT_SK_HEX,
)

worker_cert = client.add_member(
    swarm_id=swarm_id,
    agent_pk_hex=WORKER_PK,
    role=SwarmRole.WORKER,
    capabilities=["trade.equity"],   # subset of swarm capabilities
    ttl_seconds=3600,
    signing_key_hex=ROOT_SK_HEX,
)

# List active members
members = client.list_members(swarm_id)
for m in members:
    print(m.role, m.agent_pk_hex, m.last_seen)

# Remove a member
client.remove_member(swarm_id, agent_pk_hex=WORKER_PK)
```

---

## Gateway API

### Create a swarm

```bash
curl -X POST http://localhost:8080/v1/swarm/create \
  -H "Authorization: Bearer $A1_ADMIN_SECRET" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "trading-swarm",
    "capabilities": ["trade.equity", "portfolio.read"],
    "ttl_days": 30
  }'
```

**Response:**

```json
{ "swarm_id": "swarm-a1b2c3d4" }
```

### Add a member

```bash
curl -X POST http://localhost:8080/v1/swarm/member/add \
  -H "Authorization: Bearer $A1_ADMIN_SECRET" \
  -H "Content-Type: application/json" \
  -d '{
    "swarm_id": "swarm-a1b2c3d4",
    "agent_pk_hex": "4a1b2c3d...",
    "role": "worker",
    "capabilities": ["trade.equity"],
    "ttl_seconds": 3600
  }'
```

### List members

```bash
curl http://localhost:8080/v1/swarm/swarm-a1b2c3d4/members
```

**Response:**

```json
{
  "members": [
    {
      "agent_pk_hex": "4a1b2c3d...",
      "role": "worker",
      "capabilities": ["trade.equity"],
      "last_seen": "2025-05-06T12:00:00Z",
      "expires_at": "2025-05-06T13:00:00Z"
    }
  ]
}
```

### Remove a member

```bash
curl -X POST http://localhost:8080/v1/swarm/member/remove \
  -H "Authorization: Bearer $A1_ADMIN_SECRET" \
  -H "Content-Type: application/json" \
  -d '{ "swarm_id": "swarm-a1b2c3d4", "agent_pk_hex": "4a1b2c3d..." }'
```

---

## Rust

```rust
use a1::swarm::{SwarmRegistry, SwarmMember, SwarmRole};

let mut registry = SwarmRegistry::new();

let member = SwarmMember {
    agent_pk: agent.verifying_key(),
    passport_fingerprint: passport.fingerprint(),
    role: SwarmRole::Worker,
    last_seen: now,
};

registry.join(member)?;

let members = registry.list();
for m in members {
    println!("{:?}: {}", m.role, hex::encode(m.agent_pk.as_bytes()));
}
```

---

## TypeScript

```typescript
import { A1Client } from "a1-ai";

const client = new A1Client("http://localhost:8080", {
    adminSecret: process.env.A1_ADMIN_SECRET,
});

// Create swarm
const { swarmId } = await client.post("/v1/swarm/create", {
    name: "trading-swarm",
    capabilities: ["trade.equity"],
    ttlDays: 30,
});

// List members
const { members } = await client.get(`/v1/swarm/${swarmId}/members`);
```

---

## Roles

| Role | Typical use |
|---|---|
| `orchestrator` | Top-level agent that delegates to workers. Holds full swarm capability set. |
| `worker` | Executes specific tasks. Holds only the capabilities needed for its tasks. |
| `supervisor` | Monitors workers. May hold `audit.read` but not execution capabilities. |
| `auditor` | Read-only observer. No execution capabilities. |

Roles are metadata — they do not automatically restrict capabilities. Capability restrictions are enforced by the `DelegationCert` issued to each member.

---

*Source: `src/swarm.rs`, `sdk/python/a1/swarm.py`, `sdk/typescript/src/swarm.ts` · [Back to wiki home](Home)*
