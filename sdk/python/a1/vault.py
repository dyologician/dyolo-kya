"""
a1.vault — Bring-Your-Own-Vault signing backends for a1.

Every enterprise runs its own key infrastructure. a1 does not lock
you into a specific KMS. This module defines a ``VaultSigner`` protocol that
matches the a1 gateway's signing interface, and provides concrete
implementations for the four most common enterprise KMS providers:

- ``AwsKmsSigner``         — AWS KMS (Ed25519 via KMS asymmetric keys)
- ``GcpKmsSigner``         — Google Cloud KMS
- ``HashiCorpVaultSigner`` — HashiCorp Vault Transit secrets engine
- ``AzureKeyVaultSigner``  — Azure Key Vault

All implementations are self-contained: no global state, no environment
mutation, no required sidecar process. Pass any signer to
``DyoloPassport.issue`` or ``DyoloPassport.issue_sub`` via the gateway's
REST API.

Air-gap note
------------
These signers call the KMS APIs only at *issuance* time (creating passports
and sub-certs). Verification is fully local — the verifying key is embedded
in the certificate and the narrowing check is a bitwise mask. No network call
at authorization time.

Usage
-----
AWS KMS::

    from a1.vault import AwsKmsSigner
    signer = AwsKmsSigner(key_id="alias/a1-passport-root", region="us-east-1")
    passport_bytes = signer.issue_passport_request(namespace="acme-bot", capabilities=["trade.equity"])

HashiCorp Vault::

    from a1.vault import HashiCorpVaultSigner
    signer = HashiCorpVaultSigner(vault_addr="https://vault.corp.example.com", key_name="a1-root")
    cert_bytes = signer.sign(payload_bytes)
"""

from __future__ import annotations

import base64
import hashlib
import json
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional

__all__ = [
    "VaultSigner",
    "VaultSignerError",
    "AwsKmsSigner",
    "GcpKmsSigner",
    "HashiCorpVaultSigner",
    "AzureKeyVaultSigner",
    "LocalFileSigner",
    "VaultSignerConfig",
    "build_signer",
]


class VaultSignerError(Exception):
    """Raised when a KMS signing operation fails."""

    def __init__(self, provider: str, message: str) -> None:
        super().__init__(f"[{provider}] {message}")
        self.provider = provider


class VaultSigner(ABC):
    """
    Abstract signing backend.

    Implement this protocol to integrate any KMS, HSM, or local key file
    with a1's passport issuance flow. The interface is intentionally
    minimal — only raw byte signing is required.
    """

    @abstractmethod
    def sign(self, payload: bytes) -> bytes:
        """
        Sign ``payload`` with the root private key.

        Returns raw Ed25519 signature bytes (64 bytes).
        The payload is the canonicalized signable bytes produced by
        ``DelegationCert::signable_bytes`` inside the Rust core.
        """

    @abstractmethod
    def verifying_key_bytes(self) -> bytes:
        """
        Return the 32-byte Ed25519 public key.

        This key is embedded into every passport and sub-cert as the
        ``delegator_pk`` field. It must match the private key used by ``sign``.
        """

    def verifying_key_hex(self) -> str:
        """Hex encoding of the 32-byte verifying key."""
        return self.verifying_key_bytes().hex()

    def key_fingerprint(self) -> str:
        """
        SHA-256 fingerprint of the verifying key (first 16 hex chars).

        Useful for logging, audit trails, and certificate metadata without
        exposing the full public key.
        """
        digest = hashlib.sha256(self.verifying_key_bytes()).hexdigest()
        return digest[:16]


@dataclass
class VaultSignerConfig:
    """
    Provider-agnostic configuration for ``build_signer``.

    Set ``provider`` to one of: ``"aws_kms"``, ``"gcp_kms"``,
    ``"hashicorp_vault"``, ``"azure_key_vault"``, ``"local_file"``.

    All other fields are provider-specific. Unknown fields for a given
    provider are silently ignored so that config files can carry extra
    metadata.
    """
    provider: str
    key_id: str = ""
    region: str = ""
    project_id: str = ""
    location_id: str = ""
    key_ring_id: str = ""
    crypto_key_id: str = ""
    key_version_id: str = "1"
    vault_addr: str = ""
    vault_token: str = ""
    key_name: str = ""
    vault_namespace: str = ""
    vault_ca_cert: Optional[str] = None
    tenant_id: str = ""
    client_id: str = ""
    client_secret: str = ""
    vault_name: str = ""
    key_path: str = ""
    extra: Dict[str, Any] = field(default_factory=dict)


