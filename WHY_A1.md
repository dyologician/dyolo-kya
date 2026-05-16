# A1 — One Identity. Full Provenance.

> The cryptographic identity layer for AI agents. Every agent gets a verifiable passport. Every action leaves an irrefutable trail. Every enterprise gets the audit proof they need.

---

## The Problem No One Has Solved

AI agents are now doing real work: executing trades, modifying files, calling APIs, delegating tasks to other agents, which delegate to more agents. This is happening right now, at scale, in production, at enterprises that cannot afford to get it wrong.

But there is no answer to the most basic question: **Who authorized this?**

When an AI agent executes a trade or modifies a database, the real question is not "did the agent have a valid token?" It is: "which human said it was allowed to do exactly this, and can we prove that cryptographically?" Current tools — JWT tokens, OAuth scopes, SPIFFE certificates — were designed for services that run code written by humans. They collapse entirely when one AI agent starts delegating to another.

This is called the **Recursive Delegation Gap**. It is the reason most enterprises cannot deploy multi-agent AI systems in regulated environments today.

---

## What A1 Is

A1 is the cryptographic identity and authorization layer that closes the Recursive Delegation Gap.

Every AI agent gets a **passport** — a long-lived, cryptographically signed identity that encodes exactly what the agent is allowed to do. When that agent delegates a task to another agent, the delegation is signed, scoped, and time-limited. The inner agent receives a **certificate** that is cryptographically chained to the original human authorization. This chain can be verified by anyone, offline, in milliseconds, at any depth.

The result is that every agent action carries an **irrefutable chain of custody** from the executing agent all the way back to the human who authorized it. Not as a log entry that could be altered. As a cryptographic proof that cannot.

---

## Why Existing Tools Fail

| Tool | What It Does | Where It Fails |
|------|-------------|----------------|
| JWT tokens | Proves a service has a valid session | No chaining — agent A's token says nothing about agent B |
| OAuth scopes | Limits what a token can do | Scopes are strings, not cryptographic capabilities; cannot enforce delegation depth |
| SPIFFE/X.509 | Proves a workload is who it says it is | Identity only — no authorization chain, no capability narrowing, no audit trail |
| API keys | Simple authentication | No scoping, no delegation, no provenance |
| Audit logs | Records what happened | Mutable, centralized, not a proof |

A1 is none of these. It is not a replacement for your authentication system. It is the layer that makes delegation cryptographically accountable — which nothing else provides.

---

## How A1 Works (Plain English)

Think of it as a chain of signed letters of authorization.

1. A human (or root system) creates a **passport** for an AI agent. The passport says: "This agent is allowed to execute trade orders and read portfolio data, for the next 30 days, signed by me."

2. That agent receives a task. To complete it, it delegates to a sub-agent. It issues a **sub-certificate**: "I authorize this sub-agent to read portfolio data only, for the next hour, signed by me." The sub-certificate cannot exceed the passport's permissions — this is enforced cryptographically, not by policy.

3. The sub-agent acts. Before the action executes, A1 verifies the entire chain: every signature, every expiry, every scope boundary, every nonce. If anything is wrong — forged signature, expired cert, escalated scope — the action is rejected.

4. A **receipt** is produced. It records the chain fingerprint, the authorized capability, and a cryptographic commitment to the enforced scope. This receipt is the audit proof. It can be stored, shipped to a SIEM, anchored on-chain, or handed to an auditor. It proves what happened and who authorized it, permanently.

---

## What A1 Provides

### For Enterprises

**Regulatory compliance out of the box.** A1's receipt format maps directly to EU AI Act Article 13 (transparency), NIST AI RMF Govern 1.7 (accountability), and SOC 2 CC6.1 (logical access). You get the audit artifacts that regulators require without building them yourself.

**Human oversight, enforced, not logged.** Every action an AI agent takes is cryptographically linked to a human authorization. This is not a log that says "a human approved this." This is a proof that they did.

**Offline verification.** A1 does not require a network call to verify an authorization. The chain verifies with the public keys it carries. This means verification works in air-gapped environments, at the edge, in embedded hardware, and under network partitions.

