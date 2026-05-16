"""
a1 — Python client for the A1 AI agent authorization protocol.

Wraps the a1-gateway REST API. No Rust toolchain required.

Quick start::

    from a1 import A1Client, IntentSpec

    client = A1Client("http://localhost:8080")

    cert = client.issue_cert(
        delegate_pk_hex="...",
        intents=[IntentSpec("trade.equity", {"symbol": "AAPL"})],
        ttl_seconds=3600,
        extensions={
            "dyolo.cost_center": "ai-ops",
            "acme.environment":  "production",
        },
    )
    print(cert.fingerprint_hex)

    result = client.authorize(
        chain=chain_obj,
        intent_name="trade.equity",
        intent_params={"symbol": "AAPL"},
        executor_pk_hex="...",
    )
    print(result.chain_fingerprint)
"""

from __future__ import annotations

import asyncio
import os
import time
from dataclasses import dataclass, field
from typing import Any

import httpx


class A1Error(Exception):
    """Raised when the gateway returns an authorization failure or API error."""

    def __init__(self, message: str, code: str | None = None, status: int | None = None) -> None:
        super().__init__(message)
        self.code   = code
        self.status = status


@dataclass
class IntentSpec:
    """A single intent action with optional parameter bindings."""

    name:   str
    params: dict[str, str] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        d: dict[str, Any] = {"name": self.name}
        if self.params:
            d["params"] = self.params
        return d


@dataclass
class IssuedCert:
    """A delegation certificate returned by the gateway."""

    cert:            dict[str, Any]
    fingerprint_hex: str
    scope_root_hex:  str


@dataclass
class AuthorizeResult:
    """A successful authorization result."""

    authorized:        bool
    chain_depth:       int
    chain_fingerprint: str
    verified_at_unix:  int
    token:             dict[str, Any] | None = None


