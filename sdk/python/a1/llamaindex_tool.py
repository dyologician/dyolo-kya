"""
a1.llamaindex_tool — a1 guard for LlamaIndex agent tools.

Wraps any LlamaIndex ``FunctionTool`` with a cryptographic authorization check
so that no tool executes unless the full delegation chain is verified first.

Requires: ``pip install llama-index-core``

Usage
-----
    from llama_index.core.tools import FunctionTool
    from a1.llamaindex_tool import a1_llamaindex_tool
    from a1 import A1Client

    client = A1Client("http://localhost:8080")

    def read_portfolio(ticker: str) -> dict:
        return {"ticker": ticker, "holdings": 100}

    tool = a1_llamaindex_tool(
        fn=read_portfolio,
        intent_name="portfolio.read",
        client=client,
        resolve_context=lambda kwargs: {
            "chain": agent_chain,
            "executor_pk_hex": agent_pk,
        },
        name="read_portfolio",
        description="Read portfolio holdings for a given ticker.",
    )
"""

from __future__ import annotations

import functools
from typing import Any, Callable, Dict, Optional

from .client import A1Client, A1Error


__all__ = ["a1_llamaindex_tool", "a1_llamaindex_guard"]


def a1_llamaindex_tool(
    fn: Callable[..., Any],
    *,
    intent_name: str,
    client: A1Client,
    resolve_context: Callable[[Dict[str, Any]], Dict[str, Any]],
    name: Optional[str] = None,
    description: Optional[str] = None,
) -> Any:
    """
    Wrap a plain Python function as a LlamaIndex FunctionTool with an
    a1 authorization gate.

    The ``resolve_context`` callable receives the kwargs dict that LlamaIndex
    passes to the tool and must return a dict with at least::

        {
            "chain": <signed_chain_dict>,
            "executor_pk_hex": "<hex string>",
        }

    Parameters
    ----------
    fn:
        The tool's implementation function.
    intent_name:
        The capability to check, e.g. ``"portfolio.read"``.
    client:
        A ``A1Client`` pointed at the a1 gateway.
    resolve_context:
        Callable that extracts the chain and executor key from tool kwargs.
    name:
        Override for the tool name (defaults to ``fn.__name__``).
    description:
        Override for the tool description (defaults to ``fn.__doc__``).

    Returns
    -------
    llama_index.core.tools.FunctionTool
        A guarded tool ready for use in any LlamaIndex agent.
    """
    try:
        from llama_index.core.tools import FunctionTool
    except ImportError as exc:
        raise ImportError(
            "LlamaIndex is required: pip install llama-index-core"
        ) from exc

    tool_name = name or fn.__name__
    tool_description = description or (fn.__doc__ or "").strip() or tool_name

    @functools.wraps(fn)
    def guarded(**kwargs: Any) -> Any:
        ctx = resolve_context(kwargs)
        chain = ctx.get("chain")
        executor_pk = ctx.get("executor_pk_hex", "")

        if chain is None:
            raise A1Error(
                f"resolve_context must supply 'chain' for intent '{intent_name}'",
                error_code="MISSING_CHAIN",
            )

        client.authorize(
            chain=chain,
            intent_name=intent_name,
            executor_pk_hex=executor_pk,
        )
        return fn(**kwargs)

    guarded.__name__ = tool_name
    guarded.__doc__ = tool_description

    return FunctionTool.from_defaults(
        fn=guarded,
        name=tool_name,
        description=tool_description,
    )


def a1_llamaindex_guard(
    *,
    intent_name: str,
    client: A1Client,
    chain_kwarg: str = "signed_chain",
    executor_kwarg: str = "executor_pk_hex",
) -> Callable:
    """
    Decorator variant for use on standalone async functions consumed by
    LlamaIndex agents.

    The decorated function must accept ``signed_chain`` and
    ``executor_pk_hex`` kwargs (configurable via ``chain_kwarg`` and
    ``executor_kwarg``).

    Example
    -------
    ::

        @a1_llamaindex_guard(intent_name="portfolio.read", client=client)
        async def read_portfolio(ticker: str, signed_chain: dict, executor_pk_hex: str) -> dict:
            return {"ticker": ticker}
    """
    import asyncio
    import inspect

    def decorator(fn: Callable) -> Callable:
        if inspect.iscoroutinefunction(fn):
            @functools.wraps(fn)
            async def async_wrapper(*args: Any, **kwargs: Any) -> Any:
                chain = kwargs.get(chain_kwarg)
                executor_pk = kwargs.get(executor_kwarg, "")
                if chain is None:
                    raise A1Error(
                        f"missing required kwarg '{chain_kwarg}'",
                        error_code="MISSING_CHAIN",
                    )
                await client.authorize_async(
                    chain=chain,
                    intent_name=intent_name,
                    executor_pk_hex=executor_pk,
                )
                return await fn(*args, **kwargs)

            return async_wrapper

        @functools.wraps(fn)
        def sync_wrapper(*args: Any, **kwargs: Any) -> Any:
            chain = kwargs.get(chain_kwarg)
            executor_pk = kwargs.get(executor_kwarg, "")
            if chain is None:
                raise A1Error(
                    f"missing required kwarg '{chain_kwarg}'",
                    error_code="MISSING_CHAIN",
                )
            client.authorize(
                chain=chain,
                intent_name=intent_name,
                executor_pk_hex=executor_pk,
            )
            return fn(*args, **kwargs)

        return sync_wrapper

    return decorator