**Multi-tenant isolation.** Namespace-scoped chains mean that an authorization issued for tenant A cannot be replayed under tenant B. This is enforced cryptographically.

**Revocation.** Any certificate in a chain can be revoked. The gateway supports Redis-backed and Postgres-backed revocation stores with sub-millisecond check latency. Revocation is durable across restarts.

### For Developers

**One-line integration.** The Python, TypeScript, and Go SDKs provide drop-in middleware decorators that protect any AI agent tool or function with a single annotation.

**Language-agnostic REST gateway.** Any service in any language can authorize against the A1 gateway via HTTP. No Rust required.

**Framework-native.** Native integrations for LangChain, LangGraph, LlamaIndex, AutoGen, CrewAI, Semantic Kernel, and the OpenAI Agents SDK.

**W3C Verifiable Credentials.** Every passport can issue a portable VC that any system — another agent, a blockchain, an enterprise IAM, an EU eIDAS wallet — can verify without A1-specific code.

**Post-quantum ready.** The wire format supports `HybridMlDsa44Ed25519` and `HybridMlDsa65Ed25519` hybrid signatures today. Switch the feature flag when your deployment requires full quantum resistance.

---

## The Technical Foundation

A1 is built on five cryptographic primitives that together close every gap.

**1. DyoloIdentity (Ed25519)**
Each agent has an Ed25519 signing keypair. All certificates are signed with Ed25519. Batch verification uses the standard `ed25519-dalek` batch verifier, which is 5–10× faster than individual checks and constant-time.

**2. NarrowingMatrix (256-bit capability bitmask)**
Capabilities map to bits in a 256-bit field via Blake3. Enforcement is a single bitwise AND: `sub & parent == sub`. This check is O(1) regardless of how many capabilities exist. A sub-delegation can never carry capabilities its parent does not have — this is a mathematical property, not a policy.