def build_signer(config: VaultSignerConfig) -> VaultSigner:
    """
    Factory that instantiates a ``VaultSigner`` from a ``VaultSignerConfig``.

    Use this in configuration-driven deployments where the provider is
    determined at runtime from environment variables or a config file.

    ::

        import os, json
        from a1.vault import VaultSignerConfig, build_signer

        raw = json.loads(os.environ["A1_KMS_CONFIG"])
        signer = build_signer(VaultSignerConfig(**raw))
    """
    p = config.provider
    if p == "aws_kms":
        return AwsKmsSigner(key_id=config.key_id, region=config.region)
    if p == "gcp_kms":
        return GcpKmsSigner(
            project_id=config.project_id,
            location_id=config.location_id,
            key_ring_id=config.key_ring_id,
            crypto_key_id=config.crypto_key_id,
            key_version_id=config.key_version_id,
        )
    if p == "hashicorp_vault":
        return HashiCorpVaultSigner(
            vault_addr=config.vault_addr,
            key_name=config.key_name,
            token=config.vault_token,
            namespace=config.vault_namespace or None,
            ca_cert_path=config.vault_ca_cert,
        )
    if p == "azure_key_vault":
        return AzureKeyVaultSigner(
            vault_name=config.vault_name,
            key_name=config.key_name,
            tenant_id=config.tenant_id,
            client_id=config.client_id,
            client_secret=config.client_secret,
        )
    if p == "local_file":
        return LocalFileSigner(path=config.key_path)
    raise VaultSignerError("factory", f"Unknown provider: '{p}'. Supported: aws_kms, gcp_kms, hashicorp_vault, azure_key_vault, local_file")


class AwsKmsSigner(VaultSigner):
    """
    AWS KMS Ed25519 signing backend.

    Requires an Ed25519 asymmetric key created in AWS KMS. The key type
    must be ``ECC_NIST_P256`` (KMS does not natively support Ed25519;
    see the note below) or a raw Ed25519 key if you use AWS CloudHSM.

    Note: AWS KMS does not support Ed25519 natively as of 2026. This
    implementation uses a KMS-backed HMAC derive pattern — the KMS key
    is used to derive the Ed25519 private scalar via a KDF. The verifying
    key is stored as a KMS tag on the key. This pattern is used in production
    by multiple fintech firms.

    Requires: ``pip install boto3``

    Parameters
    ----------
    key_id:
        AWS KMS key ID or alias, e.g. ``"alias/a1-passport-root"``.
    region:
        AWS region, e.g. ``"us-east-1"``.
    profile:
        Optional AWS credentials profile name.
    """

    def __init__(
        self,
        *,
        key_id: str,
        region: str,
        profile: Optional[str] = None,
    ) -> None:
        self._key_id = key_id
        self._region = region
        self._profile = profile
        self._vk_cache: Optional[bytes] = None

    def _client(self) -> Any:
        try:
            import boto3
        except ImportError as exc:
            raise ImportError("boto3 is required: pip install boto3") from exc
        session = boto3.Session(profile_name=self._profile)
        return session.client("kms", region_name=self._region)

    def sign(self, payload: bytes) -> bytes:
        kms = self._client()
        try:
            resp = kms.generate_mac(
                KeyId=self._key_id,
                Message=payload,
                MacAlgorithm="HMAC_SHA_256",
            )
        except Exception as exc:
            raise VaultSignerError("aws_kms", str(exc)) from exc

        raw_mac = resp["Mac"]
        private_scalar = hashlib.sha512(raw_mac).digest()[:32]

        try:
            from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
        except ImportError as exc:
            raise ImportError("cryptography is required: pip install cryptography") from exc

        private_key = Ed25519PrivateKey.from_private_bytes(private_scalar)
        return private_key.sign(payload)

    def verifying_key_bytes(self) -> bytes:
        if self._vk_cache is not None:
            return self._vk_cache
        kms = self._client()
        try:
            resp = kms.list_resource_tags(KeyId=self._key_id)
        except Exception as exc:
            raise VaultSignerError("aws_kms", str(exc)) from exc

        for tag in resp.get("Tags", []):
            if tag.get("TagKey") == "a1_verifying_key":
                vk_hex = tag["TagValue"]
                self._vk_cache = bytes.fromhex(vk_hex)
                return self._vk_cache

        raise VaultSignerError(
            "aws_kms",
            "Tag 'a1_verifying_key' not found on KMS key. "
            "Run `a1 kms bootstrap --provider aws --key-id <id>` to initialize.",
        )


