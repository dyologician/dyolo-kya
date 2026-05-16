"""
a1.middleware — Universal @a1.protect decorator + inject_passport.

One line protects any tool in any framework.

    @a1.protect(capability="trade.equity")
    async def execute_trade(symbol, qty):
        ...

    # Or inject automatically into an entire agent:
    from a1 import inject_passport
    agent = inject_passport(my_agent, passport=client)
"""
from __future__ import annotations

import asyncio, functools, json, os, threading
from contextlib import contextmanager
from dataclasses import dataclass, field
from typing import Any, Callable, Optional

from .client import A1Client, A1Error
from .passport import PassportClient, PassportReceipt

__all__ = ["protect", "inject_passport", "A1Context", "set_context", "get_context", "a1_context", "MiddlewareError"]

_ctx_local = threading.local()


class MiddlewareError(Exception):
    def __init__(self, msg: str, code: str = "MIDDLEWARE_ERROR") -> None:
        super().__init__(msg)
        self.code = code


@dataclass
class A1Context:
    chain: Optional[dict | str] = None
    executor_pk_hex: Optional[str] = None
    gateway_url: str = field(default_factory=lambda: os.environ.get("A1_GATEWAY_URL", "http://localhost:8080"))
    client: Optional[PassportClient] = None
    _receipts: list[PassportReceipt] = field(default_factory=list)

    def record_receipt(self, r: PassportReceipt) -> None:
        self._receipts.append(r)

    def receipts(self) -> list[PassportReceipt]:
        return list(self._receipts)


def set_context(ctx: A1Context) -> None:
    _ctx_local.ctx = ctx


def get_context() -> Optional[A1Context]:
    return getattr(_ctx_local, "ctx", None)


@contextmanager
def a1_context(chain=None, executor_pk_hex=None, gateway_url=None, client=None):
    """Context manager — all @a1.protect tools inside use this chain automatically."""
    prev = get_context()
    ctx = A1Context(
        chain=chain,
        executor_pk_hex=executor_pk_hex,
        gateway_url=gateway_url or os.environ.get("A1_GATEWAY_URL", "http://localhost:8080"),
        client=client,
    )
    set_context(ctx)
    try:
        yield ctx
    finally:
        if prev is not None:
            set_context(prev)
        else:
            try: del _ctx_local.ctx
            except AttributeError: pass


def _extract_chain(kwargs: dict) -> Optional[str]:
    for key in ("signed_chain", "_a1_chain"):
        if key in kwargs:
            val = kwargs.pop(key)
            return json.dumps(val) if isinstance(val, dict) else val
    for ck in ("config", "context", "run_context"):
        cfg = kwargs.get(ck, {})
        if isinstance(cfg, dict) and "a1_chain" in cfg:
            val = cfg["a1_chain"]
            return json.dumps(val) if isinstance(val, dict) else val
    ctx = get_context()
    if ctx and ctx.chain is not None:
        return json.dumps(ctx.chain) if isinstance(ctx.chain, dict) else ctx.chain
    return os.environ.get("A1_CHAIN_JSON")


def _extract_pk(kwargs: dict) -> Optional[str]:
    for key in ("executor_pk_hex", "_a1_executor_pk", "agent_pk_hex"):
        if key in kwargs:
            return kwargs.pop(key)
    ctx = get_context()
    if ctx and ctx.executor_pk_hex:
        return ctx.executor_pk_hex
    return os.environ.get("A1_EXECUTOR_PK_HEX")


def protect(
    capability: str,
    *,
    gateway_url: Optional[str] = None,
    client: Optional[PassportClient] = None,
    intent_params_fn: Optional[Callable[..., dict]] = None,
    pass_receipt: bool = False,
    require_chain: bool = True,
):
    """
    Universal authorization decorator. Works on any sync or async function
    in any framework — LangGraph, CrewAI, AutoGen, OpenAI Agents, plain Python.

        @a1.protect(capability="trade.equity")
        async def execute_trade(symbol: str, qty: int) -> dict:
            ...
    """
    url = gateway_url or os.environ.get("A1_GATEWAY_URL", "http://localhost:8080")

    def decorator(fn: Callable) -> Callable:
        is_async = asyncio.iscoroutinefunction(fn)

        def _do_check(kwargs: dict) -> Optional[PassportReceipt]:
            chain_json = _extract_chain(kwargs)
            executor_pk = _extract_pk(kwargs)
            if chain_json is None or executor_pk is None:
                if require_chain:
                    raise MiddlewareError(
                        f"@a1.protect('{capability}'): no authorization chain found. "
                        "Pass signed_chain + executor_pk_hex, use a1_context(), "
                        "or set A1_CHAIN_JSON + A1_EXECUTOR_PK_HEX.",
                        code="MISSING_CHAIN",
                    )
                return None
            params = intent_params_fn(kwargs) if intent_params_fn else {}
            _client = client or PassportClient(url)
            receipt = _client.authorize(
                chain=json.loads(chain_json) if isinstance(chain_json, str) else chain_json,
                capability=capability,
                executor_pk_hex=executor_pk,
                intent_params=params,
            )
            ctx = get_context()
            if ctx:
                ctx.record_receipt(receipt)
            return receipt

        @functools.wraps(fn)
        async def async_wrapper(*args: Any, **kwargs: Any) -> Any:
            receipt = await asyncio.get_event_loop().run_in_executor(None, lambda: _do_check(kwargs))
            if pass_receipt and receipt:
                kwargs["_a1_receipt"] = receipt
            return await fn(*args, **kwargs)

        @functools.wraps(fn)
        def sync_wrapper(*args: Any, **kwargs: Any) -> Any:
            receipt = _do_check(kwargs)
            if pass_receipt and receipt:
                kwargs["_a1_receipt"] = receipt
            return fn(*args, **kwargs)

        return async_wrapper if is_async else sync_wrapper

    return decorator


def inject_passport(agent: Any, passport: Any, *, gateway_url: Optional[str] = None) -> Any:
    """
    Inject an A1 passport context into an agent so all @a1.protect tools
    execute with automatic authorization — no code changes to tools needed.

        agent = inject_passport(create_my_agent(), passport=client)
        agent.run("buy 100 AAPL")   # tools are authorized automatically
    """
    url = gateway_url or os.environ.get("A1_GATEWAY_URL", "http://localhost:8080")
    _client = passport if isinstance(passport, PassportClient) else PassportClient(url)

    run_fn = getattr(agent, "run", None) or getattr(agent, "invoke", None)
    if run_fn is None:
        raise MiddlewareError("agent must have .run() or .invoke()", code="UNSUPPORTED_AGENT")

    method_name = "run" if hasattr(agent, "run") else "invoke"
    is_async = asyncio.iscoroutinefunction(run_fn)

    if is_async:
        @functools.wraps(run_fn)
        async def patched(*args, **kwargs):
            ctx = A1Context(gateway_url=url, client=_client)
            set_context(ctx)
            try:
                return await run_fn(*args, **kwargs)
            finally:
                try: del _ctx_local.ctx
                except AttributeError: pass
    else:
        @functools.wraps(run_fn)
        def patched(*args, **kwargs):
            ctx = A1Context(gateway_url=url, client=_client)
            set_context(ctx)
            try:
                return run_fn(*args, **kwargs)
            finally:
                try: del _ctx_local.ctx
                except AttributeError: pass

    setattr(agent, method_name, patched)
    return agent