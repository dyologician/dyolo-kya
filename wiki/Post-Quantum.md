# Post-Quantum Signatures

A1 supports hybrid ML-DSA + Ed25519 signatures via the `post-quantum` feature flag. All v2.8.0 chains and certs remain fully valid — there is no breaking change and no hard cutover required.

---

## Background

Ed25519 is secure today but will be broken by sufficiently powerful quantum computers. The NIST post-quantum standardization process produced ML-DSA (CRYSTALS-Dilithium) as the standard for digital signatures. A1's wire format supports both algorithms simultaneously — classical chains continue to verify normally, and new chains can opt into hybrid signing at any time.

---

## Algorithm levels

| Algorithm tag | Security level | Public key | Signature | Use case |
|---|---|---|---|---|
| `Ed25519` (default) | 128-bit classical | 32 bytes | 64 bytes | All current deployments |
| `HybridMlDsa44Ed25519` | 128-bit post-quantum (NIST Level 2) | 1344 bytes | 2484 bytes | Financial, healthcare |
| `HybridMlDsa65Ed25519` | 192-bit post-quantum (NIST Level 3) | 1984 bytes | 3373 bytes | Government, defense |

Both hybrid modes require **both** the ML-DSA and Ed25519 components to pass. A verifier that cannot evaluate the ML-DSA component must reject the cert — there is no downgrade fallback.

---

## Enable post-quantum in Rust

```toml
[dependencies]
a1-ai = { version = "2.8", features = ["post-quantum"] }
```

---

## Issue a hybrid passport (Rust)

```rust
use a1::{DyoloIdentity, DyoloPassport, SystemClock};
use a1::hybrid::SignatureAlgorithm;

let root  = DyoloIdentity::generate();
let clock = SystemClock;

// Level 2: ML-DSA-44 + Ed25519
let passport = DyoloPassport::issue_with_algorithm(
    "trading-bot",
    &["trade.equity"],
    30 * 24 * 3600,
    &root,
    &clock,
    SignatureAlgorithm::HybridMlDsa44Ed25519,
)?;

// Level 3: ML-DSA-65 + Ed25519 (government / defense)
let passport = DyoloPassport::issue_with_algorithm(
    "gov-agent",
    &["audit.read"],
    30 * 24 * 3600,
    &root,
    &clock,
    SignatureAlgorithm::HybridMlDsa65Ed25519,
)?;
```

---

## Algorithm negotiation

For deployments where not all agents support post-quantum yet, use `negotiate_algorithm()` to pick the strongest algorithm both sides support:

```rust
use a1::negotiate::negotiate_algorithm;
use a1::hybrid::SignatureAlgorithm;

// Returns the strongest common algorithm
let algorithm = negotiate_algorithm(
    &[SignatureAlgorithm::HybridMlDsa65Ed25519, SignatureAlgorithm::Ed25519],
    &remote_capabilities,
);
```

This allows a gradual rollout: upgrade agents one at a time without breaking cross-agent delegation.

Feature flag: `features = ["negotiate"]`

---

## Migration strategy

A chain can mix algorithm levels monotonically — Ed25519 root → hybrid leaf certs are valid. The reverse (hybrid root → Ed25519 leaf) is rejected. This means you can:

1. Issue new passports with `HybridMlDsa44Ed25519` (root certs upgrade)
2. Issue sub-certs with any algorithm ≤ parent level
3. Existing Ed25519-only chains continue to verify with no changes

No flag day. No simultaneous upgrade requirement.

---

## Wire format

Every cert carries a `SignatureAlgorithm` tag and a `pq_context` field. The `pq_context` is a Blake3 commitment over `(algorithm_id ‖ message ‖ pq_signature_bytes)`. This commitment is verified even when the `post-quantum` feature is disabled — providing cryptographic evidence of declared algorithm intent even to verifiers that cannot evaluate ML-DSA.

---

## Key sizes at a glance

| Algorithm | Public key | Signature | Cert overhead vs Ed25519 |
|---|---|---|---|
| Ed25519 | 32 bytes | 64 bytes | baseline |
| HybridMlDsa44Ed25519 | 1344 bytes | 2484 bytes | +~4 KB per cert |
| HybridMlDsa65Ed25519 | 1984 bytes | 3373 bytes | +~5 KB per cert |

For most deployments (1–5 hop chains) the overhead is negligible. For bandwidth-constrained IoT, use CBOR encoding (`features = ["cbor"]`) to reduce wire size by ~35%.

---

*Source: `src/hybrid.rs`, `src/negotiate.rs` · [Back to wiki home](Home)*