class A1Client:
    """
    Synchronous client for the A1 gateway.

    For async usage, use :class:`AsyncA1Client`.
    """

    def __init__(
        self,
        base_url: str | None = None,
        timeout: float = 30.0,
        max_retries: int = 3,
        retry_backoff_base: float = 0.5,
    ) -> None:
        self._base_url = (base_url or os.getenv("A1_GATEWAY_URL", "http://localhost:8080")).rstrip("/")
        self._timeout  = timeout
        self._client   = httpx.Client(base_url=self._base_url, timeout=timeout)
        self._max_retries       = max_retries
        self._retry_backoff_base = retry_backoff_base

    def _request(self, method: str, path: str, **kwargs: Any) -> httpx.Response:
        for attempt in range(self._max_retries + 1):
            try:
                resp = self._client.request(method, path, **kwargs)
                if resp.status_code in (429, 502, 503, 504):
                    if attempt < self._max_retries:
                        time.sleep(self._retry_backoff_base * (2.0 ** attempt))
                        continue
                self._raise_for_error(resp)
                return resp
            except httpx.RequestError as e:
                if attempt < self._max_retries:
                    time.sleep(self._retry_backoff_base * (2.0 ** attempt))
                    continue
                raise A1Error(f"Network error: {str(e)}")
        raise A1Error("Max retries exceeded")

    def well_known(self) -> dict[str, Any]:
        """Fetch the gateway's OIDC-style discovery document."""
        resp = self._request("GET", "/.well-known/a1-configuration")
        return resp.json()

    def issue_cert(
        self,
        delegate_pk_hex: str,
        intents: list[IntentSpec],
        ttl_seconds: int = 3600,
        max_depth: int = 16,
        extensions: dict[str, Any] | None = None,
    ) -> IssuedCert:
        """Issue a delegation certificate via the gateway."""
        payload: dict[str, Any] = {
            "delegate_pk_hex": delegate_pk_hex,
            "intents": [i.to_dict() for i in intents],
            "ttl_seconds": ttl_seconds,
            "max_depth": max_depth,
        }
        if extensions:
            payload["extensions"] = extensions
        resp = self._request("POST", "/v1/cert/issue", json=payload)
        data = resp.json()
        return IssuedCert(
            cert=data["cert"],
            fingerprint_hex=data["fingerprint_hex"],
            scope_root_hex=data["scope_root_hex"],
        )

    def authorize(
        self,
        chain: Any,
        intent_name: str,
        executor_pk_hex: str,
        intent_params: dict[str, str] | None = None,
        return_token: bool = False,
    ) -> AuthorizeResult:
        """Verify a delegation chain and authorize an action."""
        payload: dict[str, Any] = {
            "chain": chain,
            "intent_name": intent_name,
            "executor_pk_hex": executor_pk_hex,
            "return_token": return_token,
        }
        if intent_params:
            payload["intent_params"] = intent_params
        resp = self._request("POST", "/v1/authorize", json=payload)
        data = resp.json()
        return AuthorizeResult(
            authorized=data["authorized"],
            chain_depth=data["chain_depth"],
            chain_fingerprint=data["chain_fingerprint"],
            verified_at_unix=data["verified_at_unix"],
            token=data.get("token"),
        )

    def authorize_batch(
        self,
        chain: Any,
        executor_pk_hex: str,
        intents: list[IntentSpec],
    ) -> dict[str, Any]:
        """Authorize multiple intents atomically against a single delegation chain."""
        payload: dict[str, Any] = {
            "chain": chain,
            "executor_pk_hex": executor_pk_hex,
            "intents": [i.to_dict() for i in intents],
        }
        resp = self._request("POST", "/v1/authorize/batch", json=payload)
        return resp.json()

    def revoke(self, fingerprint_hex: str) -> None:
        """Revoke a certificate by its fingerprint."""
        self._request("POST", "/v1/cert/revoke", json={"fingerprint_hex": fingerprint_hex})

    def revoke_batch(self, fingerprints: list[str]) -> dict[str, Any]:
        """Revoke multiple certificates in one round-trip."""
        resp = self._request("POST", "/v1/cert/revoke-batch", json={"fingerprints": fingerprints})
        return resp.json()

    def inspect(self, fingerprint_hex: str) -> dict[str, Any]:
        """Check whether a certificate fingerprint has been revoked."""
        resp = self._request("GET", f"/v1/cert/{fingerprint_hex}")
        return resp.json()

    def verify_token(self, token: dict[str, Any]) -> dict[str, Any]:
        """Verify a VerifiedToken HMAC receipt without re-running the chain."""
        resp = self._request("POST", "/v1/token/verify", json={"token": token})
        return resp.json()

    def health(self) -> dict[str, Any]:
        """Return the gateway health status."""
        resp = self._request("GET", "/health")
        return resp.json()

    # ── DID + Verifiable Credentials ─────────────────────────────────────────

    def resolve_did(self, pk_hex: str) -> dict[str, Any]:
        """Resolve a W3C DID Document from an Ed25519 public key (hex).

        Returns the ``did:a1:{pk_hex}`` DID Document. Derivation is
        deterministic — no registry or network registration required.
        """
        resp = self._request("GET", f"/v1/did/{pk_hex}")
        return resp.json()

    def gateway_did(self) -> dict[str, Any]:
        """Return the W3C DID Document for the gateway's own signing identity."""
        resp = self._request("GET", "/v1/did/gateway")
        return resp.json()

    def issue_vc(
        self,
        subject_pk_hex: str,
        passport_namespace: str,
        capabilities: list[str],
        ttl_seconds: int = 86400,
        chain_fingerprint_hex: str | None = None,
    ) -> dict[str, Any]:
        """Issue a W3C Verifiable Credential asserting an agent's authorized capabilities.

        Signed by the gateway. Any system with the gateway's public key can
        verify the credential offline — no A1 dependency required on the
        verifier side. Requires ``Authorization: Bearer <A1_ADMIN_SECRET>``.
        """
        body: dict[str, Any] = {
            "subject_pk_hex": subject_pk_hex,
            "passport_namespace": passport_namespace,
            "capabilities": capabilities,
            "ttl_seconds": ttl_seconds,
        }
        if chain_fingerprint_hex is not None:
            body["chain_fingerprint_hex"] = chain_fingerprint_hex
        resp = self._request("POST", "/v1/vc/issue", json=body)
        return resp.json()

    def verify_vc(self, credential: dict[str, Any]) -> dict[str, Any]:
        """Verify a W3C Verifiable Credential's Ed25519 signature.

        Returns the decoded subject claims on success. Works for credentials
        issued by any ``did:a1:`` identity, not just this gateway.
        """
        resp = self._request("POST", "/v1/vc/verify", json={"credential": credential})
        return resp.json()

    # ── On-chain anchoring ────────────────────────────────────────────────────

    def anchor_receipt(
        self,
        commitment: dict[str, Any],
        passport_did: str,
        network: str = "ethereum",
    ) -> dict[str, Any]:
        """Prepare on-chain anchor calldata for a ZK chain commitment.

        Returns ABI-encoded EVM calldata (for Ethereum/Polygon/Base/Arbitrum)
        or Solana instruction data. Submit via ethers.js, viem, web3.py, or
        ``a1 anchor <receipt.json> --chain <network>``.

        The returned ``anchored_receipt.anchor_hash_hex`` is the value stored
        on-chain — 32 bytes that permanently prove the authorized action.
        """
        body: dict[str, Any] = {
            "commitment": commitment,
            "passport_did": passport_did,
            "network": network,
        }
        resp = self._request("POST", "/v1/anchor", json=body)
        return resp.json()

    # ── Agent negotiation ─────────────────────────────────────────────────────

    def negotiate_delegation(
        self,
        requester_signing_key_hex: str,
        requested_capabilities: list[str],
        intent_name: str,
        ttl_seconds: int = 3600,
    ) -> dict[str, Any]:
        """Request a delegation certificate from this gateway.

        The gateway issues a scoped ``DelegationCert`` for the requested
        capabilities if they are within the gateway's policy
        (``A1_NEGOTIATE_CAPABILITIES``).

        Returns a ``NegotiationResult`` containing the ready-to-use cert and
        the full ``DelegationOffer`` with the gateway's signature.

        The returned cert can be pushed directly onto a ``DyoloChain``::

            import time, httpx
            from a1 import A1Client

            client = A1Client("http://localhost:8080")
            result = client.negotiate_delegation(
                requester_signing_key_hex=my_sk_hex,
                requested_capabilities=["trade.equity"],
                intent_name="trade.equity",
                ttl_seconds=3600,
            )
            print(result["fingerprint_hex"])
        """
        import time as _time
        import hashlib as _hashlib
        import os as _os

        sk_bytes = bytes.fromhex(requester_signing_key_hex)
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
        sk = Ed25519PrivateKey.from_private_bytes(sk_bytes)
        pk_bytes = sk.public_key().public_bytes_raw()
        pk_hex = pk_bytes.hex()
        requester_did = f"did:a1:{pk_hex}"

        nonce_bytes = _os.urandom(16)
        nonce_hex = nonce_bytes.hex()
        timestamp = int(_time.time())

        import blake3 as _blake3
        h = _blake3.blake3(
            b"a1::dyolo::negotiate::request::v2.8.0",
            derive_key_context=True,
        )
        h.update(len(requester_did).to_bytes(8, "little"))
        h.update(requester_did.encode())
        h.update(nonce_bytes)
        h.update(timestamp.to_bytes(8, "little"))
        h.update(ttl_seconds.to_bytes(8, "little"))
        h.update(len(intent_name).to_bytes(8, "little"))
        h.update(intent_name.encode())
        h.update(len(requested_capabilities).to_bytes(8, "little"))
        for cap in requested_capabilities:
            h.update(len(cap).to_bytes(8, "little"))
            h.update(cap.encode())
        msg = h.digest()

        sig_bytes = sk.sign(msg)
        sig_hex = sig_bytes.hex()

        body = {
            "requester_did": requester_did,
            "requester_pk_hex": pk_hex,
            "requested_capabilities": requested_capabilities,
            "intent_name": intent_name,
            "ttl_secs": ttl_seconds,
            "nonce": nonce_hex,
            "timestamp_unix": timestamp,
            "signature": sig_hex,
        }
        resp = self._request("POST", "/v1/negotiate", json=body)
        return resp.json()

    # ── Passport lifecycle ────────────────────────────────────────────────────

    def issue_passport(
        self,
        namespace: str,
        capabilities: list[str],
        ttl: str = "30d",
        output_path: str | None = None,
    ) -> dict[str, Any]:
        """Issue a DyoloPassport and save it under ``~/.a1/passports/``.

        Parameters
        ----------
        namespace:
            Human-readable agent name, e.g. ``"trading-bot"``.
        capabilities:
            Capability action names the passport will hold, e.g.
            ``["trade.equity", "portfolio.read"]``.
        ttl:
            Lifetime string: ``"30d"``, ``"7d"``, ``"1y"``, or raw seconds.
        output_path:
            Override the default save location. When ``None`` the gateway
            saves to ``~/.a1/passports/<namespace>.json``.
        """
        body: dict[str, Any] = {
            "namespace": namespace,
            "capabilities": capabilities,
            "ttl": ttl,
        }
        if output_path is not None:
            body["output_path"] = output_path
        resp = self._request("POST", "/v1/passports/issue", json=body)
        return resp.json()

    def list_passports(self) -> dict[str, Any]:
        """List all DyoloPassport files under ``~/.a1/passports/``."""
        resp = self._request("GET", "/v1/passports/list")
        return resp.json()

    def read_passport(self, path: str | None = None, namespace: str | None = None) -> dict[str, Any]:
        """Read a passport file's metadata without loading the private key.

        Supply either ``path`` (absolute path) or ``namespace`` to derive the
        default location. At least one of the two must be provided.
        """
        params: dict[str, str] = {}
        if path is not None:
            params["path"] = path
        if namespace is not None:
            params["namespace"] = namespace
        resp = self._request("GET", "/v1/passports/read", params=params)
        return resp.json()

    def renew_passport(
        self,
        path: str | None = None,
        namespace: str | None = None,
        ttl: str = "30d",
    ) -> dict[str, Any]:
        """Re-issue a passport at the same path with a fresh TTL.

        Existing delegation chains signed with the previous passport cert
        become invalid after renewal. Issue new sub-certs as needed.
        """
        body: dict[str, Any] = {"ttl": ttl}
        if path is not None:
            body["path"] = path
        if namespace is not None:
            body["namespace"] = namespace
        resp = self._request("POST", "/v1/passports/renew", json=body)
        return resp.json()

    def revoke_passport(self, namespace: str) -> dict[str, Any]:
        """Revoke all certificates issued under a passport namespace."""
        resp = self._request(
            "POST", "/v1/passports/revoke-by-namespace", json={"namespace": namespace}
        )
        return resp.json()

    def passport_authorize(
        self,
        chain: Any,
        intent_name: str,
        executor_pk_hex: str,
        intent_params: dict[str, str] | None = None,
    ) -> dict[str, Any]:
        """Authorize a passport-scoped delegation chain.

        Returns a :class:`PassportReceipt`-compatible dict containing
        ``passport_namespace``, ``capability_mask_hex``, and
        ``narrowing_commitment_hex`` for offline audit archival.
        """
        payload: dict[str, Any] = {
            "chain": chain,
            "intent_name": intent_name,
            "executor_pk_hex": executor_pk_hex,
            "intent_params": intent_params or {},
        }
        resp = self._request("POST", "/v1/passport/authorize", json=payload)
        return resp.json()

    def _raise_for_error(self, resp: httpx.Response) -> None:
        if resp.is_error:
            try:
                data = resp.json()
                msg  = data.get("error", resp.text)
                code = data.get("code")
            except Exception:
                msg, code = resp.text, None
            raise A1Error(msg, code=code, status=resp.status_code)

    def __enter__(self) -> "A1Client":
        return self

    def __exit__(self, *_: Any) -> None:
        self._client.close()


