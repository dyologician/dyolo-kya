# Security Policy

## Overview

A1 is a cryptographic authorization library used in production AI agent deployments. Security is the product's core function. We treat every security report seriously and respond rapidly.

---

## Supported Versions

| Version | Supported |
|---|---|
| 2.8.x | ✅ Active support — security fixes + features |
| 2.x.x (< 2.8) | ⚠️ Security fixes only (patch releases) |
| 1.x.x | ❌ End of life — upgrade to 2.8 |

---

## Reporting a Vulnerability

**Do not report security vulnerabilities through public GitHub Issues, Discussions, or Pull Requests.**

### How to report

Email **workwithdyolo@gmail.com** with the subject line: `[A1 SECURITY] <short description>`

For highly sensitive reports, request our PGP public key by email before sending details.

### What to include

A good report helps us triage and fix faster. Please include:

- **Description** — What the vulnerability is and what it allows an attacker to do.
- **Affected version(s)** — Which version(s) you tested against.
- **Affected component** — Which module or feature (e.g., `NarrowingMatrix`, `DyoloChain`, gateway `/v1/authorize`).
- **Reproduction steps** — A minimal, self-contained reproducer (Rust test, Python script, curl command).
- **Impact assessment** — Your assessment of severity (scope escalation, data exposure, denial of service, etc.).
- **Suggested fix** — If you have one (optional but appreciated).

### Response timeline

| Stage | Commitment |
|---|---|
| Initial acknowledgement | Within 48 hours of receipt |
| Triage and severity assessment | Within 7 days |
| Fix development begins | Within 7 days of confirmation |
| Patch released (critical) | Within 14 days of confirmation |
| Patch released (high/medium) | Within 30 days of confirmation |
| Public disclosure | Coordinated with reporter — typically 90 days after fix |

We will keep you informed at each stage. If you do not receive an acknowledgement within 48 hours, follow up to ensure delivery.

### Bug bounty

We do not currently operate a formal bug bounty program. We do publicly credit reporters in release notes (unless anonymity is requested) and make every effort to thank contributors who identify significant issues.

---

## Threat Model

### What A1 protects against

| Threat | Mitigation |
|---|---|
| **Scope escalation** — agent claims a capability its parent did not grant | `NarrowingMatrix` bitwise enforcement: `child_mask & parent_mask == child_mask` at both issuance and guard time |
| **Forged delegation certificates** | Ed25519 signature verification on every `DelegationCert` in the chain; tampered bytes fail immediately |
| **Replay attacks** — reuse of a previously authorized intent | Per-intent 128-bit nonce consumed atomically in `NonceStore`; same nonce rejected on second use |
| **Temporal abuse** — cert used after expiry or before validity | `expiration_unix` and `not_before` enforced on every cert; child cert cannot expire after parent |
| **Certificate revocation bypass** | Every chain authorization checks `RevocationStore` by cert fingerprint before accepting |
| **Cross-tenant authorization** | Namespace binding enforced before any signature verification; `tenant-acme` certs cannot authorize under `tenant-beta` |
| **Timing side-channels** | All equality comparisons on sensitive bytes (fingerprints, nonces, keys) use `subtle::ConstantTimeEq` |
| **Private key material leakage** | `DyoloIdentity` implements `ZeroizeOnDrop`; KMS signing patterns (`VaultSigner`) eliminate in-process key material entirely |
| **Tampered audit receipts** | `ProvableReceipt` contains a Blake3 commitment over all enforced fields; independent verification requires no secrets |
| **Chain fingerprint collision** | Blake3 over all cert bytes; preimage resistance is 256-bit |
| **Privilege escalation via ZK commitment** | `ZkChainCommitment` includes a Blake3 binding over the chain fingerprint and capability mask; the commitment cannot be detached from its original authorization |
| **Hash injection / length extension** | All intent and parameter hashing uses prefix-free canonical encoding with domain-separated Blake3 |
| **Post-quantum harvest-now-decrypt-later** | Hybrid ML-DSA + Ed25519 wire format; deployments can enable PQ verification without breaking existing chains |

### What A1 does NOT protect against

Understanding the security boundaries is as important as understanding the protections.

**Storage layer compromise.** If an attacker gains write access to the `RevocationStore` or `NonceStore` (e.g., directly to your Redis or Postgres instance), replay and revocation protections are bypassed. Securing these stores is the responsibility of the deploying application. See [wiki/Enterprise-Deployment.md](wiki/Enterprise-Deployment.md) for recommended configurations.

