# A1 Protocol Specification

**Version:** 2.8.0  
**Status:** Stable  
**Editors:** A1 Protocol Working Group

---

## Abstract

A1 is a cryptographic identity and authorization layer that closes the Recursive Delegation Gap in multi-agent AI systems. It provides AI agents with unforgeable passports, narrowly scoped delegation certificates, and an irrefutable chain of custody from a human principal to any executing agent, verifiable offline with no network calls at authorization time.

This document is the normative specification for the A1 wire format, cryptographic primitives, verification algorithm, and narrowing semantics. Any implementation that conforms to this specification can interoperate with any other conforming implementation.

---

## Table of Contents

1. Introduction
2. Terminology
3. Cryptographic Primitives
4. Core Data Types
5. Delegation Certificate
6. Delegation Chain
7. Passport
8. NarrowingMatrix
9. Verification Algorithm
10. Hybrid Signature Algorithm Framework
11. Wire Format
12. Security Considerations
13. Conformance Requirements
14. Test Vectors

---

## 1. Introduction

### 1.1 The Recursive Delegation Gap

When a human authorizes AI Agent A to complete a task, Agent A may delegate sub-tasks to Agent B, which may further delegate to Agent C. In conventional systems, the authorization chain breaks at each hop — Agent C carries no cryptographic evidence that the original human authorized its specific actions, or under what constraints.

This creates three failure modes:

1. **Scope escalation**: Agent B delegates more authority than it was granted.
2. **Principal anonymity**: An audit trail cannot trace a damaging action back to the authorizing human.
3. **Offline unverifiability**: Verification requires a live call to the issuing authority.

A1 eliminates all three by making each delegation certificate cryptographically self-contained.

### 1.2 Design Goals

- **G1 — Offline verification**: A verifier can authorize an intent from the terminal certificate chain alone, with no network calls, no shared state, and no external registry.
- **G2 — Strict scope narrowing**: It is cryptographically impossible for a delegatee to acquire capabilities beyond those granted by its delegator.
- **G3 — Irrefutable provenance**: Every authorized action produces a receipt that proves the complete principal-to-agent chain.
- **G4 — Quantum resistance migration path**: The wire format accommodates hybrid post-quantum signatures without breaking existing deployments.
- **G5 — Embeddable**: Core verification requires no heap allocation beyond the cert chain itself and runs in <10 µs on commodity hardware.

---

## 2. Terminology

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHOULD", "SHOULD NOT", "RECOMMENDED", and "MAY" apply per RFC 2119.

**Agent** — Any software entity that acts on behalf of a principal and may receive a delegation certificate.

**Principal** — A human or organization that anchors a delegation chain. The principal's public key is the root of trust.

**DelegationCert** — A signed authorization record binding a delegator's public key, a delegatee's public key, and an authorized scope.

**DyoloChain** — An ordered sequence of DelegationCerts rooted at a principal public key.

**DyoloPassport** — A long-lived agent identity document containing a root DelegationCert and a NarrowingMatrix encoding the agent's authorized capabilities.

**Intent** — A 32-byte identifier representing a specific action. The terminal agent must prove its intent falls within the authorized scope.

**NarrowingMatrix** — A 256-bit capability bitmask with a cryptographic commitment. Delegation can only reduce the set of authorized bits.

**ProvableReceipt** — An authorization outcome record containing the chain fingerprint and narrowing commitment, suitable for post-hoc audit.

---

## 3. Cryptographic Primitives

### 3.1 Hash Function

All commitments and domain-separated derivations use **BLAKE3** in keyed mode (`blake3::Hasher::new_derive_key`). The domain string is always ASCII and encodes the protocol namespace, object type, and version as `a1::<type>::<context>::v<N>`.

Implementations MUST NOT substitute SHA-256 or SHA-3 for BLAKE3 in any internally computed commitment. Wire format consumers may verify only the Ed25519 signature, which covers all commitments; however, conforming implementations MUST recompute all internal commitments from the raw data on deserialization.

### 3.2 Signature Scheme

The baseline signature scheme is **Ed25519** as specified in RFC 8032. Messages submitted to the Ed25519 signer are always 32-byte BLAKE3 outputs — never raw data. This protects against long-message attacks and ensures a fixed-size pre-image.

### 3.3 Domain Separation Table

All internal hashes use the following domain strings as the BLAKE3 key derivation input:

