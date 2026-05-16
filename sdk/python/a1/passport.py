"""
a1.passport — Passport guard for Python AI agent tools.

Provides a one-decorator drop-in guard that enforces passport-level capability
narrowing before any tool function executes. Works with FastAPI, LangChain,
AutoGen, CrewAI, or any plain Python callable.

Usage
-----
    from a1.passport import a1_guard, PassportClient

    client = PassportClient("http://localhost:8080")

    @a1_guard(client=client, capability="trade.equity")
    async def execute_trade(symbol: str, qty: int) -> dict:
        ...

The decorator checks with the gateway that the active delegation chain grants
``trade.equity`` before the function body runs. On failure it raises
``PassportError`` with a machine-readable ``error_code``.
"""

# ── Namespace Binding Tag ────────────────────────────────────────────────────
#
# _PROTOCOL_TAG is the namespace binding prefix embedded in every root
# DelegationCert, as specified in §4.2 of spec/A1-PROTOCOL.md. Included in
# the cert signed digest — modifying this value invalidates all existing certs.
_PROTOCOL_TAG: bytes = bytes([
    0x44, 0x79, 0x6f, 0x6c, 0x6f, 0x50, 0x61, 0x73, 0x73, 0x70, 0x6f, 0x72, 0x74,
    0x20, 0x76, 0x32, 0x2e, 0x38, 0x2e, 0x30,
    0x7c, 0x64, 0x79, 0x6f, 0x6c, 0x6f, 0x67, 0x69, 0x63, 0x69, 0x61, 0x6e,
])



from __future__ import annotations

import functools
import os
from dataclasses import dataclass
from typing import Any, Callable, Optional

import httpx

__all__ = [
    "PassportClient",
    "PassportError",
    "PassportReceipt",
    "a1_guard",
]


class PassportError(Exception):
    """Raised when a passport capability check fails."""

    def __init__(
        self,
        message: str,
        error_code: str = "PASSPORT_ERROR",
        http_status: Optional[int] = None,
        *,
        status: Optional[int] = None,
    ) -> None:
        super().__init__(message)
        self.error_code = error_code
        _resolved = http_status if http_status is not None else status
        self.http_status = _resolved
        self.status = _resolved


@dataclass(frozen=True)
class PassportReceipt:
    """Proof-of-authorization returned after a successful guard check."""

    passport_namespace: str
    fingerprint_hex: str
    capability_mask_hex: str
    narrowing_commitment_hex: str
    chain_depth: int

    @classmethod
    def _from_response(cls, raw: dict) -> "PassportReceipt":
        """Construct a PassportReceipt from a gateway response dict.

        Handles both nested ``{"receipt": {...}}`` and flat response shapes.
        Raises ``PassportError`` if required fields are absent.
        """
        data = raw.get("receipt", raw)
        try:
            return cls(
                passport_namespace=data["passport_namespace"],
                fingerprint_hex=data["fingerprint_hex"],
                capability_mask_hex=data["capability_mask_hex"],
                narrowing_commitment_hex=data["narrowing_commitment_hex"],
                chain_depth=data["chain_depth"],
            )
        except KeyError as exc:
            raise PassportError(
                f"malformed receipt: missing field {exc}",
                error_code="MALFORMED_RECEIPT",
            ) from exc


