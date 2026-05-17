"""
Tests for a1.passport module.

Uses respx to mock httpx transport — no live gateway required.
"""
from __future__ import annotations

import json
import pytest
import respx
import httpx

from a1.passport import PassportClient, PassportError, PassportReceipt, a1_guard

BASE = "http://localhost:8080"

SAMPLE_RECEIPT = {
    "receipt": {
        "passport_namespace": "test-agent",
        "fingerprint_hex": "deadbeef" * 8,
        "capability_mask_hex": "ff" * 32,
        "narrowing_commitment_hex": "ab" * 32,
        "chain_depth": 1,
    }
}


class TestPassportClientAuthorize:
    @respx.mock
    def test_authorize_success(self):
        respx.post(f"{BASE}/v1/passport/authorize").mock(
            return_value=httpx.Response(200, json=SAMPLE_RECEIPT)
        )
        client = PassportClient(BASE)
        receipt = client.authorize(
            signed_chain={"certs": []},
            intent_name="trade.equity",
            executor_pk_hex="aa" * 32,
        )
        assert receipt.passport_namespace == "test-agent"
        assert receipt.chain_depth == 1
        client.close()

    @respx.mock
    def test_authorize_failure_raises_passport_error(self):
        respx.post(f"{BASE}/v1/passport/authorize").mock(
            return_value=httpx.Response(
                403, json={"error": "scope violation", "error_code": "SCOPE_VIOLATION"}
            )
        )
        client = PassportClient(BASE)
        with pytest.raises(PassportError) as exc_info:
            client.authorize(
                signed_chain={},
                intent_name="trade.equity",
                executor_pk_hex="",
            )
        assert exc_info.value.error_code == "SCOPE_VIOLATION"
        assert exc_info.value.http_status == 403
        client.close()

    @respx.mock
    def test_authorize_flat_receipt_fallback(self):
        flat = {
            "passport_namespace": "bot",
            "fingerprint_hex": "00" * 32,
            "capability_mask_hex": "ff" * 32,
            "narrowing_commitment_hex": "cc" * 32,
            "chain_depth": 2,
        }
        respx.post(f"{BASE}/v1/passport/authorize").mock(
            return_value=httpx.Response(200, json=flat)
        )
        client = PassportClient(BASE)
        receipt = client.authorize(
            signed_chain={}, intent_name="read.data", executor_pk_hex=""
        )
        assert receipt.chain_depth == 2
        client.close()

    @pytest.mark.asyncio
    @respx.mock
    async def test_authorize_async_success(self):
        respx.post(f"{BASE}/v1/passport/authorize").mock(
            return_value=httpx.Response(200, json=SAMPLE_RECEIPT)
        )
        client = PassportClient(BASE)
        receipt = await client.authorize_async(
            signed_chain={},
            intent_name="trade.equity",
            executor_pk_hex="",
        )
        assert receipt.passport_namespace == "test-agent"
        client.close()


class TestDyoloGuardDecorator:
    @respx.mock
    def test_sync_decorator_calls_authorize_then_function(self):
        respx.post(f"{BASE}/v1/passport/authorize").mock(
            return_value=httpx.Response(200, json=SAMPLE_RECEIPT)
        )
        client = PassportClient(BASE)
        call_log: list[str] = []

        @a1_guard(client=client, capability="trade.equity")
        def execute(signed_chain: dict, executor_pk_hex: str = "") -> str:
            call_log.append("executed")
            return "ok"

        result = execute(signed_chain={"certs": []}, executor_pk_hex="aa" * 32)
        assert result == "ok"
        assert call_log == ["executed"]
        client.close()

    @respx.mock
    def test_sync_decorator_blocks_on_auth_failure(self):
        respx.post(f"{BASE}/v1/passport/authorize").mock(
            return_value=httpx.Response(403, json={"error": "denied", "error_code": "DENIED"})
        )
        client = PassportClient(BASE)
        call_log: list[str] = []

        @a1_guard(client=client, capability="trade.equity")
        def execute(signed_chain: dict, executor_pk_hex: str = "") -> str:
            call_log.append("executed")
            return "ok"

        with pytest.raises(PassportError):
            execute(signed_chain={}, executor_pk_hex="")

        assert call_log == [], "function body must not run on auth failure"
        client.close()

    def test_sync_decorator_missing_chain_raises(self):
        client = PassportClient(BASE)

        @a1_guard(client=client, capability="trade.equity")
        def execute(**kwargs) -> str:  # type: ignore[override]
            return "ok"

        with pytest.raises(PassportError) as exc_info:
            execute(executor_pk_hex="aa")
        assert exc_info.value.error_code == "MISSING_CHAIN"
        client.close()

    @pytest.mark.asyncio
    @respx.mock
    async def test_async_decorator_succeeds(self):
        respx.post(f"{BASE}/v1/passport/authorize").mock(
            return_value=httpx.Response(200, json=SAMPLE_RECEIPT)
        )
        client = PassportClient(BASE)

        @a1_guard(client=client, capability="portfolio.read")
        async def read(signed_chain: dict, executor_pk_hex: str = "") -> list:
            return [1, 2, 3]

        result = await read(signed_chain={}, executor_pk_hex="")
        assert result == [1, 2, 3]
        client.close()