| Domain String                  | Usage                                           |
|--------------------------------|-------------------------------------------------|
| `a1::cert::sig::v1`            | Cert signature pre-image                        |
| `a1::cert::fp::v1`             | Cert fingerprint                                |
| `a1::chain::fp::v1`            | Chain fingerprint accumulator                   |
| `a1::intent::leaf::v1`         | Merkle leaf over a single intent                |
| `a1::merkle::node::v1`         | Merkle internal node                            |
| `a1::subscope::commit::v1`     | Sub-scope proof commitment                      |
| `a1::cert::ext::v1`            | Cert extensions commitment                      |
| `a1::namespace::scope::v2`     | Namespace-bound scope derivation                |
| `a1::narrowing::v1`            | NarrowingMatrix commitment                      |
| `a1::hybrid::bind::v1`         | Hybrid signature PQ context binding             |
| `a1::hybrid::algo::v1`         | HybridPublicKey algorithm commitment            |
| `a1::kdf::v1`                  | Subkey derivation                               |

The version suffix in each domain string is frozen. Changes to the hashed content MUST increment the version suffix and MUST NOT be backward-compatible.

### 3.4 Cert Signature Pre-Image

The bytes submitted to Ed25519 for a cert are the 32-byte BLAKE3 output of:

```
H = BLAKE3_derive_key("a1::cert::sig::v1" + version_byte)
H.update(delegator_pk[32])
H.update(delegate_pk[32])
H.update(scope_root[32])
H.update(scope_proof.commitment()[32])
H.update(nonce[16])
H.update(issued_at[8, big-endian])
H.update(expiration_unix[8, big-endian])
H.update(max_depth[1])
H.update(ext_commitment[32])
signable = H.finalize()[32]
```

---

## 4. Core Data Types

### 4.1 IntentHash

A `[u8; 32]` computed as:

```
H = BLAKE3_derive_key("a1::intent::leaf::v1" + version_byte)
H.update(intent_string_bytes)
leaf = H.finalize()[32]
```

### 4.2 SubScopeProof

A compact structure encoding the Merkle proof that a delegated scope root is a valid subset of the delegator's scope. It contains:

- `commitment: [u8; 32]` — Blake3 commitment over the full proof.
- Proof nodes sufficient to reconstruct the parent Merkle root.

The full-passthrough variant (used by root certs) sets `commitment` to the hash of a zero-length proof body.

### 4.3 NarrowingMatrix

A `[u8; 32]` bit field. Bit position `p` is set if capability `name` is authorized, where:

```
byte_offset = BLAKE3(name)[0] mod 32
bit_offset  = BLAKE3(name)[1] mod 8
p = byte_offset * 8 + bit_offset
```

The subset invariant: delegated mask `D` is valid under parent mask `P` iff `(P & D) == D` (bitwise, 32 bytes).

For deployments with more than ~100 distinct capability names, use `CapabilityRegistry` to assign explicit bit positions and avoid birthday-bound collisions.

---

## 5. Delegation Certificate

### 5.1 Fields

| Field             | Type          | Description                                                            |
|-------------------|---------------|------------------------------------------------------------------------|
| `version`         | `u8`          | Cert wire format version. Currently `1`. Values 2+ enable hybrid sigs. |
| `delegator_pk`    | `[u8; 32]`    | Ed25519 public key of the issuing authority.                           |
| `delegate_pk`     | `[u8; 32]`    | Ed25519 public key of the authorized agent.                            |
| `scope_root`      | `[u8; 32]`    | Merkle root of the authorized intent set.                              |
| `scope_proof`     | `SubScopeProof` | Proof that `scope_root` ⊆ parent scope.                             |
| `nonce`           | `[u8; 16]`    | Unique random value, consumed on first use.                            |
| `issued_at`       | `u64`         | Unix timestamp of issuance (seconds).                                  |
| `expiration_unix` | `u64`         | Unix timestamp of expiration (seconds). MUST be > `issued_at`.        |
| `max_depth`       | `u8`          | Maximum remaining re-delegation depth. 0 means terminal.              |
| `extensions`      | `CertExtensions` | Arbitrary key-value metadata. MUST NOT affect authorization logic. |
| `signature`       | `[u8; 64]`    | Ed25519 signature over the pre-image defined in §3.4.                 |

### 5.2 Cert Fingerprint

```
H = BLAKE3_derive_key("a1::cert::fp::v1" + version)
H.update(signature[64])
fingerprint = H.finalize()[32]
```

### 5.3 Validity Constraints

A DelegationCert is structurally valid iff:

1. `version` is a known version byte.
2. `issued_at < expiration_unix`.
3. `signature` verifies under `delegator_pk` against the pre-image from §3.4.
4. `scope_proof.commitment()` matches the declared `scope_root`.

---

## 6. Delegation Chain