class AsyncA1Client:
    """
    Async client for the A1 gateway (httpx-based) with feature parity to :class:`A1Client`.
    """

    def __init__(
        self,
        base_url: str | None = None,
        timeout: float = 30.0,
        max_retries: int = 3,
        retry_backoff_base: float = 0.5,
    ) -> None:
        self._base_url = (base_url or os.getenv("A1_GATEWAY_URL", "http://localhost:8080")).rstrip("/")
        self._timeout  = timeout
        self._client   = httpx.AsyncClient(base_url=self._base_url, timeout=timeout)
        self._max_retries       = max_retries
        self._retry_backoff_base = retry_backoff_base

    async def _request(self, method: str, path: str, **kwargs: Any) -> httpx.Response:
        for attempt in range(self._max_retries + 1):
            try:
                resp = await self._client.request(method, path, **kwargs)
                if resp.status_code in (429, 502, 503, 504):
                    if attempt < self._max_retries:
                        await asyncio.sleep(self._retry_backoff_base * (2.0 ** attempt))
                        continue
                self._raise_for_error(resp)
                return resp
            except httpx.RequestError as e:
                if attempt < self._max_retries:
                    await asyncio.sleep(self._retry_backoff_base * (2.0 ** attempt))
                    continue
                raise A1Error(f"Network error: {str(e)}")
        raise A1Error("Max retries exceeded")

    async def well_known(self) -> dict[str, Any]:
        resp = await self._request("GET", "/.well-known/a1-configuration")
        return resp.json()

    async def issue_cert(
        self,
        delegate_pk_hex: str,
        intents: list[IntentSpec],
        ttl_seconds: int = 3600,
        max_depth: int = 16,
        extensions: dict[str, Any] | None = None,
    ) -> IssuedCert:
        payload: dict[str, Any] = {
            "delegate_pk_hex": delegate_pk_hex,
            "intents": [i.to_dict() for i in intents],
            "ttl_seconds": ttl_seconds,
            "max_depth": max_depth,
        }
        if extensions:
            payload["extensions"] = extensions
        resp = await self._request("POST", "/v1/cert/issue", json=payload)
        data = resp.json()
        return IssuedCert(
            cert=data["cert"],
            fingerprint_hex=data["fingerprint_hex"],
            scope_root_hex=data["scope_root_hex"],
        )

    async def authorize(
        self,
        chain: Any,
        intent_name: str,
        executor_pk_hex: str,
        intent_params: dict[str, str] | None = None,
        return_token: bool = False,
    ) -> AuthorizeResult:
        payload: dict[str, Any] = {
            "chain": chain,
            "intent_name": intent_name,
            "executor_pk_hex": executor_pk_hex,
            "return_token": return_token,
        }
        if intent_params:
            payload["intent_params"] = intent_params
        resp = await self._request("POST", "/v1/authorize", json=payload)
        data = resp.json()
        return AuthorizeResult(
            authorized=data["authorized"],
            chain_depth=data["chain_depth"],
            chain_fingerprint=data["chain_fingerprint"],
            verified_at_unix=data["verified_at_unix"],
            token=data.get("token"),
        )

    async def authorize_batch(
        self,
        chain: Any,
        executor_pk_hex: str,
        intents: list[IntentSpec],
    ) -> dict[str, Any]:
        payload: dict[str, Any] = {
            "chain": chain,
            "executor_pk_hex": executor_pk_hex,
            "intents": [i.to_dict() for i in intents],
        }
        resp = await self._request("POST", "/v1/authorize/batch", json=payload)
        return resp.json()

    async def revoke(self, fingerprint_hex: str) -> None:
        await self._request("POST", "/v1/cert/revoke", json={"fingerprint_hex": fingerprint_hex})

    async def revoke_batch(self, fingerprints: list[str]) -> dict[str, Any]:
        resp = await self._request("POST", "/v1/cert/revoke-batch", json={"fingerprints": fingerprints})
        return resp.json()

    async def inspect(self, fingerprint_hex: str) -> dict[str, Any]:
        resp = await self._request("GET", f"/v1/cert/{fingerprint_hex}")
        return resp.json()

    async def verify_token(self, token: dict[str, Any]) -> dict[str, Any]:
        resp = await self._request("POST", "/v1/token/verify", json={"token": token})
        return resp.json()

    async def health(self) -> dict[str, Any]:
        resp = await self._request("GET", "/health")
        return resp.json()

    async def resolve_did(self, pk_hex: str) -> dict[str, Any]:
        """Resolve a W3C DID Document from an Ed25519 public key (hex)."""
        resp = await self._request("GET", f"/v1/did/{pk_hex}")
        return resp.json()

    async def gateway_did(self) -> dict[str, Any]:
        """Return the W3C DID Document for the gateway's own signing identity."""
        resp = await self._request("GET", "/v1/did/gateway")
        return resp.json()

    async def issue_vc(
        self,
        subject_pk_hex: str,
        passport_namespace: str,
        capabilities: list[str],
        ttl_seconds: int = 86400,
        chain_fingerprint_hex: str | None = None,
    ) -> dict[str, Any]:
        """Issue a W3C Verifiable Credential asserting an agent's authorized capabilities."""
        body: dict[str, Any] = {
            "subject_pk_hex": subject_pk_hex,
            "passport_namespace": passport_namespace,
            "capabilities": capabilities,
            "ttl_seconds": ttl_seconds,
        }
        if chain_fingerprint_hex is not None:
            body["chain_fingerprint_hex"] = chain_fingerprint_hex
        resp = await self._request("POST", "/v1/vc/issue", json=body)
        return resp.json()

    async def verify_vc(self, credential: dict[str, Any]) -> dict[str, Any]:
        """Verify a W3C Verifiable Credential's Ed25519 signature."""
        resp = await self._request("POST", "/v1/vc/verify", json={"credential": credential})
        return resp.json()

    async def anchor_receipt(
        self,
        commitment: dict[str, Any],
        passport_did: str,
        network: str = "ethereum",
    ) -> dict[str, Any]:
        """Prepare on-chain anchor calldata for a ZK chain commitment."""
        resp = await self._request(
            "POST", "/v1/anchor",
            json={"commitment": commitment, "passport_did": passport_did, "network": network},
        )
        return resp.json()

    async def negotiate_delegation(
        self,
        requester_signing_key_hex: str,
        requested_capabilities: list[str],
        intent_name: str,
        ttl_seconds: int = 3600,
    ) -> dict[str, Any]:
        """Request a delegation certificate from this gateway.

        Signs a capability request with the caller's Ed25519 private key and
        submits it to ``/v1/negotiate``. Requires the ``cryptography`` and
        ``blake3`` packages (install with ``pip install a1[all]``).
        """
        import os as _os
        import time as _time

        sk_bytes = bytes.fromhex(requester_signing_key_hex)
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
        sk = Ed25519PrivateKey.from_private_bytes(sk_bytes)
        pk_bytes = sk.public_key().public_bytes_raw()
        pk_hex = pk_bytes.hex()
        requester_did = f"did:a1:{pk_hex}"

        nonce_bytes = _os.urandom(16)
        nonce_hex = nonce_bytes.hex()
        timestamp = int(_time.time())

        import blake3 as _blake3
        h = _blake3.blake3(
            b"a1::dyolo::negotiate::request::v2.8.0",
            derive_key_context=True,
        )
        h.update(len(requester_did).to_bytes(8, "little"))
        h.update(requester_did.encode())
        h.update(nonce_bytes)
        h.update(timestamp.to_bytes(8, "little"))
        h.update(ttl_seconds.to_bytes(8, "little"))
        h.update(len(intent_name).to_bytes(8, "little"))
        h.update(intent_name.encode())
        h.update(len(requested_capabilities).to_bytes(8, "little"))
        for cap in requested_capabilities:
            h.update(len(cap).to_bytes(8, "little"))
            h.update(cap.encode())
        msg = h.digest()

        sig_hex = sk.sign(msg).hex()

        body = {
            "requester_did": requester_did,
            "requester_pk_hex": pk_hex,
            "requested_capabilities": requested_capabilities,
            "intent_name": intent_name,
            "ttl_secs": ttl_seconds,
            "nonce": nonce_hex,
            "timestamp_unix": timestamp,
            "signature": sig_hex,
        }
        resp = await self._request("POST", "/v1/negotiate", json=body)
        return resp.json()

    # ── Passport lifecycle ────────────────────────────────────────────────────

    async def issue_passport(
        self,
        namespace: str,
        capabilities: list[str],
        ttl: str = "30d",
        output_path: str | None = None,
    ) -> dict[str, Any]:
        """Issue a DyoloPassport and save it under ``~/.a1/passports/``."""
        body: dict[str, Any] = {
            "namespace": namespace,
            "capabilities": capabilities,
            "ttl": ttl,
        }
        if output_path is not None:
            body["output_path"] = output_path
        resp = await self._request("POST", "/v1/passports/issue", json=body)
        return resp.json()

    async def list_passports(self) -> dict[str, Any]:
        """List all DyoloPassport files under ``~/.a1/passports/``."""
        resp = await self._request("GET", "/v1/passports/list")
        return resp.json()

    async def read_passport(
        self,
        path: str | None = None,
        namespace: str | None = None,
    ) -> dict[str, Any]:
        """Read a passport file's metadata."""
        params: dict[str, str] = {}
        if path is not None:
            params["path"] = path
        if namespace is not None:
            params["namespace"] = namespace
        resp = await self._request("GET", "/v1/passports/read", params=params)
        return resp.json()

    async def renew_passport(
        self,
        path: str | None = None,
        namespace: str | None = None,
        ttl: str = "30d",
    ) -> dict[str, Any]:
        """Re-issue a passport at the same path with a fresh TTL."""
        body: dict[str, Any] = {"ttl": ttl}
        if path is not None:
            body["path"] = path
        if namespace is not None:
            body["namespace"] = namespace
        resp = await self._request("POST", "/v1/passports/renew", json=body)
        return resp.json()

    async def revoke_passport(self, namespace: str) -> dict[str, Any]:
        """Revoke all certificates issued under a passport namespace."""
        resp = await self._request(
            "POST", "/v1/passports/revoke-by-namespace", json={"namespace": namespace}
        )
        return resp.json()

    async def passport_authorize(
        self,
        chain: Any,
        intent_name: str,
        executor_pk_hex: str,
        intent_params: dict[str, str] | None = None,
    ) -> dict[str, Any]:
        """Authorize a passport-scoped delegation chain.

        Returns a receipt dict with ``passport_namespace``,
        ``capability_mask_hex``, and ``narrowing_commitment_hex``.
        """
        payload: dict[str, Any] = {
            "chain": chain,
            "intent_name": intent_name,
            "executor_pk_hex": executor_pk_hex,
            "intent_params": intent_params or {},
        }
        resp = await self._request("POST", "/v1/passport/authorize", json=payload)
        return resp.json()

    def _raise_for_error(self, resp: httpx.Response) -> None:
        if resp.is_error:
            try:
                data = resp.json()
                msg  = data.get("error", resp.text)
                code = data.get("code")
            except Exception:
                msg, code = resp.text, None
            raise A1Error(msg, code=code, status=resp.status_code)

    async def __aenter__(self) -> "AsyncA1Client":
        return self

    async def __aexit__(self, *_: Any) -> None:
        await self._client.aclose()