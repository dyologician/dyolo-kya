# KMS Integration

A1 signs delegation certificates and passports with Ed25519. The
private root key never needs to live in application memory. This page explains
how to connect each of the four major enterprise KMS providers so that root key
material stays inside your HSM or cloud KMS at all times.

All KMS integrations share the same property: **zero KMS calls at verification
time**. The verifying key is embedded in every certificate, so agents can verify
an entire delegation chain air-gapped, with no network dependency.

---

## Architecture overview

```
            issuance time                    verification time
                  │                                 │
  Human operator  │  A1Context / passport CLI      │  Any agent process
        │         │         │                       │       │
        ▼         │         ▼                       │       ▼
   [KMS API] ─sign─► DelegationCert           cert + VK ─verify─► ✓ / ✗
                  │   (VK embedded)                 │  (no KMS call)
```

---

## AWS KMS

AWS KMS supports Ed25519 asymmetric keys via `ECC_NIST_P256`…but **not
Ed25519 directly**. Use the Python SDK's `AwsKmsSigner` which wraps AWS KMS
P-256 signing and returns a compatible signature, or generate an Ed25519 key
in KMS using a custom key material import.

### Python

```python
from a1.vault import AwsKmsSigner

signer = AwsKmsSigner(
    key_id="alias/a1-passport-root",
    region="us-east-1",
)

# Issue a passport — private key never leaves KMS
passport_bytes = signer.issue_passport_bytes(
    namespace="acme-trading-bot",
    capabilities=["trade.equity", "portfolio.read"],
    ttl_seconds=30 * 24 * 3600,
)
```

### Environment variables

| Variable | Description |
|---|---|
| `AWS_REGION` | AWS region |
| `AWS_ACCESS_KEY_ID` | IAM access key (or use instance profile) |
| `AWS_SECRET_ACCESS_KEY` | IAM secret key (or use instance profile) |
| `A1_KMS_KEY_ID` | KMS key alias or ARN |

### Required IAM policy

```json
{
  "Effect": "Allow",
  "Action": ["kms:Sign", "kms:GetPublicKey"],
  "Resource": "arn:aws:kms:<region>:<account>:key/<key-id>"
}
```

---

## Google Cloud KMS

GCP Cloud KMS supports Ed25519 keys natively via `ASYMMETRIC_SIGN` with
algorithm `EC_SIGN_ED25519`.

### Python

```python
from a1.vault import GcpKmsSigner

signer = GcpKmsSigner(
    project="acme-prod",
    location="us-central1",
    key_ring="a1-kms",
    key="passport-root",
    key_version="1",
)
```

### Environment variables

| Variable | Description |
|---|---|
| `GOOGLE_APPLICATION_CREDENTIALS` | Path to service account JSON |
| `A1_GCP_PROJECT` | GCP project ID |
| `A1_GCP_KEY_RING` | KMS key ring name |
| `A1_GCP_KEY` | KMS key name |
| `A1_GCP_KEY_VERSION` | Key version (usually `"1"`) |

### Required IAM roles

`roles/cloudkms.signerVerifier` on the key resource.

---

## HashiCorp Vault Transit

Vault Transit supports Ed25519 via `key_type=ed25519`.

### Setup

```bash
# Enable the transit secrets engine
vault secrets enable transit

# Create an Ed25519 signing key
vault write transit/keys/a1-passport-root type=ed25519
```

### Python

```python
from a1.vault import HashiCorpVaultSigner

signer = HashiCorpVaultSigner(
    vault_addr="https://vault.corp.example.com",
    key_name="a1-passport-root",
    token=os.environ["VAULT_TOKEN"],
)
```

### Environment variables

| Variable | Description |
|---|---|
| `VAULT_ADDR` | Vault server address |
| `VAULT_TOKEN` | Vault token (or use `VAULT_ROLE_ID`/`VAULT_SECRET_ID`) |
| `A1_VAULT_KEY` | Transit key name |

### Vault policy

```hcl
path "transit/sign/a1-passport-root" {
  capabilities = ["update"]
}
path "transit/keys/a1-passport-root" {
  capabilities = ["read"]
}
```

---

## Azure Key Vault

Azure Key Vault supports Ed25519 via EC keys with curve `Ed25519` (preview as
of 2024; use the `azure-keyvault-keys` SDK ≥ 4.9).

### Python

```python
from a1.vault import AzureKeyVaultSigner
from azure.identity import DefaultAzureCredential

signer = AzureKeyVaultSigner(
    vault_url="https://acme-kv.vault.azure.net/",
    key_name="a1-passport-root",
    credential=DefaultAzureCredential(),
)
```

### Environment variables

| Variable | Description |
|---|---|
| `AZURE_KEYVAULT_URL` | Key Vault endpoint |
| `AZURE_CLIENT_ID` | Service principal client ID |
| `AZURE_CLIENT_SECRET` | Service principal secret |
| `AZURE_TENANT_ID` | Azure AD tenant ID |
| `A1_AZURE_KEY` | Key Vault key name |

### Required role

`Key Vault Crypto User` on the key vault resource.

---

## Implementing a custom `VaultSigner` in Python

Any signing backend implements the `VaultSigner` protocol:

```python
from a1.vault import VaultSigner
from typing import Protocol

class MyHsmSigner(VaultSigner):
    def get_verifying_key_hex(self) -> str:
        # Return the Ed25519 public key as hex
        ...

    def sign(self, payload_bytes: bytes) -> bytes:
        # Return a 64-byte Ed25519 signature over payload_bytes
        ...
```

Pass it directly to `PassportClient` or use it with the gateway's
`/v1/cert/issue` endpoint by serializing certificates server-side.

---

## Implementing a custom `Signer` in Rust

```rust
use a1::Signer;
use ed25519_dalek::{Signature, VerifyingKey};

struct MyHsmSigner {
    public_key: VerifyingKey,
}

impl Signer for MyHsmSigner {
    fn verifying_key(&self) -> VerifyingKey {
        self.public_key
    }

    fn sign_message(&self, msg: &[u8]) -> Signature {
        // Call your HSM SDK here and return the 64-byte signature
        todo!()
    }
}
```

For async KMS SDKs, implement `AsyncSigner` and call
`CertBuilder::sign_async`.

---

## Key rotation

1. Issue a new passport under the new key.
2. Distribute the new passport to all agents.
3. Let all existing sub-certs expire naturally (their TTL is short by design).
4. Retire the old KMS key.

Because verification is purely local, no agent needs a network call to
discover the new key. The verifying key is embedded in every cert.

---

## Air-gap guarantee

Once certs are issued and distributed:

- **Zero KMS calls at verification time.** The `NarrowingMatrix` check is a
  bitwise operation. The signature check uses the embedded verifying key.
- **Works offline.** Agents can verify chains with no network access.
- **No secrets in memory at verification.** Only the verifying key (public) is
  needed.
