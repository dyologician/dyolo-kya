"""
dyolo_kya — Python client for the dyolo-kya AI agent authorization protocol.

Wraps the dyolo-kya-gateway REST API. No Rust toolchain required.

Quick start::

    from dyolo_kya import KyaClient, IntentSpec

    client = KyaClient("http://localhost:8080")

    # dyolo.* keys are well-known protocol fields (rate limits, quotas, billing tags).
    # Use a reverse-DNS prefix for application-specific metadata, e.g. "acme.env".
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


class KyaError(Exception):
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

    authorized:       bool
    chain_depth:      int
    chain_fingerprint: str
    verified_at_unix: int
    token:            dict[str, Any] | None = None


class KyaClient:
    """
    Synchronous client for the dyolo-kya gateway.

    For async usage, use :class:`AsyncKyaClient`.
    """

    def __init__(
        self,
        base_url: str | None = None,
        timeout: float = 30.0,
        max_retries: int = 3,
        retry_backoff_base: float = 0.5,
    ) -> None:
        self._url = (base_url or os.getenv("DYOLO_GATEWAY_URL", "http://localhost:8080")).rstrip("/")
        self._client = httpx.Client(base_url=self._url, timeout=timeout)
        self._max_retries = max_retries
        self._retry_backoff_base = retry_backoff_base

    def _request(self, method: str, path: str, **kwargs: Any) -> httpx.Response:
        """Native retry loop matching the TS SDK for transient errors."""
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
                raise KyaError(f"Network error: {str(e)}")
        raise KyaError("Max retries exceeded")

    def well_known(self) -> dict[str, Any]:
        """Fetch the gateway's OIDC-style discovery document."""
        resp = self._request("GET", "/.well-known/kya-configuration")
        return resp.json()

    def issue_cert(self, delegate_pk_hex: str, intents: list[IntentSpec], ttl_seconds: int = 3600, max_depth: int = 16, extensions: dict[str, Any] | None = None) -> IssuedCert:
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
        return IssuedCert(cert=data["cert"], fingerprint_hex=data["fingerprint_hex"], scope_root_hex=data["scope_root_hex"])

    def authorize(self, chain: Any, intent_name: str, executor_pk_hex: str, intent_params: dict[str, str] | None = None, return_token: bool = False) -> AuthorizeResult:
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
        return AuthorizeResult(authorized=data["authorized"], chain_depth=data["chain_depth"], chain_fingerprint=data["chain_fingerprint"], verified_at_unix=data["verified_at_unix"], token=data.get("token"))

    def authorize_batch(self, chain: Any, executor_pk_hex: str, intents: list[IntentSpec]) -> dict[str, Any]:
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

    def _raise_for_error(self, resp: httpx.Response) -> None:
        if resp.is_error:
            try:
                data = resp.json()
                msg  = data.get("error", resp.text)
                code = data.get("code")
            except Exception:
                msg, code = resp.text, None
            raise KyaError(msg, code=code, status=resp.status_code)

    def __enter__(self) -> "KyaClient":
        return self

    def __exit__(self, *_: Any) -> None:
        self._client.close()


class AsyncKyaClient:
    """
    Async client for the dyolo-kya gateway (httpx-based) with feature parity to Sync.
    """

    def __init__(
        self,
        base_url: str | None = None,
        timeout: float = 30.0,
        max_retries: int = 3,
        retry_backoff_base: float = 0.5,
    ) -> None:
        self._url = (base_url or os.getenv("DYOLO_GATEWAY_URL", "http://localhost:8080")).rstrip("/")
        self._client = httpx.AsyncClient(base_url=self._url, timeout=timeout)
        self._max_retries = max_retries
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
                raise KyaError(f"Network error: {str(e)}")
        raise KyaError("Max retries exceeded")

    async def well_known(self) -> dict[str, Any]:
        resp = await self._request("GET", "/.well-known/kya-configuration")
        return resp.json()

    async def issue_cert(self, delegate_pk_hex: str, intents: list[IntentSpec], ttl_seconds: int = 3600, max_depth: int = 16, extensions: dict[str, Any] | None = None) -> IssuedCert:
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
        return IssuedCert(cert=data["cert"], fingerprint_hex=data["fingerprint_hex"], scope_root_hex=data["scope_root_hex"])

    async def authorize(self, chain: Any, intent_name: str, executor_pk_hex: str, intent_params: dict[str, str] | None = None, return_token: bool = False) -> AuthorizeResult:
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
        return AuthorizeResult(authorized=data["authorized"], chain_depth=data["chain_depth"], chain_fingerprint=data["chain_fingerprint"], verified_at_unix=data["verified_at_unix"], token=data.get("token"))

    async def authorize_batch(self, chain: Any, executor_pk_hex: str, intents: list[IntentSpec]) -> dict[str, Any]:
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

    def _raise_for_error(self, resp: httpx.Response) -> None:
        if resp.is_error:
            try:
                data = resp.json()
                msg  = data.get("error", resp.text)
                code = data.get("code")
            except Exception:
                msg, code = resp.text, None
            raise KyaError(msg, code=code, status=resp.status_code)

    async def __aenter__(self) -> "AsyncKyaClient":
        return self

    async def __aexit__(self, *_: Any) -> None:
        await self._client.aclose()