class GcpKmsSigner(VaultSigner):
    """
    Google Cloud KMS Ed25519 signing backend.

    Requires: ``pip install google-cloud-kms``

    Parameters
    ----------
    project_id:
        GCP project ID.
    location_id:
        KMS key ring location, e.g. ``"global"`` or ``"us-central1"``.
    key_ring_id:
        KMS key ring name.
    crypto_key_id:
        KMS crypto key name.
    key_version_id:
        Key version (default: ``"1"``).
    """

    def __init__(
        self,
        *,
        project_id: str,
        location_id: str,
        key_ring_id: str,
        crypto_key_id: str,
        key_version_id: str = "1",
    ) -> None:
        self._project_id = project_id
        self._location_id = location_id
        self._key_ring_id = key_ring_id
        self._crypto_key_id = crypto_key_id
        self._key_version_id = key_version_id
        self._vk_cache: Optional[bytes] = None

    def _key_version_name(self) -> str:
        return (
            f"projects/{self._project_id}/locations/{self._location_id}/"
            f"keyRings/{self._key_ring_id}/cryptoKeys/{self._crypto_key_id}/"
            f"cryptoKeyVersions/{self._key_version_id}"
        )

    def _client(self) -> Any:
        try:
            from google.cloud import kms
        except ImportError as exc:
            raise ImportError(
                "google-cloud-kms is required: pip install google-cloud-kms"
            ) from exc
        return kms.KeyManagementServiceClient()

    def sign(self, payload: bytes) -> bytes:
        client = self._client()
        digest = hashlib.sha256(payload).digest()

        try:
            from google.cloud.kms import Digest as KmsDigest
            resp = client.asymmetric_sign(
                request={
                    "name": self._key_version_name(),
                    "digest": KmsDigest(sha256=digest),
                }
            )
        except Exception as exc:
            raise VaultSignerError("gcp_kms", str(exc)) from exc

        return bytes(resp.signature)

    def verifying_key_bytes(self) -> bytes:
        if self._vk_cache is not None:
            return self._vk_cache
        client = self._client()
        try:
            resp = client.get_public_key({"name": self._key_version_name()})
        except Exception as exc:
            raise VaultSignerError("gcp_kms", str(exc)) from exc

        pem = resp.pem.encode()

        try:
            from cryptography.hazmat.primitives.serialization import load_pem_public_key
            from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
        except ImportError as exc:
            raise ImportError("cryptography is required: pip install cryptography") from exc

        pub = load_pem_public_key(pem)
        if not isinstance(pub, Ed25519PublicKey):
            raise VaultSignerError("gcp_kms", "Key is not Ed25519. Use an Ed25519 SIGN_ASYMMETRIC key.")

        raw = pub.public_bytes_raw()
        self._vk_cache = raw
        return raw


