# Security Model

This document describes the cryptographic and operational security properties of A1 v2.8. It is written for security architects, compliance engineers, and senior engineers evaluating the system for enterprise adoption.

---

## Threat model

A1 protects against the following threat classes:

| Threat | Mitigation |
|---|---|
| Recursive delegation gap (agent exceeds granted scope) | NarrowingMatrix bitwise enforcement at issuance and guard time |
| Forged authorization credentials | Ed25519 signature verification on every cert in the chain |
| Replay of a previously authorized intent | Per-intent nonce consumption (NonceStore) |
| Escalation attack (sub-cert claims more than parent) | SubScopeProof Merkle containment proof required at issuance |
| Compromised agent credential | Revocation by cert fingerprint (RevocationStore) |
| Cross-tenant authorization (tenant isolation bypass) | Namespace binding enforced before any signature verification |
| Timing side-channel in signature verification | `subtle` crate constant-time comparison |
| Private key material leakage | ZeroizeOnDrop on DyoloIdentity; KMS signing patterns eliminate in-process key material |
| Tampered audit receipt | Blake3 commitment in ProvableReceipt; independent verification requires no secrets |
| Chain fingerprint collision | Blake3 over all cert fields; preimage resistance is 256-bit |

---

## Cryptographic primitives

### Ed25519 (signatures)

Every `DelegationCert` is signed with Ed25519 (using `ed25519-dalek` 2.x). Key properties:

- **Security level**: 128-bit (equivalent to RSA-3072)
- **Key size**: 32-byte private scalar, 32-byte verifying key
- **Signature size**: 64 bytes
- **Verification speed**: ~20 µs on modern hardware
- **No weak parameters**: Unlike ECDSA or RSA, Ed25519 has no per-signature randomness requirement. There is no parameter space for a weak parameter attack.
- **Batch verification**: `ed25519-dalek` supports batch verification (used in `authorize_batch`) which is ~2× faster than individual verification for large batches.

### Blake3 (hashing and commitments)

All internal hashing uses Blake3 with domain separation. Specific usages:

| Usage | Domain prefix |
|---|---|
| Capability name → bit position | `dyolo::narrowing::v1` |
| NarrowingMatrix commitment | `dyolo::narrowing::v1` |
| Intent hash | `dyolo::intent::v1` |
| Chain fingerprint | Blake3 over all cert fingerprints |
| SubScopeProof Merkle nodes | `dyolo::merkle::v1` |

Domain separation ensures that an output in one context cannot be repurposed in another, even if the inputs collide.

### subtle (constant-time comparison)

All equality comparisons on sensitive byte arrays (cert fingerprints, nonces, public keys) use `subtle::ConstantTimeEq` to prevent timing side-channel attacks. This is enforced by the workspace dependency pinning to `subtle = { version = "2.5", default-features = false }`.

---

## Authorization flow (step by step)

When `DyoloChain::authorize` is called:

1. **Namespace check** — If a namespace is set on the chain, it must match the context namespace. Mismatch returns `A1Error::NamespaceMismatch` before any cryptographic work.

2. **Revocation check (fast path)** — The principal cert's fingerprint is checked against `RevocationStore`. If revoked, authorization fails immediately.

3. **Chain traversal** — For each cert `C_i` in the chain:
   a. Verify Ed25519 signature: `C_i.signature` over `C_i.signable_bytes()` using `C_{i-1}.delegate_pk` (or `principal_pk` for the first cert).
   b. Verify expiry: `C_i.issued_at ≤ now < C_i.expires_at`.
   c. Verify depth budget: `C_i.max_depth ≥ remaining chain length`.
   d. If `C_i.scope_proof` is present, verify the `SubScopeProof`: the Merkle inclusion proof that `C_i.scope_root` is a subset of `C_{i-1}.scope_root`.
   e. Check revocation: `C_i.fingerprint()` against `RevocationStore`.

4. **Intent authorization** — Verify that `intent_hash` is in the terminal cert's scope via the provided `MerkleProof`.

5. **Nonce consumption** — Call `NonceStore::consume(intent_nonce)`. If the nonce was already consumed (replay), authorization fails.

6. **Audit emission** — Emit `AuditEvent` to all registered `AuditSink` instances.

7. **Receipt production** — Return `AuthorizedAction` containing a `VerificationReceipt`.

For `DyoloPassport::guard`, steps 0 and 6.5 are prepended/appended:

0. **NarrowingMatrix check (O(1))** — `(passport.capability_mask & intent_mask) == intent_mask`. Fails fast before any chain traversal.
6.5. **ProvableReceipt construction** — Wrap the `VerificationReceipt` with the passport namespace and Blake3 commitment over the enforced mask.

---

## NarrowingMatrix — the O(1) enforcement algorithm

The `NarrowingMatrix` is a 256-bit bitmask. Each capability name maps to a `(byte_index, bit_index)` pair:

```
byte_index = blake3(DOMAIN || name)[0] % 32
bit_index  = blake3(DOMAIN || name)[1] % 8
```

The narrowing invariant is:

```
child.mask & parent.mask == child.mask
```

This is equivalent to `child ⊆ parent`. It is computed as eight 64-bit AND operations (the 256-bit mask split into four `u64` words processed in parallel on modern CPUs).

