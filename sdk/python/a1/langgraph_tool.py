"""
a1.langgraph_tool — a1 guard for LangGraph agent nodes.

Provides ``a1_node`` — a decorator that wraps a LangGraph node function
with a cryptographic authorization check. The node body only executes if
the full delegation chain carried in the graph state is verified by the
a1 gateway.

Also exports ``A1StateSchema`` — a TypedDict mixin that adds the two
required fields (``signed_chain``, ``executor_pk_hex``) to any LangGraph
state, enabling seamless integration without modifying existing state types.

Requires: ``pip install langgraph langchain-core``

Usage
-----
    from typing import TypedDict
    from langgraph.graph import StateGraph
    from a1.langgraph_tool import a1_node, A1StateSchema
    from a1 import A1Client

    client = A1Client("http://localhost:8080")

    class AgentState(A1StateSchema):
        messages: list
        ticker: str

    @a1_node(intent_name="portfolio.read", client=client)
    async def read_portfolio(state: AgentState) -> AgentState:
        data = await fetch_holdings(state["ticker"])
        return {**state, "messages": state["messages"] + [data]}

    graph = StateGraph(AgentState)
    graph.add_node("read_portfolio", read_portfolio)
"""

from __future__ import annotations

import functools
import inspect
from typing import Any, Callable, Dict, Optional

from .client import A1Client, A1Error

__all__ = [
    "a1_node",
    "A1StateSchema",
]


class A1StateSchema(Dict[str, Any]):
    """
    TypedDict mixin that adds the two fields required by a1 guarded nodes.

    Extend your LangGraph state from this class so every node that uses
    ``@a1_node`` can find ``signed_chain`` and ``executor_pk_hex`` in the
    state dict without any glue code.

    ::

        class AgentState(A1StateSchema):
            messages: list
            ticker: str
    """
    signed_chain: Any
    executor_pk_hex: str


def a1_node(
    *,
    intent_name: str,
    client: A1Client,
    chain_key: str = "signed_chain",
    executor_key: str = "executor_pk_hex",
    propagate_receipt: bool = False,
    receipt_key: str = "a1_receipt",
) -> Callable:
    """
    Decorator that guards a LangGraph node function with an a1
    authorization check.

    The decorated function receives the LangGraph ``state`` dict as its
    sole positional argument. The decorator reads ``chain_key`` and
    ``executor_key`` from the state, calls the gateway, and then invokes
    the original function. If authorization fails, ``A1Error`` is raised,
    which LangGraph surfaces as a node error.

    Parameters
    ----------
    intent_name:
        The capability to enforce, e.g. ``"portfolio.read"``.
    client:
        A ``A1Client`` pointed at the a1 gateway.
    chain_key:
        Key in the state dict that carries the signed delegation chain
        (default: ``"signed_chain"``).
    executor_key:
        Key in the state dict carrying the executor public key hex
        (default: ``"executor_pk_hex"``).
    propagate_receipt:
        When ``True``, the authorization receipt is merged into the
        returned state dict under ``receipt_key``. Useful for building
        an audit trail inside the graph state.
    receipt_key:
        Key under which the receipt is stored when ``propagate_receipt``
        is ``True`` (default: ``"a1_receipt"``).

    Example
    -------
    ::

        @a1_node(intent_name="trade.equity", client=client, propagate_receipt=True)
        async def execute_trade(state: AgentState) -> AgentState:
            await broker.place_order(state["symbol"], state["qty"])
            return state
    """

    def decorator(fn: Callable) -> Callable:
        if inspect.iscoroutinefunction(fn):
            @functools.wraps(fn)
            async def async_wrapper(state: Dict[str, Any]) -> Dict[str, Any]:
                chain = state.get(chain_key)
                executor_pk = state.get(executor_key, "")

                if chain is None:
                    raise A1Error(
                        f"LangGraph state missing '{chain_key}' for intent '{intent_name}'",
                        error_code="MISSING_CHAIN",
                    )

                receipt = await client.authorize_async(
                    chain=chain,
                    intent_name=intent_name,
                    executor_pk_hex=executor_pk,
                )

                result = await fn(state)

                if propagate_receipt and isinstance(result, dict):
                    result = {**result, receipt_key: receipt}

                return result

            return async_wrapper

        @functools.wraps(fn)
        def sync_wrapper(state: Dict[str, Any]) -> Dict[str, Any]:
            chain = state.get(chain_key)
            executor_pk = state.get(executor_key, "")

            if chain is None:
                raise A1Error(
                    f"LangGraph state missing '{chain_key}' for intent '{intent_name}'",
                    error_code="MISSING_CHAIN",
                )

            receipt = client.authorize(
                chain=chain,
                intent_name=intent_name,
                executor_pk_hex=executor_pk,
            )

            result = fn(state)

            if propagate_receipt and isinstance(result, dict):
                result = {**result, receipt_key: receipt}

            return result

        return sync_wrapper

    return decorator


def a1_edge_guard(
    *,
    intent_name: str,
    client: A1Client,
    chain_key: str = "signed_chain",
    executor_key: str = "executor_pk_hex",
    allow_target: str = "authorized",
    deny_target: str = "denied",
) -> Callable[[Dict[str, Any]], str]:
    """
    Build a LangGraph conditional edge function that routes based on
    a1 authorization.

    Use as the condition function in ``graph.add_conditional_edges`` to gate
    transitions on cryptographic chain-of-custody verification.

    Parameters
    ----------
    intent_name:
        The capability to check.
    client:
        A ``A1Client`` pointed at the a1 gateway.
    chain_key:
        Key in the state dict for the signed delegation chain.
    executor_key:
        Key in the state dict for the executor public key hex.
    allow_target:
        Node name to route to when authorization succeeds.
    deny_target:
        Node name to route to when authorization fails.

    Returns
    -------
    Callable
        A function ``(state) -> str`` suitable as a LangGraph edge condition.

    Example
    -------
    ::

        guard = a1_edge_guard(
            intent_name="trade.equity",
            client=client,
        )
        graph.add_conditional_edges("check_auth", guard, {
            "authorized": "execute_trade",
            "denied": "reject",
        })
    """
    def edge_condition(state: Dict[str, Any]) -> str:
        chain = state.get(chain_key)
        executor_pk = state.get(executor_key, "")

        if chain is None:
            return deny_target

        try:
            client.authorize(
                chain=chain,
                intent_name=intent_name,
                executor_pk_hex=executor_pk,
            )
            return allow_target
        except A1Error:
            return deny_target

    return edge_condition