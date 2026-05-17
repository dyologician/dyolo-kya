# DID and Verifiable Credentials

Every A1 agent identity maps to a W3C Decentralized Identifier (DID) and can issue W3C Verifiable Credentials (VCs). A1-issued credentials can be verified by any W3C-compliant toolchain without a Rust dependency.

---

## Enable

```toml
[dependencies]
a1-ai = { version = "2.8", features = ["did"] }
```

---

## Agent DIDs

Every Ed25519 public key in A1 maps deterministically to a `did:a1:` identifier. No registry, no network call, no external system required.

### Format

```
did:a1:<hex-encoded-ed25519-verifying-key>
```

### Rust

```rust
use a1::{DyoloIdentity, did::AgentDid};

let identity = DyoloIdentity::generate();
let did = AgentDid::from_key(&identity.verifying_key());

println!("{did}");  // did:a1:4a1b2c3d...

// Resolve back to the verifying key
let vk = did.verifying_key().unwrap();
assert_eq!(vk.as_bytes(), identity.verifying_key().as_bytes());
```

### Gateway

```
GET /v1/did/:pk_hex
```

Returns a W3C DID Document in JSON-LD format.

```json
{
  "@context": ["https://www.w3.org/ns/did/v1"],
  "id": "did:a1:4a1b2c3d...",
  "verificationMethod": [{
    "id": "did:a1:4a1b2c3d...#key-1",
    "type": "Ed25519VerificationKey2020",
    "controller": "did:a1:4a1b2c3d...",
    "publicKeyMultibase": "z6Mk..."
  }],
  "authentication": ["did:a1:4a1b2c3d...#key-1"]
}
```

The gateway's own DID Document is at `GET /v1/did/gateway`.

---

## Verifiable Credentials

### Issue a VC

```rust
use a1::{DyoloIdentity, did::VerifiableCredential};

let issuer = DyoloIdentity::generate();

let vc = VerifiableCredential::issue(
    &issuer,
    "did:a1:4a1b2c3d...",   // subject DID
    serde_json::json!({
        "role": "trader",
        "clearance": "level-2",
        "tradingDesk": "equities"
    }),
)?;

println!("{}", serde_json::to_string_pretty(&vc).unwrap());
```

**Output (W3C VC JSON-LD):**

```json
{
  "@context": ["https://www.w3.org/2018/credentials/v1"],
  "type": ["VerifiableCredential", "A1AgentCredential"],
  "issuer": "did:a1:9f8e7d...",
  "issuanceDate": "2025-05-06T12:00:00Z",
  "credentialSubject": {
    "id": "did:a1:4a1b2c3d...",
    "role": "trader",
    "clearance": "level-2",
    "tradingDesk": "equities"
  },
  "proof": {
    "type": "Ed25519Signature2020",
    "created": "2025-05-06T12:00:00Z",
    "verificationMethod": "did:a1:9f8e7d...#key-1",
    "proofValue": "z..."
  }
}
```

### Verify a VC

```rust
let claims = VerifiableCredential::verify(&vc_json)?;
println!("{:?}", claims);
// {"role": "trader", "clearance": "level-2", ...}
```

---

### Via gateway (any language)

**Issue:**

```bash
curl -X POST http://localhost:8080/v1/vc/issue \
  -H "Authorization: Bearer $A1_ADMIN_SECRET" \
  -H "Content-Type: application/json" \
  -d '{
    "subject_pk_hex": "4a1b2c3d...",
    "claims": { "role": "trader", "clearance": "level-2" }
  }'
```

**Verify:**

```bash
curl -X POST http://localhost:8080/v1/vc/verify \
  -H "Content-Type: application/json" \
  -d '{ "vc": { /* VC JSON-LD */ } }'
```

---

## W3C compatibility

A1 DIDs and VCs are compatible with:

- **W3C DID Core 1.0** — `did:a1:` method resolves offline from the public key alone
- **W3C Verifiable Credentials 1.1** — JSON-LD format with `Ed25519Signature2020`
- **EU eIDAS wallets** — W3C VC format is accepted by eIDAS 2.0 compliant wallets
- **Enterprise IAM platforms** — any system that can resolve a DID Document and verify an Ed25519 signature can consume A1 credentials
- **Blockchains** — submit the DID Document hash to any EVM-compatible chain, IPFS, or Ceramic

---

## Use cases

| Use case | How |
|---|---|
| Cross-org agent identity | Share the DID — no shared secret, no PKI infrastructure |
| Regulated industry credential | Issue a VC attesting the agent's authorization level |
| Blockchain proof | Anchor the VC hash on-chain via `/v1/anchor` |
| EU eIDAS compliance | Issue VCs in W3C format for regulatory audit trail |
| Cross-chain identity | DID resolves from public key — works on any chain |

---

*Source: `src/did.rs`, `a1-gateway/src/routes/did.rs` · [Back to wiki home](Home)*
