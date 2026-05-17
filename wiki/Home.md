# A1 Wiki

**Cryptographic chain-of-custody for recursive AI agent delegation.**
Version 2.8.0 · MIT OR Apache-2.0

---

## What is A1?

When one AI agent hands off a task to another AI agent, the original human's authorization disappears. There is no way to prove who approved the action at the end of the chain. This is called the **Recursive Delegation Gap** — and it is the reason regulated industries (finance, healthcare, government) cannot safely deploy multi-agent AI systems at scale.

A1 closes this gap. It gives every agent in every delegation chain an unforgeable cryptographic identity, and produces an independently verifiable receipt for every action taken.

---

## Key properties

| Property | Description |
|---|---|
| **Air-gap compatible** | All verification is local. No network call at authorization time. |
| **No vendor lock-in** | Self-hostable, open-source, no cloud dependency. |
| **Zero unsafe Rust** | `#![deny(unsafe_code)]` enforced at crate level (isolated `ffi` module with documented contracts). |
| **Backward compatible** | All v2.0.0 chains and certs remain fully valid under v2.8.0. |
| **One-line adoption** | `@a1_guard`, `withA1Passport`, `a1.WithPassport`. |
| **Post-quantum ready** | ML-DSA hybrid wire format. Zero migration cost when you upgrade. |
| **Enterprise-ready** | AWS KMS, GCP KMS, HashiCorp Vault, Azure Key Vault. SOC 2 + ISO 27001 mapping included. |

---

## Pages in this wiki

### Getting started
- [Quickstart Guide](Quickstart) — From zero to a guarded agent in 5 minutes
- [Passport Guide](Passport-Guide) — Full passport lifecycle and delegation patterns

### Architecture and security
- [Security Model](Security-Model) — Ed25519, Blake3, NarrowingMatrix, nonce replay, revocation
- [How It Compares](How-It-Compares) — A1 vs JWT vs SPIFFE/SPIRE vs OAuth2
- [Post-Quantum Signatures](Post-Quantum) — Hybrid ML-DSA-44/65 + Ed25519, zero breaking changes
- [Zero-Knowledge Commitments](ZK-Commitments) — ZkChainCommitment, reasoning traces, on-chain anchoring
- [DID and Verifiable Credentials](DID-VC) — W3C DID, Verifiable Credentials, eIDAS compatibility

### Enterprise
- [Enterprise Deployment](Enterprise-Deployment) — Production topology, TLS, Postgres, Redis, KMS
- [KMS Integration](KMS-Integration) — AWS KMS, GCP KMS, HashiCorp Vault, Azure Key Vault
- [SIEM Integration](SIEM-Integration) — Datadog, Splunk, OpenTelemetry, NDJSON, buffered export
- [Compliance](Compliance) — SOC 2 Type II, ISO 27001:2022, HIPAA mapping
- [Swarm Coordination](Swarm) — Multi-agent swarms, roles, peer discovery
- [Governance](Governance) — On-chain governance votes, DAO integration, audit reports

### SDK guides
- [Python SDK](Python-SDK) — LangChain, LangGraph, CrewAI, AutoGen, LlamaIndex, KMS, SIEM
- [TypeScript SDK](TypeScript-SDK) — LangChain.js, LangGraph, middleware, JWT exchange
- [Go SDK](Go-SDK) — Typed client, WithPassport guard, PassportClient