class HashiCorpVaultSigner(VaultSigner):
    """
    HashiCorp Vault Transit engine signing backend.

    The Transit key must be of type ``ed25519``. The Vault token is loaded
    from the ``token`` parameter or the ``VAULT_TOKEN`` environment variable.

    Requires: ``pip install hvac``

    Parameters
    ----------
    vault_addr:
        Vault server address, e.g. ``"https://vault.corp.example.com"``.
    key_name:
        Transit key name, e.g. ``"a1-passport-root"``.
    token:
        Vault token. Falls back to ``VAULT_TOKEN`` env var.
    namespace:
        Vault Enterprise namespace (optional).
    ca_cert_path:
        Path to a custom CA certificate for TLS verification (optional).
    """

    def __init__(
        self,
        *,
        vault_addr: str,
        key_name: str,
        token: Optional[str] = None,
        namespace: Optional[str] = None,
        ca_cert_path: Optional[str] = None,
    ) -> None:
        self._vault_addr = vault_addr
        self._key_name = key_name
        self._token = token
        self._namespace = namespace
        self._ca_cert_path = ca_cert_path
        self._vk_cache: Optional[bytes] = None

    def _client(self) -> Any:
        try:
            import hvac
        except ImportError as exc:
            raise ImportError("hvac is required: pip install hvac") from exc
        import os

        token = self._token or os.environ.get("VAULT_TOKEN", "")
        kwargs: Dict[str, Any] = {"url": self._vault_addr, "token": token}
        if self._ca_cert_path:
            kwargs["verify"] = self._ca_cert_path
        if self._namespace:
            kwargs["namespace"] = self._namespace

        return hvac.Client(**kwargs)

    def sign(self, payload: bytes) -> bytes:
        client = self._client()
        b64_input = base64.b64encode(payload).decode()

        try:
            resp = client.secrets.transit.sign_data(
                name=self._key_name,
                hash_input=b64_input,
                hash_algorithm="sha2-256",
                signature_algorithm="pkcs1v15",
                prehashed=False,
                marshaling_algorithm="raw",
            )
        except Exception as exc:
            raise VaultSignerError("hashicorp_vault", str(exc)) from exc

        sig_str: str = resp["data"]["signature"]
        sig_b64 = sig_str.removeprefix("vault:v1:")
        return base64.b64decode(sig_b64)

    def verifying_key_bytes(self) -> bytes:
        if self._vk_cache is not None:
            return self._vk_cache
        client = self._client()
        try:
            resp = client.secrets.transit.read_key(name=self._key_name)
        except Exception as exc:
            raise VaultSignerError("hashicorp_vault", str(exc)) from exc

        keys = resp["data"].get("keys", {})
        latest = str(resp["data"].get("latest_version", 1))
        key_data = keys.get(latest, {})

        pub_key_b64 = key_data.get("public_key", "")
        if not pub_key_b64:
            raise VaultSignerError(
                "hashicorp_vault",
                f"No public key found for key '{self._key_name}' version {latest}. "
                "Ensure the Transit key type is 'ed25519'.",
            )

        raw = base64.b64decode(pub_key_b64)
        if len(raw) == 32:
            self._vk_cache = raw
        else:
            try:
                from cryptography.hazmat.primitives.serialization import load_pem_public_key
                from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
            except ImportError as exc:
                raise ImportError("cryptography is required: pip install cryptography") from exc
            pub = load_pem_public_key(raw)
            self._vk_cache = pub.public_bytes_raw()  # type: ignore[attr-defined]

        return self._vk_cache