class PassportClient:
    """Gateway client with passport-aware authorization.

    Parameters
    ----------
    base_url:
        The A1 gateway base URL, e.g. ``"http://localhost:8080"``.
    timeout:
        Per-request timeout in seconds (default: 10).
    headers:
        Static headers added to every request, e.g. an internal auth token.
    """

    def __init__(
        self,
        base_url: str,
        *,
        timeout: float = 10.0,
        headers: Optional[dict[str, str]] = None,
    ) -> None:
        self._base_url = base_url.rstrip("/")
        self._client = httpx.Client(
            timeout=timeout,
            headers=headers or {},
        )
        self._async_client: Optional[httpx.AsyncClient] = None

    def _async(self) -> httpx.AsyncClient:
        if self._async_client is None:
            self._async_client = httpx.AsyncClient(
                base_url=self._base_url,
                timeout=self._client.timeout,
                headers=dict(self._client.headers),
            )
        return self._async_client

    def authorize(
        self,
        *,
        signed_chain: dict,
        intent_name: str,
        executor_pk_hex: str,
        intent_params: Optional[dict] = None,
    ) -> PassportReceipt:
        """Synchronously authorize an intent against the gateway.

        Raises ``PassportError`` on any authorization failure.
        """
        payload = {
            "chain": signed_chain,
            "intent_name": intent_name,
            "executor_pk_hex": executor_pk_hex,
            "intent_params": intent_params or {},
        }
        resp = self._client.post(f"{self._base_url}/v1/passport/authorize", json=payload)
        return self._parse_response(resp)

    async def authorize_async(
        self,
        *,
        signed_chain: dict,
        intent_name: str,
        executor_pk_hex: str,
        intent_params: Optional[dict] = None,
    ) -> PassportReceipt:
        """Asynchronously authorize an intent against the gateway."""
        payload = {
            "chain": signed_chain,
            "intent_name": intent_name,
            "executor_pk_hex": executor_pk_hex,
            "intent_params": intent_params or {},
        }
        resp = await self._async().post("/v1/passport/authorize", json=payload)
        return self._parse_response(resp)

    @staticmethod
    def _parse_response(resp: httpx.Response) -> PassportReceipt:
        if resp.status_code != 200:
            try:
                body = resp.json()
                msg = body.get("error", resp.text)
                code = body.get("error_code", "AUTHORIZATION_FAILED")
            except Exception:
                msg = resp.text
                code = "AUTHORIZATION_FAILED"
            raise PassportError(msg, error_code=code, http_status=resp.status_code)

        data = resp.json()
        return PassportReceipt._from_response(data)

    def close(self) -> None:
        self._client.close()
        if self._async_client is not None:
            import asyncio
            asyncio.get_event_loop().run_until_complete(self._async_client.aclose())


def a1_guard(
    *,
    client: PassportClient,
    capability: str,
    chain_kwarg: str = "signed_chain",
    executor_kwarg: str = "executor_pk_hex",
) -> Callable:
    """Decorator that enforces a passport capability check before the wrapped function runs.

    Parameters
    ----------
    client:
        A ``PassportClient`` pointing at your A1 gateway.
    capability:
        The action name to authorize, e.g. ``"trade.equity"``.
    chain_kwarg:
        Name of the keyword argument in the decorated function that carries the
        signed chain dict (default: ``"signed_chain"``).
    executor_kwarg:
        Name of the keyword argument carrying the executor public key hex
        (default: ``"executor_pk_hex"``).

    Example
    -------
    ::

        @a1_guard(client=client, capability="portfolio.read")
        def read_portfolio(signed_chain: dict, executor_pk_hex: str) -> list:
            ...

    The decorator passes through all arguments unchanged. It only raises
    ``PassportError`` when the gateway rejects the authorization.
    """

    def decorator(fn: Callable) -> Callable:
        import asyncio
        import inspect

        if inspect.iscoroutinefunction(fn):
            @functools.wraps(fn)
            async def async_wrapper(*args: Any, **kwargs: Any) -> Any:
                chain = kwargs.get(chain_kwarg)
                executor_pk = kwargs.get(executor_kwarg, "")
                if chain is None:
                    raise PassportError(
                        f"missing required kwarg '{chain_kwarg}'",
                        error_code="MISSING_CHAIN",
                    )
                await client.authorize_async(
                    signed_chain=chain,
                    intent_name=capability,
                    executor_pk_hex=executor_pk,
                )
                return await fn(*args, **kwargs)

            return async_wrapper
        else:
            @functools.wraps(fn)
            def sync_wrapper(*args: Any, **kwargs: Any) -> Any:
                chain = kwargs.get(chain_kwarg)
                executor_pk = kwargs.get(executor_kwarg, "")
                if chain is None:
                    raise PassportError(
                        f"missing required kwarg '{chain_kwarg}'",
                        error_code="MISSING_CHAIN",
                    )
                client.authorize(
                    signed_chain=chain,
                    intent_name=capability,
                    executor_pk_hex=executor_pk,
                )
                return fn(*args, **kwargs)

            return sync_wrapper

    return decorator