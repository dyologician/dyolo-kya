"""
AutoGen v0.4+ integration for dyolo-kya.

Uses the autogen_core `FunctionTool` API for agentchat.
Requires: ``pip install autogen-agentchat autogen-core``
"""

from __future__ import annotations

import functools
from typing import Any, Callable, Dict, Optional, TypeVar

from autogen_core.tools import FunctionTool
from .client import KyaClient

F = TypeVar("F", bound=Callable[..., Any])


def build_kya_function_tool(
    fn: F,
    chain: Any,
    executor_pk_hex: str,
    intent_name: str,
    intent_params: Optional[Dict[str, str]] = None,
    gateway_url: Optional[str] = None,
    name: Optional[str] = None,
    description: Optional[str] = None,
) -> FunctionTool:
    """
    Build an AutoGen v0.4 FunctionTool with a dyolo-kya authorization gate.

    Args:
        fn:              The underlying tool function to execute.
        chain:           The serialized delegation chain (``SignedChain`` dict or JSON).
        executor_pk_hex: Hex-encoded Ed25519 public key of the executing agent.
        intent_name:     The intent action name to verify (e.g. ``"trade.equity"``).
        intent_params:   Optional intent parameter bindings.
        gateway_url:     Gateway base URL (default: ``DYOLO_GATEWAY_URL`` env).
        name:            Optional tool name override.
        description:     Optional tool description override.

    Returns:
        An ``autogen_core.tools.FunctionTool`` instance ready for agent registration.
    """
    client = KyaClient(gateway_url)

    @functools.wraps(fn)
    def wrapper(*args: Any, **kwargs: Any) -> Any:
        client.authorize(
            chain=chain,
            intent_name=intent_name,
            executor_pk_hex=executor_pk_hex,
            intent_params=intent_params,
        )
        return fn(*args, **kwargs)

    return FunctionTool(
        wrapper,
        name=name or fn.__name__,
        description=description or fn.__doc__ or "Authorized tool",
    )