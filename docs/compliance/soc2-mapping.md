# A1 SOC 2 Type II Control Mapping

**Version:** 2.8.0  
**Document type:** Compliance reference  
**Scope:** AI agent delegation authorization using A1

---

## How to use this document

This document maps A1 capabilities to SOC 2 Trust Service Criteria (TSC). Present it to your auditor alongside your A1 deployment configuration. For each criterion, the table shows the control objective, how A1 satisfies it, and where to find evidence.

---

## CC6 — Logical and Physical Access Controls

| Criterion | Control Objective | A1 Mechanism | Evidence Location |
|-----------|------------------|---------------------|-------------------|
| CC6.1 | Logical access is restricted to authorized individuals | `DyoloPassport` issues per-agent identity. No agent executes without a valid, unexpired, cryptographically signed `DelegationCert`. | `passport.json` files; gateway `/authorize` logs |
| CC6.2 | New access provisioning requires authorization | `DyoloPassport.issue_sub` enforces that sub-certs can only be issued by the passport holder's private key. Unauthorized issuance produces a signature verification failure, not a silent grant. | `DyoloChain::authorize` — Ed25519 signature check on every cert |
| CC6.3 | Access is removed upon termination | `RevocationStore` maintains a deny-list. `a1 revoke <cert-id>` adds the cert fingerprint. All subsequent authorization attempts with that cert return `REVOKED`. | `RevocationStore` implementation; `a1 revoke` CLI |
| CC6.6 | Logical access is restricted to authorized individuals | Capability narrowing via `NarrowingMatrix` enforces that each sub-agent can only perform actions explicitly listed in its cert. Requesting an unauthorized capability returns `PASSPORT_NARROWING_VIOLATION`. | `NarrowingMatrix::enforce_narrowing` |
| CC6.7 | Transmission of confidential information is protected | Private keys never leave the signing backend. All certs contain only public keys and signed hashes. The gateway communicates over TLS. | `VaultSigner` implementations; `docker/Dockerfile` TLS config |
| CC6.8 | Unauthorized changes to system components are detected | `DelegationCert::signable_bytes` includes all cert fields in the signed digest. Any field mutation invalidates the Ed25519 signature. | `src/cert.rs::DelegationCert` |

---

## CC7 — System Operations

| Criterion | Control Objective | A1 Mechanism | Evidence Location |
|-----------|------------------|---------------------|-------------------|
| CC7.1 | Infrastructure is protected from unauthorized changes | Air-gappable: all verification is local. The gateway has no outbound dependencies at authorization time. | `DyoloChain::authorize` — zero network calls |
| CC7.2 | Security events are monitored | `AuditSink` captures every authorization attempt (AUTHORIZED / DENIED / POLICY_VIOLATION / STORAGE_ERROR). NDJSON wire format feeds Splunk, Datadog, or any SIEM. | `src/audit.rs`; `sdk/python/a1/siem.py` |
| CC7.3 | Security events are evaluated and responded to | `ProvableReceipt` provides a tamper-evident, independently verifiable record of every authorized action. Auditors can replay the receipt without retaining secrets. | `src/identity/receipt.rs` |
| CC7.4 | Incidents are identified and responded to | Structured DENIED events with `error_message` fields enable automated alerting. Feed `outcome=DENIED` events to your PagerDuty or OpsGenie integration. | `AuditEvent::outcome`; `siem.py` exporters |

---

## CC9 — Risk Mitigation

| Criterion | Control Objective | A1 Mechanism | Evidence Location |
|-----------|------------------|---------------------|-------------------|
| CC9.2 | Business disruption risk is managed | Offline-first: nonce stores and revocation stores can be backed by Redis or Postgres with automatic failover. Memory stores are used for single-node deployments. | `a1-redis`; `a1-pg` crates |

---

## A1 — Availability

| Criterion | Control Objective | A1 Mechanism | Evidence Location |
|-----------|------------------|---------------------|-------------------|
| A1.1 | Availability commitments are established | Gateway `/healthz` endpoint exposes `NonceStore` and `RevocationStore` health. Load balancers pull unhealthy instances automatically. | `a1-gateway/src/routes/health.rs` |
| A1.2 | Infrastructure is protected | Docker compose configuration includes resource limits. Benchmark results show sub-microsecond narrowing checks — no throughput bottleneck under load. | `docker/docker-compose.yml`; `benches/chain_bench.rs` |

---

## PI1 — Processing Integrity

| Criterion | Control Objective | A1 Mechanism | Evidence Location |
|-----------|------------------|---------------------|-------------------|
| PI1.1 | Processing is complete, valid, accurate, and authorized | Every authorization result is a `VerifiedToken` or `ProvableReceipt` signed by the chain's principal. The `chain_fingerprint` field is a Blake3 digest over the full cert chain — any truncation or reordering invalidates it. | `src/chain.rs::VerificationReceipt` |
| PI1.4 | Processing deviations are identified and corrected | Nonce replay protection prevents the same intent token from being authorized twice. `NonceStore::mark_used` is idempotent and safe under concurrent access. | `src/registry.rs::NonceStore` |

---

## C1 — Confidentiality

| Criterion | Control Objective | A1 Mechanism | Evidence Location |
|-----------|------------------|---------------------|-------------------|
| C1.1 | Confidential information is identified | `DelegationCert` contains no plaintext action names — only Blake3 hashes of intent values. Capability names are hashed in `NarrowingMatrix` via Blake3 with a domain-separated key. | `src/intent.rs`; `src/identity/narrowing.rs` |
| C1.2 | Confidential information is protected during transmission | Certs are signed JSON objects. The signing key never transits the wire. All gateway communication occurs over TLS. | `wire/schema.json`; `docker/Dockerfile` |

---

## Auditor evidence package checklist

- [ ] `passport.json` files for all deployed root passports (redact private fields before sharing)
- [ ] Gateway access logs (NDJSON from `AuditSink`) for the audit period
- [ ] `a1 inspect <passport.json>` output showing namespace, capabilities, and expiry
- [ ] `RevocationStore` export showing all revoked certs during the audit period
- [ ] `/healthz` monitoring alert configuration
- [ ] KMS key rotation policy (if using `vault.py` signers)
- [ ] TLS certificate validity for all gateway endpoints

---

*This document is provided as a reference for SOC 2 audit preparation. It does not constitute legal or compliance advice. Engage your compliance team and qualified auditors before submitting.*