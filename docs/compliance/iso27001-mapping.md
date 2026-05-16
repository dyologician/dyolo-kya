# A1 ISO/IEC 27001:2022 Annex A Control Mapping

**Version:** 2.8.0
**Document type:** Compliance reference
**Scope:** AI agent delegation authorization using A1

---

## How to use this document

Present this mapping to your ISO 27001 certification auditor alongside your A1 deployment configuration. Each row shows the Annex A control, its objective, how A1 satisfies it, and where to locate evidence in the codebase or deployment.

---

## Annex A.5 — Organizational Controls

| Control | Title | A1 Implementation | Evidence |
|---------|-------|--------------------------|----------|
| A.5.15 | Access control | `DyoloPassport` assigns a unique cryptographic identity per agent. No agent executes a capability without a valid `DelegationCert` signed by the authorizing principal. | `src/passport/mod.rs`; `src/identity/narrowing.rs` |
| A.5.16 | Identity management | Each passport carries a `namespace` field that uniquely identifies the agent system. Sub-passports carry the full delegation lineage. | `src/passport/mod.rs`; `DyoloPassport::namespace` |
| A.5.18 | Access rights | `NarrowingMatrix` enforces that delegated rights are a strict subset of the grantor's rights. Escalation beyond the parent mask is cryptographically impossible. | `src/identity/narrowing.rs::NarrowingMatrix::enforce_narrowing` |
| A.5.33 | Protection of records | `AuditRecord` is append-only. Each record is Blake3-hashed and timestamped. Exporters ship records to SIEM without modification. | `src/audit.rs`; `sdk/python/a1/siem.py` |

---

## Annex A.6 — People Controls

| Control | Title | A1 Implementation | Evidence |
|---------|-------|--------------------------|----------|
| A.6.2 | Terms and conditions of employment | `DyoloPassport` binds a human principal (identified by Ed25519 public key) to every delegation chain. The `ProvableReceipt` records which human authorized each agent action. | `src/identity/receipt.rs` |

---

## Annex A.8 — Technological Controls

| Control | Title | A1 Implementation | Evidence |
|---------|-------|--------------------------|----------|
| A.8.2 | Privileged access rights | Capability narrowing via `NarrowingMatrix` is a 256-bit bitwise AND. A delegated agent can only receive capabilities that the delegating passport already holds. Over-delegation returns `PASSPORT_NARROWING_VIOLATION`. | `src/identity/narrowing.rs` |
| A.8.3 | Information access restriction | `DyoloChain::with_namespace` provides hard multi-tenant namespace isolation. Chains from namespace A cannot authorize intents scoped to namespace B. | `src/chain.rs` |
| A.8.5 | Secure authentication | All delegation certs are signed with Ed25519 (RFC 8037). Key material is never transmitted. `VaultSigner` integrations support hardware-backed HSMs and cloud KMS. | `src/crypto.rs`; `sdk/python/a1/vault.py` |
| A.8.12 | Data leakage prevention | Private keys are zeroized on drop (`zeroize` crate). The gateway exposes only public information in its API responses. | `src/crypto.rs` |
| A.8.15 | Logging | Every `authorize` call emits a structured `AuditRecord` with principal, agent, intent, timestamp, and outcome. Records can be forwarded to Splunk, Datadog, or any OpenTelemetry-compatible backend. | `sdk/python/a1/siem.py` |
| A.8.16 | Monitoring activities | `AuditRecord` stream feeds enterprise SIEM exporters. Anomalous delegation patterns (e.g. unexpected capability escalation attempts) surface as structured events with `outcome: denied`. | `src/audit.rs`; `sdk/python/a1/siem.py` |
| A.8.20 | Network security | Gateway communication uses TLS. The Docker Compose configuration binds to localhost by default. Air-gapped deployments are supported: no external calls are made. | `docker/Dockerfile`; `docker/docker-compose.yml` |
| A.8.23 | Web filtering | Not applicable (A1 does not perform web filtering). |  |
| A.8.24 | Use of cryptography | Ed25519 for signing, Blake3 for hashing, optional CBOR for binary encoding. All primitives are FIPS-adjacent and widely audited. | `src/crypto.rs`; `Cargo.toml` dependency list |
| A.8.28 | Secure coding | Rust's memory safety guarantees eliminate buffer overflows and use-after-free. `subtle` crate provides constant-time comparison to prevent timing side-channels. | `Cargo.toml`; `src/crypto.rs` |
| A.8.32 | Change management | Semantic versioning. Wire schema (`wire/schema.json`) is versioned. Breaking changes require a major version bump. | `wire/schema.json`; `CHANGELOG.md` |
| A.8.33 | Test information | Full integration test suite (`tests/`). Property-based tests with `proptest`. Benchmark suite with Criterion. | `tests/integration.rs`; `tests/passport_integration.rs`; `benches/chain_bench.rs` |

---

## Certification preparation checklist

- [ ] Deploy A1 gateway with TLS termination
- [ ] Configure `VaultSigner` with your organization's KMS (AWS KMS, GCP KMS, HashiCorp Vault, or Azure Key Vault)
- [ ] Enable audit log forwarding to your SIEM (see `sdk/python/a1/siem.py`)
- [ ] Store `passport.json` files in your secrets manager, not on disk
- [ ] Run `a1 revoke` as part of your offboarding procedure
- [ ] Include `wire/schema.json` as an appendix to your ISMS documentation
- [ ] Reference this document in your Statement of Applicability (SoA)