**3. DelegationCert (signed, chained, expiring)**
Every hop in a delegation chain is a signed certificate. Certificates carry a nonce (preventing replay), an expiry (preventing indefinite delegation), a max-depth field (preventing unbounded chains), and a scope proof (proving the authorized intent set is a subset of the parent's). All of these are checked in a single pass during `DyoloChain::authorize`.

**4. Blake3 (domain-separated everywhere)**
All hashing uses Blake3 with distinct domain strings for every context. This prevents length-extension attacks, cross-context collisions, and hash confusion. The domain for capability commitment, cert fingerprinting, nonce storage, chain fingerprinting, VC signing, and ZK commitment are all distinct.

**5. ProvableReceipt**
After every successful authorization, a receipt is produced. It contains the chain depth, the chain fingerprint, the authorized intent hash, the enforced capability mask (hex), a Blake3 commitment over the mask, and an optional reasoning trace root. Receipts are immutable, self-describing, and independently verifiable.

---

## W3C DID + Verifiable Credentials

Every DyoloPassport holder has a permanent `did:a1:{hex-pubkey}` identifier. This is a W3C DID that resolves to a standard DID Document with no registry, no network dependency.

The gateway issues W3C Verifiable Credentials from passports. A VC asserts the subject agent's authorized capabilities, signed by the issuing authority. The signature covers a Blake3 hash of all credential fields, immune to JSON canonicalization attacks.

Any system that can verify an Ed25519 signature can verify an A1 VC — no A1 library required.

---

## Post-Quantum Security

The A1 wire format is quantum-ready today. Every delegation certificate supports three signature modes:

- **Ed25519** (default) — classical, 128-bit security, fast
- **HybridMlDsa44Ed25519** — ML-DSA-44 + Ed25519 dual signature, 128-bit post-quantum
- **HybridMlDsa65Ed25519** — ML-DSA-65 + Ed25519 dual signature, 192-bit post-quantum (recommended for financial and government deployments)

Classical and hybrid certs are interoperable within a chain during a migration window. A chain can start with Ed25519 root certs and transition to hybrid leaf certs monotonically — no hard cutover required.

The ML-DSA backend is activated with the `post-quantum` feature flag. Until then, the full hybrid wire format is in place and the Ed25519 component is fully verified. Archives created today can be retroactively upgraded to full PQ verification.

---

## ZK Chain Commitments

The gateway can produce a `ZkChainCommitment` — a 32-byte compact commitment to a full delegation chain's validity. Instead of shipping the entire chain to every verifier, distribute the commitment. Verification is O(1): one Blake3 check plus one Ed25519 check, regardless of chain depth.

The commitment carries a `mode` field. `Blake3Commit` is the default — cryptographic commitment, no zkVM required. `ExternalZkvm` signals that a real zero-knowledge proof (RISC Zero, Jolt, SP1) has been attached. Both modes share the same wire format. Consumers that check commitments today will continue working unchanged when the zkVM upgrade ships.

---

## Integration in 5 Minutes

**Python (LangChain, AutoGen, CrewAI)**

```python
from a1 import a1_guard, PassportClient

client = PassportClient("http://localhost:8080")

@a1_guard(client, "trade.equity")
def execute_trade(symbol: str, qty: int):
    # This function only runs if the calling agent has trade.equity capability
    ...
```

**TypeScript (OpenAI Agents SDK, LangGraph)**

```typescript
import { withA1Passport } from '@a1/sdk';

const guardedTool = withA1Passport(client, 'trade.equity', async (params) => {
  // This only runs if the signed chain authorizes trade.equity
});
```

**REST (any language)**

```bash
curl -X POST http://localhost:8080/v1/authorize \
  -H "Content-Type: application/json" \
  -d '{
    "chain": { ... },
    "intent_name": "trade.equity",
    "executor_pk_hex": "abc123..."
  }'
```

**Issue a passport**

```bash
a1 passport issue \
  --namespace "acme-trading-bot" \
  --allow "trade.equity,portfolio.read" \
  --ttl 30d \
  --out acme-passport.json
```

---

## Regulatory Compliance Mapping

| Requirement | Standard | A1 Capability |
|-------------|----------|---------------|
| Human oversight of AI systems | EU AI Act Art. 13, 14 | `ProvableReceipt` traces every action to a human authorization |
| Accountability and auditability | NIST AI RMF Govern 1.7 | Blake3 commitment in every receipt; SIEM exporters built in |
| Access control and least privilege | SOC 2 CC6.1, ISO 27001 A.9 | `NarrowingMatrix` enforces capability subsets cryptographically |
| Non-repudiation | NIST SP 800-53 AU-10 | Ed25519 signatures on every cert; batch verification |
| Key management | NIST SP 800-57 | `VaultSigner` for AWS KMS, GCP KMS, HashiCorp Vault, Azure Key Vault |
| Cryptographic agility | NIST post-quantum guidance | Hybrid ML-DSA wire format; feature-flag activation path |
| Multi-tenancy isolation | FedRAMP, SOC 2 | Namespace-scoped chains with cryptographic tenant separation |

---

## What A1 Is Not

A1 does not replace your authentication system. If your agents log in with OAuth, keep that. A1 sits alongside it, answering a different question: not "is this agent who it says it is?" but "is this agent authorized to do this specific thing right now, under a verifiable chain of human oversight?"

A1 does not require a blockchain. All verification is offline, using only the public keys in the chain. On-chain anchoring is optional, using ZK commitments.

A1 does not require a central server for verification. The chain verifies anywhere the public keys are available. The gateway is optional — useful for enterprise deployments but not required.

---

## Get Started

```bash
# Install the CLI
cargo install a1-cli

# Generate a keypair
a1 keygen

# Start the gateway (Docker)
docker run -p 8080:8080 ghcr.io/dyologician/a1-gateway:2.8.0

# Issue your first passport
a1 passport issue --namespace "my-agent" --allow "read,write" --ttl 7d

# Integrate in Python
pip install a1-sdk
```

Full documentation: https://docs.a1.dev  
GitHub: https://github.com/dyologician/a1  
crates.io: https://crates.io/crates/a1