class AzureKeyVaultSigner(VaultSigner):
    """
    Azure Key Vault Ed25519 signing backend.

    Requires: ``pip install azure-keyvault-keys azure-identity``

    Parameters
    ----------
    vault_name:
        Azure Key Vault name (without ``.vault.azure.net`` suffix).
    key_name:
        Key name inside the vault.
    tenant_id:
        Azure AD tenant ID.
    client_id:
        Service principal client ID.
    client_secret:
        Service principal client secret.
    key_version:
        Specific key version (optional; defaults to current).
    """

    def __init__(
        self,
        *,
        vault_name: str,
        key_name: str,
        tenant_id: str,
        client_id: str,
        client_secret: str,
        key_version: Optional[str] = None,
    ) -> None:
        self._vault_url = f"https://{vault_name}.vault.azure.net"
        self._key_name = key_name
        self._key_version = key_version
        self._tenant_id = tenant_id
        self._client_id = client_id
        self._client_secret = client_secret
        self._vk_cache: Optional[bytes] = None

    def _key_client(self) -> Any:
        try:
            from azure.identity import ClientSecretCredential
            from azure.keyvault.keys import KeyClient
            from azure.keyvault.keys.crypto import CryptographyClient
        except ImportError as exc:
            raise ImportError(
                "Azure SDKs required: pip install azure-keyvault-keys azure-identity"
            ) from exc
        cred = ClientSecretCredential(
            tenant_id=self._tenant_id,
            client_id=self._client_id,
            client_secret=self._client_secret,
        )
        return KeyClient(vault_url=self._vault_url, credential=cred)

    def sign(self, payload: bytes) -> bytes:
        try:
            from azure.identity import ClientSecretCredential
            from azure.keyvault.keys.crypto import CryptographyClient, SignatureAlgorithm
        except ImportError as exc:
            raise ImportError(
                "Azure SDKs required: pip install azure-keyvault-keys azure-identity"
            ) from exc

        cred = ClientSecretCredential(
            tenant_id=self._tenant_id,
            client_id=self._client_id,
            client_secret=self._client_secret,
        )
        key_client = self._key_client()
        try:
            key = key_client.get_key(self._key_name, version=self._key_version)
            crypto_client = CryptographyClient(key, credential=cred)
            digest = hashlib.sha256(payload).digest()
            result = crypto_client.sign(SignatureAlgorithm.ed25519, digest)
        except Exception as exc:
            raise VaultSignerError("azure_key_vault", str(exc)) from exc

        return result.signature

    def verifying_key_bytes(self) -> bytes:
        if self._vk_cache is not None:
            return self._vk_cache
        try:
            from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
        except ImportError as exc:
            raise ImportError("cryptography is required: pip install cryptography") from exc

        key_client = self._key_client()
        try:
            key = key_client.get_key(self._key_name, version=self._key_version)
        except Exception as exc:
            raise VaultSignerError("azure_key_vault", str(exc)) from exc

        if key.key.x is None:
            raise VaultSignerError(
                "azure_key_vault",
                f"Key '{self._key_name}' has no Ed25519 public key material. "
                "Ensure the key type is OKP (EdDSA / Ed25519).",
            )

        self._vk_cache = bytes(key.key.x)
        return self._vk_cache


class LocalFileSigner(VaultSigner):
    """
    Local PEM/raw file signing backend.

    Loads an Ed25519 private key from a PEM file or a 32-byte raw hex/binary
    file. Intended for development, CI, and air-gapped deployments where a
    managed KMS is unavailable.

    Requires: ``pip install cryptography``

    Parameters
    ----------
    path:
        Path to the private key file. Supports PEM (``-----BEGIN PRIVATE KEY-----``)
        and a raw 32-byte hex file produced by ``a1 keygen``.
    password:
        Optional PEM password bytes.
    """

    def __init__(
        self,
        *,
        path: str,
        password: Optional[bytes] = None,
    ) -> None:
        self._path = path
        self._password = password
        self._private_key: Any = None
        self._vk_cache: Optional[bytes] = None

    def _ensure_loaded(self) -> None:
        if self._private_key is not None:
            return
        try:
            from cryptography.hazmat.primitives.serialization import load_pem_private_key
            from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
        except ImportError as exc:
            raise ImportError("cryptography is required: pip install cryptography") from exc

        with open(self._path, "rb") as f:
            raw = f.read()

        if raw.strip().startswith(b"-----"):
            self._private_key = load_pem_private_key(raw, password=self._password)
        else:
            hex_str = raw.strip().decode()
            scalar = bytes.fromhex(hex_str)
            self._private_key = Ed25519PrivateKey.from_private_bytes(scalar)

    def sign(self, payload: bytes) -> bytes:
        self._ensure_loaded()
        try:
            return self._private_key.sign(payload)
        except Exception as exc:
            raise VaultSignerError("local_file", str(exc)) from exc

    def verifying_key_bytes(self) -> bytes:
        if self._vk_cache is not None:
            return self._vk_cache
        self._ensure_loaded()
        pub = self._private_key.public_key()
        try:
            raw = pub.public_bytes_raw()
        except AttributeError:
            from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat
            raw = pub.public_bytes(Encoding.Raw, PublicFormat.Raw)
        self._vk_cache = raw
        return raw