### Reference
- [CLI Reference](CLI-Reference) — All `a1` commands with flags and examples
- [Gateway API Reference](Gateway-API) — All REST endpoints, request/response schemas, error codes
- [Capabilities Reference](https://github.com/dyologician/A1/blob/main/CAPABILITIES.md) — Every feature explained
- [Protocol spec](https://github.com/dyologician/A1/blob/main/spec/A1-PROTOCOL.md) — Formal wire protocol
- [Rust API docs](https://docs.rs/a1-ai) — Full rustdoc on docs.rs

---

## Architecture diagram

```
┌─────────────────────────────────────────────────────┐
│                  Human Principal                     │
│          (issues DyoloPassport via CLI)              │
└────────────────────┬────────────────────────────────┘
                     │ Ed25519-signed root cert
                     ▼
┌─────────────────────────────────────────────────────┐
│              Orchestrator Agent                      │
│   (DelegationCert: trade.equity, portfolio.read)    │
└────────────────────┬────────────────────────────────┘
                     │ Ed25519-signed sub-cert
                     │ (NarrowingMatrix enforces subset)
                     ▼
┌─────────────────────────────────────────────────────┐
│               Executor Agent                         │
│        (DelegationCert: trade.equity only)           │
│                                                     │
│   calls: A1Gateway /v1/authorize                    │
│   receives: ProvableReceipt                         │
└────────────────────┬────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────┐
│                  A1 Gateway                          │
│  ┌─────────────┐  ┌────────────┐  ┌─────────────┐  │
│  │  NonceStore │  │RevocationSt│  │  AuditSink  │  │
│  │  (Redis/PG) │  │  (Redis/PG)│  │(DD/Splunk/..)│ │
│  └─────────────┘  └────────────┘  └─────────────┘  │
└─────────────────────────────────────────────────────┘
```

---

## Quick links

| Resource | URL |
|---|---|
| GitHub repository | https://github.com/dyologician/A1 |
| Rust crate (core) | https://crates.io/crates/a1-ai |
| Rust crate (a1-cli) | https://crates.io/crates/a1-cli |
| Rust crate (a1-gateway) | https://crates.io/crates/a1-gateway |
| Rust crate (a1-redis) | https://crates.io/crates/a1-redis |
| Rust crate (a1-pg) | https://crates.io/crates/a1-pg |
| Rust crate (a1-identity) | https://crates.io/crates/a1-identity |
| Python package | https://pypi.org/project/a1identity |
| npm package | https://www.npmjs.com/package/a1-ai |
| Rust API docs | https://docs.rs/a1-ai |
| Security policy | [SECURITY.md](../SECURITY.md) |
| Contributing | [CONTRIBUTING.md](../CONTRIBUTING.md) |
| Changelog | [CHANGELOG.md](../CHANGELOG.md) |
| Protocol spec | [spec/A1-PROTOCOL.md](../spec/A1-PROTOCOL.md) |

---

## Concepts glossary

| Term | Definition |
|---|---|
| **DyoloPassport** | The root identity for an AI agent. A self-signed Ed25519 certificate encoding the agent's full capability set. |
| **DelegationCert** | A signed credential issued by a delegator (agent or passport) to a delegatee (sub-agent). Always a subset of the delegator's capabilities. |
| **DyoloChain** | An ordered list of `DelegationCert`s from the root passport to the executing agent. |
| **NarrowingMatrix** | The 256-bit bitmask that enforces `child ⊆ parent` in O(1). |
| **ProvableReceipt** | The tamper-evident receipt produced for every authorized action. Independently verifiable. |
| **Intent** | A named action an agent requests authorization to perform (e.g., `trade.equity`). |
| **NonceStore** | The replay-attack prevention store. Each intent nonce can only be consumed once. |
| **RevocationStore** | The deny-list for cert fingerprints. Revoked certs cannot authorize. |
| **VaultSigner** | An `AsyncSigner` implementation that calls a KMS for signing. Key material never enters application memory. |
| **AuditSink** | A composable destination for authorization audit events (SIEM, OTLP, file). |
| **SubScopeProof** | A Merkle inclusion proof that a sub-cert's capability set is contained in its parent's. |
| **Namespace** | A tenant isolation scope. Chains scoped to one namespace cannot authorize in another. |
| **ZkChainCommitment** | A compact commitment proving an authorization occurred without revealing the chain. |

---

*Built and maintained by dyolo ([@dyologician](https://github.com/dyologician)).*