**Collision analysis**: Two distinct capability names may map to the same bit. This is intentional and conservative — if names A and B share a bit, authorizing A also authorizes B from the narrowing check's perspective. This is acceptable because:
1. The `IntentTree` Merkle proof separately verifies the exact intent hash.
2. The narrowing check is an *additional* defense layer, not the sole gate.
3. With 256 bits and typical capability sets of 5–20 names, collision probability is negligible (< 0.1% for 20 names against 256 slots).

**Upgrade path**: The domain prefix `dyolo::narrowing::v1` allows a future `v2` to change the bit assignment algorithm without breaking existing certs.

---

## SubScopeProof — Merkle subset enforcement

When a sub-cert is issued with a subset of the parent's capabilities, it carries a `SubScopeProof`. This is a Merkle inclusion proof proving that each hash in the sub-cert's `IntentTree` is present in the parent cert's `IntentTree`.

The chain verifier checks this proof at step 3d. A sub-cert without a `SubScopeProof` when one is required is rejected. A sub-cert whose proof fails Merkle verification is rejected. There is no way to produce a valid sub-cert with capabilities outside the parent's scope without either:
- Forging an Ed25519 signature (infeasible)
- Finding a Blake3 collision (infeasible)

---

## Nonce replay protection

Every `DelegationCert` carries a random 32-byte nonce. `NonceStore::consume` uses a compare-and-insert operation:

- **MemoryNonceStore**: `HashMap` with an `OnceLock`-based single-writer pattern.
- **a1-pg**: `INSERT INTO nonces (nonce) VALUES ($1) ON CONFLICT DO NOTHING`. The `ON CONFLICT DO NOTHING` ensures exactly-once semantics at the database level, even under concurrent requests.
- **a1-redis**: `SET nonce:$1 1 NX EX $ttl`. The `NX` flag provides atomic compare-and-set.

Nonces are generated via `rand::rngs::OsRng`, providing 256 bits of entropy. Collision probability for a 32-byte nonce is negligible.

---

## Revocation propagation

Revocation is a fingerprint deny-list, not a revocation certificate. This design choice has tradeoffs:

**Advantages**:
- O(1) lookup (hash map or database index).
- No online requirement for issuers — revocation is stored at the verifier, not the issuer.
- Revocation takes effect instantly at the next authorization attempt.

**Limitation**:
- Revocation is not propagated passively. It must be written to the same `RevocationStore` that the verifier reads. For multi-instance deployments, use a shared Redis or Postgres backend.

**Fingerprint**: `blake3(cert.signable_bytes())[0..32]`. Because it is a hash of all cert fields including the signature, fingerprint collision requires forging the signature.

---

## Key material handling

### In-process keys (DyoloIdentity)

`DyoloIdentity` uses `ed25519-dalek::SigningKey` wrapped with `ZeroizeOnDrop` (from the `zeroize` crate). When the struct is dropped, the 32-byte key scalar is overwritten with zeros in memory before the memory is freed. This limits the window for cold-boot or memory-dump attacks.

`DyoloIdentity` does not implement `Clone`. To share across threads, use `SharedIdentity(Arc::new(identity))`, which provides a reference count without duplicating key material.

### KMS-backed keys (VaultSigner)

For production, implement the `Signer` trait over your KMS so the private key never touches application memory:

- **AWS KMS**: The HMAC-KDF pattern (see `AwsKmsSigner`) derives the Ed25519 scalar from a KMS HMAC operation. The scalar is used to sign and immediately dropped.
- **HashiCorp Vault Transit**: Vault signs the payload server-side. The private key never leaves Vault.
- **GCP KMS / Azure Key Vault**: Asymmetric sign operations are performed server-side.

In all cases, `verifying_key_bytes()` returns the 32-byte public key, which is embedded in the cert. At verification time, no KMS call is required.

---

## Authorization without secrets

The `ProvableReceipt` is designed so that an auditor can independently verify it without any secrets:

1. Load the `DyoloPassport` file (public cert, no private key needed).
2. Recompute `NarrowingMatrix::from_hex(receipt.capability_mask_hex).commitment()`.
3. Compare with `receipt.narrowing_commitment`.
4. Check `receipt.inner.chain_fingerprint` against your audit log.

No private key, no KMS access, no network call is required for this verification.

---

## Feature flag security surface

| Feature | Additional dependencies | Notes |
|---|---|---|
| `serde` | `serde`, `ed25519-dalek/serde` | Serialization only |
| `wire` | `serde_json` | JSON encoding of all wire types |
| `async` | `async-trait`, `tokio` | Async trait wrappers |
| `ffi` | None beyond `wire` | C ABI exports; `unsafe` is gated behind this flag |
| `policy-yaml` | `serde_yaml` | YAML parsing only |
| `otel` | `opentelemetry` | Tracing integration |
| `cbor` | `ciborium` | Binary encoding |

The `ffi` feature is the only one that uses `unsafe` code, and only because C ABI export requires it. The unsafe blocks are narrowly scoped to FFI boundary marshaling.

---

## `#![deny(unsafe_code)]`

The core `a1` crate enforces `#![deny(unsafe_code)]` at the crate level. The compiler will reject any unsafe block introduced in the future unless the `ffi` feature and `#[allow(unsafe_code)]` are explicitly present on the specific module.

---

## Responsible disclosure

If you discover a security vulnerability in A1, see [SECURITY.md](../SECURITY.md) for the responsible disclosure policy and contact information.