### 6.1 Structure

A `DyoloChain` is an ordered list `[C₀, C₁, …, Cₙ]` of `DelegationCert` values, paired with:

- `principal_pk: [u8; 32]` — the root authority public key.
- `principal_scope: [u8; 32]` — the root authorized scope.
- `drift_tolerance_secs: u64` — maximum allowed clock skew (default 15s).
- `namespace: Option<String>` — optional deployment namespace.

### 6.2 Linkage Rule

For each hop `i`:

- `C₀.delegator_pk == principal_pk`
- `Cᵢ.delegator_pk == Cᵢ₋₁.delegate_pk` for all `i > 0`

### 6.3 Chain Fingerprint

The chain fingerprint is an accumulation over all cert fingerprints, the principal key, and the authorized intent:

```
H = BLAKE3_derive_key("a1::chain::fp::v1")
H.update(intent[32])
H.update(principal_pk[32])
for cert in chain:
    H.update(cert.fingerprint()[32])
chain_fingerprint = H.finalize()[32]
```

---

## 7. Passport

A `DyoloPassport` is a JSON document with the following top-level fields:

| Field         | Type               | Description                                                  |
|---------------|--------------------|--------------------------------------------------------------|
| `namespace`   | `String`           | Human-readable agent identity name.                          |
| `cert`        | `DelegationCert`   | Root cert: `delegator_pk == delegate_pk`.                    |
| `capabilities`| `Vec<String>`      | Named capability strings.                                    |
| `mask`        | `NarrowingMatrix`  | Pre-computed bitmask over `capabilities`.                    |
| `issued_at`   | `u64`              | Creation timestamp.                                          |
| `ttl_secs`    | `u64`              | Lifetime of the root cert.                                   |

Passports are saved to storage at issuance and MUST NOT be transmitted over the wire as part of an authorization request. Only delegation certs derived from the passport are transmitted.

---

## 8. NarrowingMatrix

### 8.1 Bit Assignment

For a capability name `s`:

```
hash = BLAKE3(DOMAIN="a1::narrowing::v1", data=s)[32]
byte_offset = hash[0] mod 32
bit_offset  = hash[1] mod 8
```

### 8.2 Commitment

```
H = BLAKE3_derive_key("a1::narrowing::v1")
H.update(mask[32])
commitment = H.finalize()[32]
```

### 8.3 Subset Enforcement

Delegation is valid iff `(parent_mask & child_mask) == child_mask`, evaluated byte-by-byte across all 32 bytes. This check is mandatory at both issuance time and verification time. A chain where any cert's `NarrowingMatrix` is not a subset of its parent's MUST be rejected.

---

## 9. Verification Algorithm

### 9.1 Inputs

- `chain: DyoloChain`
- `agent_pk: [u8; 32]`
- `intent: IntentHash`
- `proof: MerkleProof`
- `clock: u64` (current Unix time)
- `nonce_store: NonceStore`
- `revocation_store: RevocationStore`

### 9.2 Algorithm

```
VERIFY(chain, agent_pk, intent, proof, clock, nonces, revocations):
  assert len(chain.certs) > 0                         // not empty
  assert chain.certs[0].delegator_pk == chain.principal_pk  // root anchors
  assert chain.certs[-1].delegate_pk == agent_pk        // terminal is agent

  parent_scope = chain.principal_scope
  parent_expiry = UINT64_MAX
  prev_pk = chain.principal_pk
  seen_nonces = {}
  fingerprints = []

  for i, cert in enumerate(chain.certs):
    assert cert.delegator_pk == prev_pk               // G2: linkage
    assert cert.version in KNOWN_VERSIONS              // known version
    assert cert.verify_signature()                    // G2: signature
    assert cert.issued_at < cert.expiration_unix      // structural
    assert cert.expiration_unix <= parent_expiry       // G2: temporal narrowing
    assert cert.issued_at <= clock + drift_tolerance  // clock skew
    assert cert.expiration_unix > clock - drift_tolerance  // not expired
    assert cert.max_depth >= (len(chain.certs) - i - 1)   // G2: depth
    assert cert.nonce not in seen_nonces               // replay
    assert cert.nonce not in nonces                   // global replay
    assert cert.fingerprint() not in revocations      // revocation
    assert SCOPE_IS_SUBSET(cert.scope_root, parent_scope, cert.scope_proof)

    seen_nonces.add(cert.nonce)
    fingerprints.append(cert.fingerprint())
    parent_scope = cert.scope_root
    parent_expiry = cert.expiration_unix
    prev_pk = cert.delegate_pk

  assert MERKLE_VERIFY(intent, proof, parent_scope)    // G1: intent in scope

  consume_nonces(seen_nonces, nonces)

  return VerificationReceipt {
    chain_depth: len(chain.certs),
    verified_scope_root: parent_scope,
    intent: intent,
    verified_at_unix: clock,
    chain_fingerprint: CHAIN_FP(intent, chain.principal_pk, fingerprints),
  }
```

