"""
a1.semantic_kernel_tool — a1 guard for Semantic Kernel functions.

Drop-in guard that wraps any SK KernelFunction so that every invocation is
cryptographically authorized before execution. Compatible with
semantic-kernel >= 1.0.

Usage
-----
    from a1.semantic_kernel_tool import a1_sk_guard

    @a1_sk_guard(passport_path="passport.json")
    @kernel_function(name="execute_trade", description="Execute a trade order")
    async def execute_trade(symbol: str, qty: int) -> str:
        ...
"""

from __future__ import annotations

import functools
import json
import os
from pathlib import Path
from typing import Any, Callable, TypeVar

from .client import AsyncA1Client, IntentSpec, A1Error
from .passport import PassportClient, a1_guard

F = TypeVar("F", bound=Callable[..., Any])

_GATEWAY_ENV = "A1_GATEWAY_URL"
_DEFAULT_GATEWAY = "http://localhost:8080"


def a1_sk_guard(
    passport_path: str | Path | None = None,
    gateway_url: str | None = None,
    capability: str | None = None,
    namespace: str | None = None,
) -> Callable[[F], F]:
    """Decorator that adds a1 authorization to a Semantic Kernel function.

    Parameters
    ----------
    passport_path:
        Path to the ``passport.json`` file.  Defaults to ``A1_PASSPORT``
        env var, then ``./passport.json``.
    gateway_url:
        a1 gateway URL.  Defaults to ``A1_GATEWAY_URL`` env var, then
        ``http://localhost:8080``.
    capability:
        Override the capability string sent for authorization.  If omitted the
        wrapped function's ``__name__`` is used.
    namespace:
        Passport namespace to verify.  If omitted the namespace stored in the
        passport file is used.
    """

    _gw = gateway_url or os.environ.get(_GATEWAY_ENV, _DEFAULT_GATEWAY)
    _pp = Path(passport_path or os.environ.get("A1_PASSPORT", "passport.json"))

    def decorator(fn: F) -> F:
        cap = capability or fn.__name__

        @functools.wraps(fn)
        async def wrapper(*args: Any, **kwargs: Any) -> Any:
            if not _pp.exists():
                raise A1Error(
                    f"Passport file not found: {_pp}. "
                    "Run `a1 passport issue` to create one."
                )

            passport_data = json.loads(_pp.read_text())
            pc = PassportClient(gateway_url=_gw)

            receipt = await pc.authorize_async(
                passport=passport_data,
                capability=cap,
                namespace=namespace,
            )

            return await fn(*args, **kwargs)

        return wrapper  # type: ignore[return-value]

    return decorator


class DyoloKernelPlugin:
    """Semantic Kernel plugin wrapper that guards all methods with a1.

    Usage
    -----
        plugin = DyoloKernelPlugin(
            plugin_instance=MyPlugin(),
            passport_path="passport.json",
        )
        kernel.add_plugin(plugin, plugin_name="MyPlugin")
    """

    def __init__(
        self,
        plugin_instance: Any,
        passport_path: str | Path | None = None,
        gateway_url: str | None = None,
    ) -> None:
        self._plugin = plugin_instance
        self._passport_path = Path(
            passport_path or os.environ.get("A1_PASSPORT", "passport.json")
        )
        self._gateway_url = gateway_url or os.environ.get(_GATEWAY_ENV, _DEFAULT_GATEWAY)
        self._pc = PassportClient(gateway_url=self._gateway_url)

        for attr_name in dir(plugin_instance):
            if attr_name.startswith("_"):
                continue
            attr = getattr(plugin_instance, attr_name)
            if callable(attr):
                setattr(self, attr_name, self._wrap(attr, attr_name))

    def _wrap(self, fn: Callable[..., Any], name: str) -> Callable[..., Any]:
        passport_path = self._passport_path
        pc = self._pc

        @functools.wraps(fn)
        async def guarded(*args: Any, **kwargs: Any) -> Any:
            if not passport_path.exists():
                raise A1Error(f"Passport file not found: {passport_path}")
            passport_data = json.loads(passport_path.read_text())
            await pc.authorize_async(passport=passport_data, capability=name)
            return await fn(*args, **kwargs)

        return guarded
