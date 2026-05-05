"""
Tests for dyolo_kya Python SDK.

Uses respx to mock httpx transport — no live gateway required.
"""
from __future__ import annotations

import json
import pytest
import respx
import httpx

from dyolo_kya.client import (
    KyaClient,
    KyaError,
    IntentSpec,
    IssuedCert,
    AuthorizeResult,
)

GATEWAY = "http://kya-test.local"

MOCK_CHAIN = {
    "version": 1,
    "principal_pk": "aa" * 32,
    "principal_scope": "bb" * 32,
    "certs": [],
}


# ── KyaError ──────────────────────────────────────────────────────────────────

def test_kya_error_carries_code_and_status():
    err = KyaError("chain expired", code="CHAIN_EXPIRED", status=403)
    assert str(err) == "chain expired"
    assert err.code == "CHAIN_EXPIRED"
    assert err.status == 403


def test_kya_error_optional_fields():
    err = KyaError("generic error")
    assert err.code is None
    assert err.status is None


# ── IntentSpec ────────────────────────────────────────────────────────────────

def test_intent_spec_to_dict_no_params():
    spec = IntentSpec("trade.equity")
    d = spec.to_dict()
    assert d == {"name": "trade.equity"}


def test_intent_spec_to_dict_with_params():
    spec = IntentSpec("trade.equity", {"symbol": "AAPL", "qty": "10"})
    d = spec.to_dict()
    assert d["name"] == "trade.equity"
    assert d["params"] == {"symbol": "AAPL", "qty": "10"}


# ── KyaClient — sync ──────────────────────────────────────────────────────────

class TestKyaClientSync:
    def client(self) -> KyaClient:
        return KyaClient(GATEWAY)

    @respx.mock
    def test_health(self):
        respx.get(f"{GATEWAY}/health").mock(
            return_value=httpx.Response(200, json={"status": "ok", "version": "2.0.0"})
        )
        result = self.client().health()
        assert result["status"] == "ok"
        assert result["version"] == "2.0.0"

    @respx.mock
    def test_issue_cert_success(self):
        respx.post(f"{GATEWAY}/v1/cert/issue").mock(
            return_value=httpx.Response(
                200,
                json={
                    "fingerprint_hex": "aabbcc",
                    "scope_root_hex": "ddeeff",
                },
            )
        )
        cert = self.client().issue_cert(
            delegate_pk_hex="cc" * 32,
            intents=[IntentSpec("trade.equity")],
            ttl_seconds=3600,
        )
        assert isinstance(cert, IssuedCert)
        assert cert.fingerprint_hex == "aabbcc"
        assert cert.scope_root_hex == "ddeeff"

    @respx.mock
    def test_authorize_success(self):
        respx.post(f"{GATEWAY}/v1/authorize").mock(
            return_value=httpx.Response(
                200,
                json={
                    "authorized": True,
                    "chain_depth": 1,
                    "chain_fingerprint": "ff" * 32,
                    "verified_at_unix": 1_700_000_000,
                },
            )
        )
        result = self.client().authorize(
            chain=MOCK_CHAIN,
            intent_name="trade.equity",
            executor_pk_hex="dd" * 32,
        )
        assert isinstance(result, AuthorizeResult)
        assert result.authorized is True
        assert result.chain_depth == 1

    @respx.mock
    def test_authorize_raises_kya_error_on_403(self):
        respx.post(f"{GATEWAY}/v1/authorize").mock(
            return_value=httpx.Response(
                403,
                json={"error": "cert revoked", "error_code": "CERT_REVOKED"},
            )
        )
        with pytest.raises(KyaError) as exc_info:
            self.client().authorize(
                chain=MOCK_CHAIN,
                intent_name="trade.equity",
                executor_pk_hex="dd" * 32,
            )
        err = exc_info.value
        assert err.code == "CERT_REVOKED"
        assert err.status == 403

    @respx.mock
    def test_authorize_batch_all_authorized(self):
        respx.post(f"{GATEWAY}/v1/authorize/batch").mock(
            return_value=httpx.Response(
                200,
                json={
                    "all_authorized": True,
                    "authorized_count": 2,
                    "total_count": 2,
                    "results": [
                        {"intent_name": "query.portfolio", "authorized": True},
                        {"intent_name": "trade.equity", "authorized": True},
                    ],
                },
            )
        )
        result = self.client().authorize_batch(
            chain=MOCK_CHAIN,
            executor_pk_hex="dd" * 32,
            intents=[IntentSpec("query.portfolio"), IntentSpec("trade.equity")],
        )
        assert result["all_authorized"] is True
        assert result["authorized_count"] == 2

    @respx.mock
    def test_revoke_success(self):
        respx.post(f"{GATEWAY}/v1/cert/revoke").mock(
            return_value=httpx.Response(200, json={})
        )
        # Should not raise
        self.client().revoke("aabbcc")

    @respx.mock
    def test_revoke_batch_success(self):
        respx.post(f"{GATEWAY}/v1/cert/revoke-batch").mock(
            return_value=httpx.Response(
                200, json={"revoked_count": 2, "failed": []}
            )
        )
        result = self.client().revoke_batch(["aabb", "ccdd"])
        assert result["revoked_count"] == 2
        assert result["failed"] == []

    @respx.mock
    def test_non_json_500_raises_kya_error(self):
        respx.post(f"{GATEWAY}/v1/authorize").mock(
            return_value=httpx.Response(502, text="<html>Bad Gateway</html>")
        )
        with pytest.raises(KyaError) as exc_info:
            self.client().authorize(
                chain=MOCK_CHAIN,
                intent_name="trade.equity",
                executor_pk_hex="dd" * 32,
            )
        assert exc_info.value.status == 502