**Key compromise.** If an agent's private Ed25519 key is leaked, actions taken by an attacker using that key are indistinguishable from legitimate actions — until revocation. Revoke the compromised cert immediately using `a1 revoke <fingerprint>` or the gateway's `POST /v1/cert/revoke` endpoint.

**Host compromise.** A1 cannot protect against an attacker with root access to the machine running the gateway or agent process. Use standard host hardening, secret management, and network segmentation.

**Human principal compromise.** If the human who issued the root passport is compromised, the entire delegation tree under that passport is compromised. Protect root passport keys using KMS (see [wiki/KMS-Integration.md](wiki/KMS-Integration.md)).

**Logic errors outside A1.** A1 verifies that an agent was authorized to execute a capability. It does not verify that the agent's actual code does what it claims. Capability names are human-defined strings — "trade.equity" means whatever your system says it means.

---

## Cryptographic Primitive Choices

### Ed25519 (RFC 8032)

Used for all `DelegationCert` signatures.

- **Security level:** 128-bit (equivalent to RSA-3072, ECDSA-P256)
- **Key size:** 32-byte private scalar, 32-byte verifying key
- **Signature size:** 64 bytes
- **Why not ECDSA?** Ed25519 requires no per-signature randomness (no `k` value), eliminating the most common ECDSA implementation failure mode. There are no weak parameter choices.
- **Why not RSA?** RSA keys are 10–100× larger, verification is slower, and RSA has a significantly larger parameter attack surface.
- **Library:** `ed25519-dalek` v2 (audited; uses `curve25519-dalek` v4 with `zeroize` support).

### Blake3

Used for all hashing and commitments.

- **Security level:** 256-bit collision resistance
- **Domain separation:** All uses are prefixed with a unique domain string (e.g., `"dyolo::narrowing::v1"`, `"dyolo::intent::v1"`) to prevent cross-context collisions.
- **Why not SHA-256?** Blake3 is ~10× faster, hardware-accelerated on modern CPUs, and inherently immune to length-extension attacks (unlike SHA-256 in Merkle-Damgård mode).
- **Why not SHA-3/SHAKE?** Blake3 is faster in software and achieves the same security level.

### subtle (constant-time comparisons)

All equality checks on fingerprints, nonces, and public keys use `subtle::ConstantTimeEq` to prevent timing side-channels that could leak information about partial matches.

### ML-DSA (CRYSTALS-Dilithium) — hybrid post-quantum

Used in `HybridSignature` when the `post-quantum` feature is enabled.

- **Levels supported:** ML-DSA-44 (NIST Level 2, 128-bit post-quantum) and ML-DSA-65 (NIST Level 3, 192-bit post-quantum)
- **Hybrid mode:** Both Ed25519 and ML-DSA signatures must verify. A classical-only attacker must break Ed25519; a quantum attacker must break ML-DSA. Neither alone is sufficient.
- **Wire stability:** The hybrid cert format is stable as of v2.8.0. Enabling `post-quantum` activates real ML-DSA verification without a wire format change.

---

## Unsafe Code Policy

The main `a1` crate enforces `#![deny(unsafe_code)]` at the crate level. The only exception is the `ffi` module, which:

1. Uses `unsafe` exclusively for crossing the C ABI boundary.
2. Has explicit safety contracts documented on every `unsafe` block.
3. Is isolated in `src/ffi.rs` and gated behind the `ffi` feature flag.

Sub-crates (`a1-redis`, `a1-pg`, `a1-gateway`, `a1-cli`, `a1-identity`) all enforce `#![deny(unsafe_code)]` with no exceptions.

---

## Dependency Security

All Rust dependencies are pinned to major versions and audited through `cargo audit` in CI. Third-party dependencies that handle cryptographic material are limited to:

| Crate | Purpose | Audit status |
|---|---|---|
| `ed25519-dalek` v2 | Ed25519 signing and verification | Regularly audited by the dalek cryptography team |
| `blake3` | Hashing and commitments | Audited; developed by the BLAKE3 team |
| `subtle` | Constant-time comparisons | Part of the dalek ecosystem; widely reviewed |
| `zeroize` | Secure memory clearing | Part of the RustCrypto project |

We run `cargo audit` on every CI push and immediately triage any advisories.

---

## Security Contacts

| Role | Contact |
|---|---|
| Primary security contact | workwithdyolo@gmail.com |
| GitHub repository | https://github.com/dyologician/a1 |
| Security advisories | https://github.com/dyologician/a1/security/advisories |

---

## Acknowledgements

We publicly acknowledge all reporters who identify and responsibly disclose security issues, unless they request anonymity. Past acknowledgements will be listed in [CHANGELOG.md](CHANGELOG.md) under the relevant release.
