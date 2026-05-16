"""
sdk/python/tests/passport_langchain_example.py

End-to-end example: protect a LangChain tool with a1 passport guards.

Prerequisites
-------------
pip install a1[langchain] langchain-openai

Set environment variables:
    OPENAI_API_KEY=...
    A1_GATEWAY_URL=http://localhost:8080   (or your gateway address)

Run the a1 gateway first:
    docker run -p 8080:8080 ghcr.io/dyologician/a1-gateway:2

This example runs entirely locally — no live trades, no real keys.
"""
from __future__ import annotations

import json
import os
from typing import Any

# ── a1 imports ─────────────────────────────────────────────────────────

from a1.passport import PassportClient, a1_guard, PassportError

# ── Configuration ──────────────────────────────────────────────────────────────

GATEWAY_URL: str = os.getenv("A1_GATEWAY_URL", "http://localhost:8080")

# In a real deployment these come from your secrets manager or HSM:
#   AGENT_PK_HEX  — the sub-agent's Ed25519 public key (hex)
#   SIGNED_CHAIN  — the JSON delegation chain produced by `a1 passport sub`
AGENT_PK_HEX: str = os.getenv("AGENT_PK_HEX", "aa" * 32)
SIGNED_CHAIN: dict = json.loads(os.getenv("AGENT_SIGNED_CHAIN", '{"version":1,"principal_pk":"","principal_scope":"","certs":[]}'))

# ── Passport client ───────────────────────────────────────────────────────────

client = PassportClient(GATEWAY_URL)

# ── Guarded tools ─────────────────────────────────────────────────────────────

@a1_guard(client=client, capability="trade.equity")
def execute_trade(
    *,
    signed_chain: dict,
    executor_pk_hex: str,
    symbol: str,
    qty: int,
) -> dict:
    """Execute an equity trade.  Guarded by the a1 passport layer."""
    print(f"[broker] BUY {qty} × {symbol}  (authorized)")
    return {"status": "filled", "symbol": symbol, "qty": qty}


@a1_guard(client=client, capability="portfolio.read")
def read_portfolio(
    *,
    signed_chain: dict,
    executor_pk_hex: str,
) -> list[dict]:
    """Return current portfolio positions.  Guarded by the a1 passport layer."""
    return [
        {"symbol": "AAPL", "qty": 100},
        {"symbol": "GOOG", "qty": 50},
    ]


# ── LangChain integration ────────────────────────────────────────────────────

def make_langchain_tools() -> list:
    """
    Wrap the guarded tools as LangChain Tool objects.

    The signed chain and executor public key are injected into every tool call
    via a partial wrapper so the LLM never needs to know about them.
    """
    try:
        from langchain.tools import Tool
        import functools

        def _bind(fn: Any, **fixed: Any):
            @functools.wraps(fn)
            def wrapper(**kwargs: Any) -> Any:
                return fn(**{**fixed, **kwargs})
            return wrapper

        bound_trade = _bind(
            execute_trade,
            signed_chain=SIGNED_CHAIN,
            executor_pk_hex=AGENT_PK_HEX,
        )
        bound_portfolio = _bind(
            read_portfolio,
            signed_chain=SIGNED_CHAIN,
            executor_pk_hex=AGENT_PK_HEX,
        )

        return [
            Tool(
                name="execute_trade",
                func=lambda inp: json.dumps(bound_trade(**json.loads(inp))),
                description=(
                    "Execute an equity trade. "
                    "Input JSON: {\"symbol\": \"AAPL\", \"qty\": 10}"
                ),
            ),
            Tool(
                name="read_portfolio",
                func=lambda _: json.dumps(bound_portfolio()),
                description="Read current portfolio positions. No input required.",
            ),
        ]
    except ImportError:
        print("langchain not installed — run: pip install a1[langchain]")
        return []


# ── Main demo ─────────────────────────────────────────────────────────────────

def main() -> None:
    print("a1 passport + LangChain demo")
    print(f"Gateway: {GATEWAY_URL}")
    print()

    # Demonstrate the guard directly (without a running gateway — for unit demos):
    print("Simulating guarded tool call (will fail without a running gateway)...")
    try:
        result = execute_trade(
            signed_chain=SIGNED_CHAIN,
            executor_pk_hex=AGENT_PK_HEX,
            symbol="AAPL",
            qty=10,
        )
        print(f"Trade result: {result}")
    except PassportError as e:
        print(f"Auth failed (expected without live gateway): {e.error_code}")

    print()
    print("LangChain tools:")
    for tool in make_langchain_tools():
        print(f"  - {tool.name}: {tool.description[:60]}...")


if __name__ == "__main__":
    main()