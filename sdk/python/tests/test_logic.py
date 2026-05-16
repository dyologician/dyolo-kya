"""
Pure-logic unit tests for the A1 Python SDK.

These tests exercise data validation, error handling, and client configuration
without requiring a live gateway or any HTTP mocking.
"""
from __future__ import annotations

import pytest

from a1.client import (
    AsyncA1Client,
    AuthorizeResult,
    IntentSpec,
    IssuedCert,
    A1Client,
    A1Error,
)
from a1.passport import PassportClient, PassportError, PassportReceipt


class TestA1Error:
    def test_carries_code_and_status(self):
        err = A1Error("chain expired", code="CHAIN_EXPIRED", status=403)
        assert str(err) == "chain expired"
        assert err.code == "CHAIN_EXPIRED"
        assert err.status == 403

    def test_optional_fields_default_to_none(self):
        err = A1Error("generic error")
        assert err.code is None
        assert err.status is None

    def test_is_exception(self):
        with pytest.raises(A1Error):
            raise A1Error("test")


class TestIntentSpec:
    def test_to_dict_no_params(self):
        spec = IntentSpec("trade.equity")
        assert spec.to_dict() == {"name": "trade.equity"}

    def test_to_dict_with_params(self):
        spec = IntentSpec("trade.equity", {"symbol": "AAPL", "qty": "10"})
        d = spec.to_dict()
        assert d["name"] == "trade.equity"
        assert d["params"] == {"symbol": "AAPL", "qty": "10"}

    def test_to_dict_omits_empty_params(self):
        spec = IntentSpec("read.data", {})
        d = spec.to_dict()
        assert "params" not in d


class TestA1ClientInit:
    def test_default_url_from_env_var(self, monkeypatch):
        monkeypatch.setenv("A1_GATEWAY_URL", "http://my-gateway:9090")
        client = A1Client()
        assert client._base_url == "http://my-gateway:9090"

    def test_explicit_url_overrides_env(self, monkeypatch):
        monkeypatch.setenv("A1_GATEWAY_URL", "http://ignored:9090")
        client = A1Client("http://explicit:8080")
        assert client._base_url == "http://explicit:8080"

    def test_trailing_slash_stripped(self):
        client = A1Client("http://localhost:8080/")
        assert not client._base_url.endswith("/")

    def test_default_timeout(self):
        client = A1Client("http://localhost:8080")
        assert client._timeout > 0

    def test_custom_timeout(self):
        client = A1Client("http://localhost:8080", timeout=5.0)
        assert client._timeout == 5.0


class TestAsyncA1ClientInit:
    def test_url_and_timeout(self):
        client = AsyncA1Client("http://localhost:8080", timeout=10.0)
        assert "localhost:8080" in client._base_url
        assert client._timeout == 10.0


class TestPassportError:
    def test_carries_status(self):
        err = PassportError("not authorized", status=403)
        assert str(err) == "not authorized"
        assert err.status == 403

    def test_optional_status(self):
        err = PassportError("parse failure")
        assert err.status is None


class TestPassportReceipt:
    def test_construction(self):
        r = PassportReceipt(
            passport_namespace="test-agent",
            fingerprint_hex="de" * 32,
            capability_mask_hex="ff" * 32,
            narrowing_commitment_hex="ab" * 32,
            chain_depth=2,
        )
        assert r.passport_namespace == "test-agent"
        assert r.chain_depth == 2
        assert len(r.fingerprint_hex) == 64

    def test_from_dict_nested(self):
        raw = {
            "receipt": {
                "passport_namespace": "bot",
                "fingerprint_hex": "aa" * 32,
                "capability_mask_hex": "bb" * 32,
                "narrowing_commitment_hex": "cc" * 32,
                "chain_depth": 1,
            }
        }
        r = PassportReceipt._from_response(raw)
        assert r.passport_namespace == "bot"

    def test_from_dict_flat(self):
        raw = {
            "passport_namespace": "flat-bot",
            "fingerprint_hex": "aa" * 32,
            "capability_mask_hex": "bb" * 32,
            "narrowing_commitment_hex": "cc" * 32,
            "chain_depth": 3,
        }
        r = PassportReceipt._from_response(raw)
        assert r.passport_namespace == "flat-bot"
        assert r.chain_depth == 3

    def test_from_dict_raises_on_missing_field(self):
        with pytest.raises((KeyError, PassportError)):
            PassportReceipt._from_response({"incomplete": True})


class TestPassportClientInit:
    def test_base_url_stored(self):
        client = PassportClient("http://localhost:8080")
        assert "8080" in client._base_url

    def test_trailing_slash_stripped(self):
        client = PassportClient("http://localhost:8080/")
        assert not client._base_url.endswith("/")