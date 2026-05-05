"""
CrewAI integration for dyolo-kya.

Provides a `KyaAuthorizationTool` that inherits from `crewai.tools.BaseTool`.
Requires: ``pip install crewai``
"""

from __future__ import annotations

from typing import Any, Callable, Dict, Optional, Type
from crewai.tools import BaseTool
from pydantic import BaseModel, Field

from .client import KyaClient


class KyaAuthorizationTool(BaseTool):
    """
    CrewAI tool wrapper that enforces dyolo-kya authorization.
    
    The underlying function is only executed if the delegation chain
    is successfully verified against the gateway.
    """
    name: str = "kya_authorized_tool"
    description: str = "Executes an action after verifying cryptographic authorization."
    args_schema: Optional[Type[BaseModel]] = None

    func: Callable[..., Any] = Field(exclude=True)
    chain: Any
    executor_pk_hex: str
    intent_name: str
    intent_params: Optional[Dict[str, str]] = None
    gateway_url: Optional[str] = None

    def _run(self, *args: Any, **kwargs: Any) -> Any:
        client = KyaClient(self.gateway_url)
        client.authorize(
            chain=self.chain,
            intent_name=self.intent_name,
            executor_pk_hex=self.executor_pk_hex,
            intent_params=self.intent_params,
        )
        return self.func(*args, **kwargs)