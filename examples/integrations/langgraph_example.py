"""
a1 + LangGraph — stateful agent graph with per-node authorization.

Each graph node that modifies state is guarded by a a1 passport check.
Run: python examples/integrations/langgraph_example.py
"""

from __future__ import annotations

import asyncio
from typing import TypedDict

from a1.langgraph_tool import a1_node


class TradingState(TypedDict):
    symbol: str
    quantity: int
    authorized: bool
    result: str


# 1. Guard individual nodes.

@a1_node(passport_path="passport.json", capability="trade.equity")
async def execute_trade_node(state: TradingState) -> dict:  # type: ignore[type-arg]
    """Execute a trade — only runs after a1 authorization."""
    return {
        **state,
        "authorized": True,
        "result": f"Traded {state['quantity']} shares of {state['symbol']}",
    }


@a1_node(passport_path="passport.json", capability="portfolio.read")
async def read_portfolio_node(state: TradingState) -> dict:  # type: ignore[type-arg]
    """Read portfolio — authorized separately from the trade node."""
    return {**state, "result": "Portfolio: [...]"}


async def main() -> None:
    state: TradingState = {
        "symbol": "ACME",
        "quantity": 100,
        "authorized": False,
        "result": "",
    }

    state = await execute_trade_node(state)  # type: ignore[assignment]
    print(state["result"])

    state = await read_portfolio_node(state)  # type: ignore[assignment]
    print(state["result"])


if __name__ == "__main__":
    asyncio.run(main())
