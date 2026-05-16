"""
OpenAI Assistants integration for a1.

These helpers gate tool execution behind a cryptographic authorization check
without changing the tool's interface or the agent framework's calling convention.

Requires: ``pip install a1[openai]``

OpenAI function-calling example::

    from a1.openai_tool import a1_function_guard

    def execute_trade(symbol: str, qty: int) -> str:
        return f"Bought {qty} shares of {symbol}"

    secured = a1_function_guard(
        execute_trade,
        chain=CHAIN_JSON,
        executor_pk_hex=AGENT_PK,
        intent_name="trade.equity",
    )
    # Pass ``secured`` as the callable behind your OpenAI tool definition.
"""

from __future__ import annotations

import functools
from typing import Any, Callable, TypeVar

from .client import A1Client, A1Error

F = TypeVar("F", bound=Callable[..., Any])


def a1_function_guard(
    fn: F,
    chain: Any,
    executor_pk_hex: str,
    intent_name: str,
    intent_params: dict[str, str] | None = None,
    gateway_url: str | None = None,
) -> F:
    """
    Wrap a plain Python function with a a1 authorization gate.

    Suitable for any framework that calls Python functions based on tool schemas:
    OpenAI function calling, OpenAI Assistants, Anthropic tool use, etc.

    The wrapped function has the same signature as the original. On every call
    the delegation chain is verified against the gateway before the real function
    runs. A failed authorization raises :class:`A1Error` — the framework's
    exception handling will surface this appropriately.
    """
    client = A1Client(gateway_url)

    @functools.wraps(fn)
    def wrapper(*args: Any, **kwargs: Any) -> Any:
        client.authorize(
            chain=chain,
            intent_name=intent_name,
            executor_pk_hex=executor_pk_hex,
            intent_params=intent_params,
        )
        return fn(*args, **kwargs)

    return wrapper  # type: ignore[return-value]