### 9.3 Batch Verification

Implementations SHOULD use Ed25519 batch verification when authorizing multiple intents in a single call to reduce the per-cert signature verification cost from O(n) scalar multiplications to O(n/B) where B is the batch size.

---

## 10. Hybrid Signature Algorithm Framework

### 10.1 Motivation

Ed25519 is secure against classical adversaries but will be broken by sufficiently large quantum computers. A1 v2.8.0 introduces a structured migration path that:

1. Does not change the wire format for existing Ed25519 deployments.
2. Adds a versioned `SignatureAlgorithm` tag to certs issued in hybrid mode.
3. Defines the complete `HybridSignature` envelope so that PQ-capable verifiers and classical verifiers can coexist in a single chain.

### 10.2 SignatureAlgorithm Tag

| Value | Name                        | Ed25519 | PQ Algorithm | PQ Public Key | PQ Signature |
|-------|-----------------------------|---------|--------------|---------------|--------------|
| `1`   | `ed25519`                   | Yes     | None         | 0 B           | 0 B          |
| `2`   | `hybrid-ml-dsa-44-ed25519`  | Yes     | ML-DSA-44    | 1312 B        | 2420 B       |
| `3`   | `hybrid-ml-dsa-65-ed25519`  | Yes     | ML-DSA-65    | 1952 B        | 3309 B       |

Values 4–255 are reserved for future algorithms. A verifier that encounters an unknown tag MUST reject with `UnsupportedAlgorithm`.

### 10.3 HybridSignature Envelope

```json
{
  "algorithm": "ed25519",
  "classical_sig": "<hex-encoded 64 bytes>",
  "pq_sig_bytes": "",
  "pq_context": "<hex-encoded 32 bytes>"
}
```

`pq_context` is always verified. It is computed as:

```
H = BLAKE3_derive_key("a1::hybrid::bind::v1")
H.update([algorithm_tag])
H.update(message_len[8, little-endian])
H.update(message[N])
H.update(pq_sig_len[8, little-endian])
H.update(pq_sig_bytes[M])
pq_context = H.finalize()[32]
```

For `algorithm == ed25519`, `pq_sig_len = 0` and `pq_sig_bytes = ""`.

### 10.4 Chain Compatibility

A chain may contain a mix of Ed25519 and hybrid-algorithm certs during the migration window, subject to the monotonicity rule: all classical certs MUST appear before any hybrid cert in the chain order. Reverting from hybrid to classical within a single chain is not permitted.

### 10.5 Algorithm Negotiation

Issuers choose the algorithm to use with:

```
chosen = negotiate_algorithm(supported_algorithms)
```

Without the `post-quantum` feature flag, `negotiate_algorithm` always returns `ed25519` regardless of the input list. With the feature flag, it returns the highest-security algorithm in the input list that the build can verify.

---

## 11. Wire Format

### 11.1 JSON Encoding

The canonical wire format for a `DyoloChain` is JSON with the following structure:

```json
{
  "version": 1,
  "principal_pk": "<hex-32>",
  "principal_scope": "<hex-32>",
  "certs": [
    {
      "version": 1,
      "delegator_pk": "<hex-32>",
      "delegate_pk": "<hex-32>",
      "scope_root": "<hex-32>",
      "scope_proof": { ... },
      "nonce": "<hex-16>",
      "issued_at": 1700000000,
      "expiration_unix": 1700003600,
      "max_depth": 8,
      "extensions": {},
      "signature": "<hex-64>"
    }
  ]
}
```

Ed25519 public keys and signatures are hex-encoded. All integer fields are JSON numbers. All byte arrays are lowercase hexadecimal strings.

### 11.2 CBOR Encoding

When the `cbor` feature is enabled, all types support CBOR serialization. CBOR uses the same field names as JSON. Binary fields are encoded as CBOR byte strings (major type 2) rather than hex strings. This reduces wire size by approximately 40%.

### 11.3 VerifiedToken

For high-throughput paths where re-running chain verification is too expensive, a `VerifiedToken` carries an HMAC-authenticated `VerificationReceipt`:

```json
{
  "receipt": { ... },
  "mac": "<hex-32>"
}
```

