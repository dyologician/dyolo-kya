"""
examples/integrations/passport_openai_example.py

OpenAI Agents SDK + a1 passport guard.

Prerequisites
-------------
pip install a1 openai

Set environment variables:
    OPENAI_API_KEY=...
    A1_GATEWAY_URL=http://localhost:8080

Run the a1 gateway first:
    docker run -p 8080:8080 ghcr.io/dyologician/a1-gateway:2.1

Issue a passport first:
    a1 passport issue \
        --namespace acme-trading-bot \
        --allow "trade.equity,portfolio.read" \
        --ttl 30d

Then delegate to this agent:
    a1 passport sub \
        --passport acme-trading-bot-passport.json \
        --key acme-trading-bot-key.hex \
        --delegate <AGENT_PK_HEX> \
        --allow "trade.equity" \
        --ttl 1h
"""
from __future__ import annotations

import json
import os
from typing import Any

from a1.passport import PassportClient, a1_guard, PassportError

# ── Configuration ──────────────────────────────────────────────────────────────

GATEWAY_URL = os.getenv("A1_GATEWAY_URL", "http://localhost:8080")

# These come from your secrets manager / session context in production.
AGENT_PK_HEX: str = os.getenv("AGENT_PK_HEX", "aa" * 32)
SIGNED_CHAIN: dict = json.loads(
    os.getenv(
        "AGENT_SIGNED_CHAIN",
        '{"version":1,"principal_pk":"","principal_scope":"","certs":[]}',
    )
)

client = PassportClient(GATEWAY_URL)

# ── Guarded tool functions ─────────────────────────────────────────────────────

@a1_guard(client=client, capability="trade.equity")
def execute_trade(
    *,
    signed_chain: dict,
    executor_pk_hex: str,
    symbol: str,
    qty: int,
) -> str:
    """Execute an equity trade.  Guarded: requires trade.equity capability."""
    result = {"status": "filled", "symbol": symbol, "qty": qty}
    print(f"[broker] BUY {qty} × {symbol}")
    return json.dumps(result)


@a1_guard(client=client, capability="portfolio.read")
def read_portfolio(
    *,
    signed_chain: dict,
    executor_pk_hex: str,
) -> str:
    """Read portfolio positions.  Guarded: requires portfolio.read capability."""
    positions = [{"symbol": "AAPL", "qty": 100}, {"symbol": "GOOG", "qty": 50}]
    return json.dumps(positions)


# ── OpenAI Agents SDK integration ─────────────────────────────────────────────

def make_function_definitions() -> list[dict]:
    """Return OpenAI-format function definitions for the guarded tools."""
    return [
        {
            "type": "function",
            "function": {
                "name": "execute_trade",
                "description": (
                    "Execute an equity trade on behalf of the authorized principal. "
                    "Requires a valid a1 delegation chain with trade.equity capability."
                ),
                "parameters": {
                    "type": "object",
                    "properties": {
                        "symbol": {
                            "type": "string",
                            "description": "Ticker symbol, e.g. AAPL",
                        },
                        "qty": {
                            "type": "integer",
                            "description": "Number of shares to buy",
                        },
                    },
                    "required": ["symbol", "qty"],
                },
            },
        },
        {
            "type": "function",
            "function": {
                "name": "read_portfolio",
                "description": "Read the current portfolio positions.",
                "parameters": {"type": "object", "properties": {}},
            },
        },
    ]


def dispatch_tool_call(name: str, arguments: dict[str, Any]) -> str:
    """Route an OpenAI tool call to the appropriate guarded function."""
    # The signed_chain and executor_pk_hex are injected here from the agent
    # session context — the LLM never needs to know about them.
    shared = {"signed_chain": SIGNED_CHAIN, "executor_pk_hex": AGENT_PK_HEX}

    try:
        if name == "execute_trade":
            return execute_trade(**shared, **arguments)
        if name == "read_portfolio":
            return read_portfolio(**shared)
        return json.dumps({"error": f"unknown tool: {name}"})
    except PassportError as e:
        return json.dumps({"error": str(e), "error_code": e.error_code})


# ── Demo runner ───────────────────────────────────────────────────────────────

def main() -> None:
    print("a1 passport + OpenAI Agents demo")
    print(f"Gateway: {GATEWAY_URL}")
    print()

    # Simulate what the OpenAI Agents SDK does when it calls a tool:
    print("Simulating execute_trade tool call...")
    result = dispatch_tool_call("execute_trade", {"symbol": "AAPL", "qty": 10})
    print(f"Result: {result}")

    print()
    print("Simulating read_portfolio tool call...")
    result = dispatch_tool_call("read_portfolio", {})
    print(f"Result: {result}")

    print()
    print("Function definitions for OpenAI API:")
    print(json.dumps(make_function_definitions(), indent=2))


if __name__ == "__main__":
    main()