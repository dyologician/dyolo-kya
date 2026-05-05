"""
examples/integrations/autogen_example.py

Shows how to gate an AutoGen agent tool with dyolo-kya authorization.
Every registered function is checked against the cryptographic delegation
chain before execution.

Run the gateway first:

    docker run -p 8080:8080 ghcr.io/dyologician/dyolo-kya-gateway:2

Install:

    pip install dyolo-kya pyautogen
"""
from __future__ import annotations

import json
import os

import autogen

from dyolo_kya import KyaClient
from dyolo_kya.openai_tool import kya_function_guard

# ── Setup ─────────────────────────────────────────────────────────────────────

kya = KyaClient(os.getenv("DYOLO_GATEWAY_URL", "http://localhost:8080"))

AGENT_PK_HEX: str  = os.environ["AGENT_PK_HEX"]
SIGNED_CHAIN: dict = json.loads(os.environ["AGENT_SIGNED_CHAIN"])

# ── Guarded tool ──────────────────────────────────────────────────────────────

@kya_function_guard(
    intent_name="trade.equity",
    client=kya,
    chain=SIGNED_CHAIN,
    executor_pk_hex=AGENT_PK_HEX,
)
def execute_trade(symbol: str, qty: int) -> str:
    """Execute an equity trade."""
    print(f"[broker] BUY {qty} × {symbol}")
    return json.dumps({"status": "filled", "symbol": symbol, "qty": qty})


# ── AutoGen agent setup ───────────────────────────────────────────────────────

def main() -> None:
    config_list = autogen.config_list_from_json("OAI_CONFIG_LIST")

    assistant = autogen.AssistantAgent(
        name="trading_assistant",
        llm_config={"config_list": config_list},
        system_message="You are a trading assistant. Use execute_trade to place orders.",
    )

    user_proxy = autogen.UserProxyAgent(
        name="user_proxy",
        human_input_mode="NEVER",
        max_consecutive_auto_reply=5,
        code_execution_config=False,
    )

    autogen.register_function(
        execute_trade,
        caller=assistant,
        executor=user_proxy,
        name="execute_trade",
        description="Execute an equity trade on behalf of the authorized user",
    )

    user_proxy.initiate_chat(
        assistant,
        message="Buy 10 shares of AAPL for me.",
    )


if __name__ == "__main__":
    main()