`mac` is `BLAKE3(key ‖ canonical_receipt_bytes)` using a 32-byte pre-shared key. The receipt bytes are a canonical serialization of the `VerificationReceipt` fields in a fixed order. HMAC keys MUST be rotated at least every 24 hours.

---

## 12. Security Considerations

### 12.1 Nonce Consumption

Every nonce in a chain MUST be marked consumed in the `NonceStore` before the `VerificationReceipt` is returned to the caller. Implementations MUST NOT return a receipt for any chain containing a nonce that is already marked consumed. The nonce store MUST be durable; an in-memory store is only acceptable for single-process deployments.

### 12.2 Clock Skew

The default `drift_tolerance_secs` of 15 seconds prevents clock skew from causing spurious rejections while bounding replay window extension. For high-security deployments, reduce to 5 seconds. Do not set to zero unless all participants share a synchronized clock.

### 12.3 Revocation

Cert revocation is keyed on cert fingerprint (§5.2). Revocation entries MUST be propagated to all verifiers before the cert's `expiration_unix` elapses. A revocation list that cannot be reached is a denial-of-service against valid agents, not a security bypass — expired certs are rejected regardless.

### 12.4 Capability Hash Collisions

The `NarrowingMatrix` bit assignment is a hash function; with ~100 distinct capability names, the birthday bound becomes non-negligible. Collisions are conservative false positives (capability A authorizes slot X, and so does unrelated capability B). Use `CapabilityRegistry` to assign explicit collision-free bit positions for deployments with more than 100 distinct names.

### 12.5 Algorithm Confusion

A verifier that receives a cert with an unknown `SignatureAlgorithm` tag MUST reject it. Silently downgrading to Ed25519 verification for a cert tagged as a hybrid algorithm would allow a downgrade attack.

### 12.6 Quantum Threat

Ed25519 is broken by Shor's algorithm on a sufficiently large quantum computer (estimated threshold: 2030–2035 for practical attacks, with significant uncertainty). Deployments requiring 10+ year security horizons SHOULD begin issuing root passports with `HybridMlDsa44Ed25519` now. The A1 wire format supports this without breaking existing classical-only verifiers within the `MixedClassicalToHybrid` compatibility mode.

---

## 13. Conformance Requirements

A conforming A1 implementation MUST:

- **C1**: Implement all verification steps in §9.2 in the stated order.
- **C2**: Reject certs with unknown `version` bytes.
- **C3**: Reject certs where `issued_at >= expiration_unix`.
- **C4**: Verify the Ed25519 signature using the pre-image from §3.4.
- **C5**: Enforce the subset invariant `(parent_mask & child_mask) == child_mask` for every hop.
- **C6**: Mark nonces consumed atomically with receipt issuance.
- **C7**: Reject certs whose fingerprint appears in the revocation store.
- **C8**: Reject `HybridSignature` payloads where `pq_context` does not verify.
- **C9**: Reject certs tagged with an unknown `SignatureAlgorithm` byte.
- **C10**: Never fall back to a weaker algorithm on verification failure.

A conforming implementation SHOULD:

- **S1**: Use Ed25519 batch verification for multi-cert chains.
- **S2**: Use a durable nonce store in production deployments.
- **S3**: Export audit events to a SIEM for all authorization decisions.

---

## 14. Test Vectors

All test vectors are expressed as hex-encoded byte strings. They are authoritative for checking interoperability of new implementations.

### 14.1 Intent Leaf Hash

**Input**: `"trade.equity"` (UTF-8, no terminator)  
**Domain**: `a1::intent::leaf::v1` (+ version byte `0x01`)  
**Output**: computed by the reference implementation test suite at `tests/integration.rs`.

### 14.2 NarrowingMatrix Bit Assignment

**Capability**: `"trade.equity"`  
**Expected bit position**: computed deterministically from BLAKE3. Verified by `NarrowingMatrix::from_capabilities(&["trade.equity"]).commitment()`.

### 14.3 Conformance Test Suite

The machine-readable conformance test suite is published at `tests/` in the reference implementation. It covers:

- Empty chain rejection.
- Root mismatch rejection.
- Signature forgery rejection.
- Scope escalation rejection.
- Expired cert rejection.
- Nonce replay rejection.
- Revoked cert rejection.
- Max depth exceeded rejection.
- Algorithm mismatch rejection.
- PQ context tampering rejection.
- Roundtrip JSON serialization and deserialization.

Any A1 implementation MUST pass all tests in the conformance suite to be considered conformant.

---

*A1 Protocol Specification v2.8.0 — One Identity. Full Provenance.*
