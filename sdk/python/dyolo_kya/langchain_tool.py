"""
LangChain integration for dyolo-kya.

Wraps any LangChain tool with a dyolo-kya authorization check.
The tool's ``invoke`` method is only called after the delegation chain
has been cryptographically verified against the gateway.

Requires: ``pip install dyolo-kya[langchain]``

Example::

    from langchain_core.tools import tool
    from dyolo_kya.langchain_tool import kya_tool

    @tool
    def execute_trade(symbol: str, qty: int) -> str:
        return f"Bought {qty} shares of {symbol}"

    secured = kya_tool(
        execute_trade,
        chain=CHAIN_JSON,
        executor_pk_hex=AGENT_PUBLIC_KEY,
        intent_name="trade.equity",
        intent_params={"symbol": "AAPL"},
    )
"""

from __future__ import annotations

from typing import Any, Callable

from .client import AsyncKyaClient, KyaClient, IntentSpec


def kya_tool(
    tool: Any,
    chain: Any,
    executor_pk_hex: str,
    intent_name: str,
    intent_params: dict[str, str] | None = None,
    gateway_url: str | None = None,
) -> Any:
    """
    Wrap a LangChain tool with a dyolo-kya authorization gate.

    The original tool's ``invoke`` is only called after the delegation chain
    verifies successfully. A failed authorization raises :class:`KyaError`.

    Args:
        tool:            Any LangChain ``BaseTool`` or ``@tool``-decorated function.
        chain:           The serialized delegation chain (``SignedChain`` dict or JSON).
        executor_pk_hex: Hex-encoded Ed25519 public key of the executing agent.
        intent_name:     The intent action name to verify (e.g. ``"trade.equity"``).
        intent_params:   Optional intent parameter bindings.
        gateway_url:     Gateway base URL (default: ``DYOLO_GATEWAY_URL`` env).

    Returns:
        The same tool object, with ``invoke`` replaced by an authorization-gated version.
    """
    client = KyaClient(gateway_url)
    original_invoke = tool.invoke

    def guarded_invoke(input: Any, **kwargs: Any) -> Any:
        client.authorize(
            chain=chain,
            intent_name=intent_name,
            executor_pk_hex=executor_pk_hex,
            intent_params=intent_params,
        )
        return original_invoke(input, **kwargs)

    tool.invoke = guarded_invoke
    return tool


def async_kya_tool(
    tool: Any,
    chain: Any,
    executor_pk_hex: str,
    intent_name: str,
    intent_params: dict[str, str] | None = None,
    gateway_url: str | None = None,
) -> Any:
    """
    Async version of :func:`kya_tool` for ``ainvoke``-based tools.
    """
    original_ainvoke = tool.ainvoke

    async def guarded_ainvoke(input: Any, **kwargs: Any) -> Any:
        async with AsyncKyaClient(gateway_url) as c:
            await c.authorize(
                chain=chain,
                intent_name=intent_name,
                executor_pk_hex=executor_pk_hex,
                intent_params=intent_params,
            )
        return await original_ainvoke(input, **kwargs)

    tool.ainvoke = guarded_ainvoke
    return tool
