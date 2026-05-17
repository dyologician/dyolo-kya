# Compliance

A1 ships with pre-built compliance documentation for the two most common enterprise security frameworks. The mapping files live in `docs/compliance/` and are designed to be presented directly to auditors.

---

## SOC 2 Type II

`docs/compliance/soc2-mapping.md` maps every SOC 2 Trust Service Criteria (TSC) control to A1 code controls.

### CC6 — Logical and Physical Access Controls

| Criterion | A1 Mechanism |
|---|---|
| CC6.1 — Logical access restricted to authorized individuals | `DyoloPassport` issues per-agent identity. No agent executes without a valid, unexpired, cryptographically signed `DelegationCert`. |
| CC6.2 — New access provisioning requires authorization | `DyoloPassport.issue_sub` enforces that sub-certs can only be issued by the passport holder's private key. Unauthorized issuance produces a signature failure, not a silent grant. |
| CC6.3 — Access removed upon termination | `RevocationStore` maintains a deny-list. `a1 revoke <cert-id>` adds the fingerprint immediately. All subsequent attempts return `REVOKED`. |
| CC6.6 — Unauthorized access prevention | Capability narrowing via `NarrowingMatrix` enforces that each agent can only perform actions in its cert. Requesting an unauthorized capability returns `PASSPORT_NARROWING_VIOLATION`. |
| CC6.7 — Confidential transmission protected | Private keys never leave the signing backend. All certs contain only public keys and signed hashes. Gateway communicates over TLS. |
| CC6.8 — Unauthorized changes detected | `DelegationCert::signable_bytes` includes all cert fields in the signed digest. Any field mutation invalidates the Ed25519 signature. |

### CC7 — System Operations

| Criterion | A1 Mechanism |
|---|---|
| CC7.1 — Infrastructure protected from unauthorized changes | Air-gappable: all verification is local. Gateway has no outbound dependencies at authorization time. |
| CC7.2 — Security events monitored | `AuditSink` captures every authorization attempt. NDJSON wire format feeds Splunk, Datadog, or any SIEM. |
| CC7.3 — Security events evaluated | `ProvableReceipt` provides a tamper-evident, independently verifiable record of every authorized action. |
| CC7.4 — Incidents identified and responded to | Structured DENIED events with `error_code` fields enable automated alerting to PagerDuty, OpsGenie, etc. |

### CC9 — Risk Mitigation

| Criterion | A1 Mechanism |
|---|---|
| CC9.2 — Vendor risk management | Self-hostable gateway, no cloud dependency at authorization time. Air-gap compatible. |

---

## ISO/IEC 27001:2022

`docs/compliance/iso27001-mapping.md` maps Annex A controls.

### Annex A.5 — Organizational Controls

| Control | A1 Implementation |
|---|---|
| A.5.15 — Access control | `DyoloPassport` assigns a unique cryptographic identity per agent. No agent executes without a valid `DelegationCert`. |
| A.5.16 — Identity management | Each passport carries a `namespace` uniquely identifying the agent system. Sub-passports carry the full delegation lineage. |
| A.5.18 — Access rights | `NarrowingMatrix` enforces that delegated rights are a strict subset of the grantor's rights. Escalation is cryptographically impossible. |
| A.5.33 — Protection of records | `AuditRecord` is append-only. Each record is Blake3-hashed and timestamped. Exporters ship records to SIEM without modification. |

### Annex A.8 — Technological Controls

| Control | A1 Implementation |
|---|---|
| A.8.2 — Privileged access rights | Capability narrowing via `NarrowingMatrix` is a 256-bit bitwise AND. Over-delegation returns `PASSPORT_NARROWING_VIOLATION`. |
| A.8.3 — Information access restriction | `DyoloChain::with_namespace` provides hard multi-tenant namespace isolation. Chains from namespace A cannot authorize in namespace B. |
| A.8.5 — Secure authentication | All certs are signed with Ed25519. Key material is never transmitted. `VaultSigner` integrations support HSMs and cloud KMS. |
| A.8.15 — Logging | `AuditSink` with tamper-evident `ProvableReceipt`. Every event includes chain fingerprint, capability mask, intent hash. |
| A.8.24 — Use of cryptography | Ed25519, Blake3, optional ML-DSA hybrid. All algorithms are NIST-approved. Key rotation via passport reissue. |

---

## HIPAA

For healthcare deployments where A1 protects AI agents that access PHI:

| HIPAA Safeguard | A1 Mechanism |
|---|---|
| Access controls (§164.312(a)) | `DyoloPassport` + `NarrowingMatrix` enforce per-agent access restrictions |
| Audit controls (§164.312(b)) | `AuditSink` captures every access attempt with agent identity |
| Transmission security (§164.312(e)) | Gateway TLS; private keys never transmitted |
| Workforce authorization (§164.308(a)(3)) | Revocation via `RevocationStore` — immediate effect |

---

## Evidence collection for auditors

| Audit requirement | Where to find evidence |
|---|---|
| Access control configuration | `passport.json` files; gateway `/v1/passports/list` |
| Authorization logs | `AuditSink` export; gateway SIEM integration |
| Revocation records | `RevocationStore`; `/v1/cert/:fingerprint` |
| Key management | `VaultSigner` implementation; KMS audit logs |
| Cryptographic controls | `src/crypto.rs`; `src/identity/narrowing.rs` |
| Incident response | DENIED audit events; `error_code` fields |
| Sample audit report | `docs/compliance/sample-audit-report.md` |

---

## Generate an audit report

```bash
curl -X POST http://localhost:8080/v1/governance/audit-report \
  -H "Authorization: Bearer $A1_ADMIN_SECRET" \
  -H "Content-Type: application/json" \
  -d '{
    "from": "2025-01-01T00:00:00Z",
    "to": "2025-06-01T00:00:00Z"
  }'
```

The report includes all authorization events with full chain fingerprints, suitable for direct submission to auditors.

---

*Source: `docs/compliance/` · [Back to wiki home](Home)*