# ── KyaClient — async ─────────────────────────────────────────────────────────

class TestKyaClientAsync:
    def client(self) -> KyaClient:
        return KyaClient(GATEWAY, async_client=True)

    @pytest.mark.asyncio
    @respx.mock
    async def test_health_async(self):
        respx.get(f"{GATEWAY}/health").mock(
            return_value=httpx.Response(200, json={"status": "ok", "version": "2.0.0"})
        )
        result = await self.client().health()
        assert result["status"] == "ok"

    @pytest.mark.asyncio
    @respx.mock
    async def test_authorize_async_success(self):
        respx.post(f"{GATEWAY}/v1/authorize").mock(
            return_value=httpx.Response(
                200,
                json={
                    "authorized": True,
                    "chain_depth": 2,
                    "chain_fingerprint": "ee" * 32,
                    "verified_at_unix": 1_700_000_001,
                },
            )
        )
        result = await self.client().authorize(
            chain=MOCK_CHAIN,
            intent_name="trade.equity",
            executor_pk_hex="dd" * 32,
        )
        assert result.authorized is True
        assert result.chain_depth == 2

    @pytest.mark.asyncio
    @respx.mock
    async def test_authorize_async_raises_on_403(self):
        respx.post(f"{GATEWAY}/v1/authorize").mock(
            return_value=httpx.Response(
                403,
                json={"error": "scope too narrow", "error_code": "SCOPE_VIOLATION"},
            )
        )
        with pytest.raises(KyaError) as exc_info:
            await self.client().authorize(
                chain=MOCK_CHAIN,
                intent_name="trade.equity",
                executor_pk_hex="dd" * 32,
            )
        assert exc_info.value.code == "SCOPE_VIOLATION"

    @pytest.mark.asyncio
    @respx.mock
    async def test_revoke_batch_async(self):
        respx.post(f"{GATEWAY}/v1/cert/revoke-batch").mock(
            return_value=httpx.Response(
                200, json={"revoked_count": 3, "failed": []}
            )
        )
        result = await self.client().revoke_batch(["aa", "bb", "cc"])
        assert result["revoked_count"] == 3
