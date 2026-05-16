"""
a1 + Semantic Kernel — guarded plugin functions.

Every kernel function is authorized against a DyoloPassport before execution.
Run: python examples/integrations/semantic_kernel_example.py
"""

from __future__ import annotations

import asyncio

from a1.semantic_kernel_tool import a1_sk_guard

try:
    from semantic_kernel.functions import kernel_function
except ImportError:
    # Stub for environments without semantic-kernel installed.
    def kernel_function(name: str = "", description: str = ""):  # type: ignore[misc]
        def decorator(fn):  # type: ignore[misc]
            return fn
        return decorator


class TradingPlugin:
    """Example Semantic Kernel plugin for a trading agent."""

    @a1_sk_guard(passport_path="passport.json", capability="trade.equity")
    @kernel_function(name="execute_trade", description="Execute an equity trade order")
    async def execute_trade(self, symbol: str, quantity: int) -> str:
        return f"Traded {quantity} shares of {symbol}"

    @a1_sk_guard(passport_path="passport.json", capability="portfolio.read")
    @kernel_function(name="get_portfolio", description="Return current portfolio positions")
    async def get_portfolio(self) -> str:
        return "Portfolio: ACME x100, BETA x50"


async def main() -> None:
    plugin = TradingPlugin()

    result = await plugin.execute_trade(symbol="ACME", quantity=100)
    print(result)

    portfolio = await plugin.get_portfolio()
    print(portfolio)


if __name__ == "__main__":
    asyncio.run(main())
