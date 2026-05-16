# Enterprise Key Management — Bring Your Own Vault

By default, A1 accepts any Ed25519 signing key. For production deployments, use a `VaultSigner` to keep root keys inside your organization's KMS or HSM — the key material never leaves the vault boundary.

## Supported backends

| Backend | Class | Requires |
|---------|-------|---------|
| AWS KMS | `AwsKmsSigner` | `boto3` |
| GCP Cloud KMS | `GcpKmsSigner` | `google-cloud-kms` |
| HashiCorp Vault Transit | `VaultTransitSigner` | `hvac` |
| Azure Key Vault | `AzureKeyVaultSigner` | `azure-keyvault-keys`, `azure-identity` |

## AWS KMS

```python
from a1.vault import AwsKmsSigner
from a1.passport import PassportClient

signer = AwsKmsSigner(key_id="arn:aws:kms:us-east-1:123456789012:key/mrk-…")
pc = PassportClient(gateway_url="http://localhost:8080", signer=signer)
passport = await pc.issue_async(namespace="trading-bot", capabilities=["trade.equity"])
```

## GCP Cloud KMS

```python
from a1.vault import GcpKmsSigner

signer = GcpKmsSigner(
    key_name="projects/my-proj/locations/us-central1/keyRings/agents/cryptoKeys/root"
)
```

## HashiCorp Vault Transit

```python
from a1.vault import VaultTransitSigner

signer = VaultTransitSigner(
    vault_url="https://vault.internal:8200",
    token="s.xxxx",          # or use approle / kubernetes auth
    key_name="a1-root",
)
```

## Azure Key Vault

```python
from a1.vault import AzureKeyVaultSigner
from azure.identity import DefaultAzureCredential

signer = AzureKeyVaultSigner(
    vault_url="https://my-vault.vault.azure.net",
    key_name="a1-root",
    credential=DefaultAzureCredential(),
)
```

## Custom signer

Implement the two-method interface to plug in any backend:

```python
from a1.vault import VaultSigner

class MyHsmSigner(VaultSigner):
    def sign(self, message: bytes) -> bytes:
        return my_hsm.sign_ed25519(message)

    def public_key_bytes(self) -> bytes:
        return my_hsm.get_public_key()
```

## Air-gap support

All `VaultSigner` implementations are synchronous-first. They work in fully air-gapped environments as long as the KMS endpoint is reachable from the signing host. The A1 library itself makes no external network calls — only the signer does.
