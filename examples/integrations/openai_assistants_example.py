"""
examples/integrations/openai_assistants_example.py

Shows how to plug dyolo-kya into the OpenAI Assistants API tool-call loop.
Every function call is authorized against the full cryptographic delegation
chain before the function body executes.

Run the gateway first:

    docker run -p 8080:8080 ghcr.io/dyologician/dyolo-kya-gateway:2

Install:

    pip install dyolo-kya openai
"""
from __future__ import annotations

import json
import os
import time

import openai

from dyolo_kya import KyaClient
from dyolo_kya.openai_tool import kya_function_guard

# ── Setup ─────────────────────────────────────────────────────────────────────

openai_client = openai.OpenAI()
kya           = KyaClient(os.getenv("DYOLO_GATEWAY_URL", "http://localhost:8080"))

AGENT_PK_HEX: str  = os.environ["AGENT_PK_HEX"]
SIGNED_CHAIN: dict = json.loads(os.environ["AGENT_SIGNED_CHAIN"])

# ── Guarded function ──────────────────────────────────────────────────────────

@kya_function_guard(
    intent_name="trade.equity",
    client=kya,
    chain=SIGNED_CHAIN,
    executor_pk_hex=AGENT_PK_HEX,
)
def execute_trade(symbol: str, qty: int) -> dict:
    """Execute an equity trade."""
    print(f"[broker] BUY {qty} × {symbol}")
    return {"status": "filled", "symbol": symbol, "qty": qty}


TOOLS = [
    {
        "type": "function",
        "function": {
            "name": "execute_trade",
            "description": "Execute an equity trade on behalf of the authorized user.",
            "parameters": {
                "type": "object",
                "properties": {
                    "symbol": {"type": "string", "description": "Ticker symbol, e.g. AAPL"},
                    "qty":    {"type": "integer", "description": "Number of shares to buy"},
                },
                "required": ["symbol", "qty"],
            },
        },
    }
]

TOOL_REGISTRY = {"execute_trade": execute_trade}

# ── Run loop ──────────────────────────────────────────────────────────────────

def run(user_message: str) -> str:
    assistant = openai_client.beta.assistants.create(
        model="gpt-4o",
        instructions="You are a trading assistant. Use execute_trade to place orders.",
        tools=TOOLS,
    )
    thread = openai_client.beta.threads.create()
    openai_client.beta.threads.messages.create(
        thread_id=thread.id, role="user", content=user_message,
    )
    run_obj = openai_client.beta.threads.runs.create(
        thread_id=thread.id, assistant_id=assistant.id,
    )

    while True:
        run_obj = openai_client.beta.threads.runs.retrieve(
            thread_id=thread.id, run_id=run_obj.id,
        )
        if run_obj.status == "requires_action":
            outputs = []
            for tc in run_obj.required_action.submit_tool_outputs.tool_calls:
                fn   = TOOL_REGISTRY.get(tc.function.name)
                args = json.loads(tc.function.arguments)
                out  = fn(**args) if fn else {"error": "unknown function"}
                outputs.append({"tool_call_id": tc.id, "output": json.dumps(out)})
            openai_client.beta.threads.runs.submit_tool_outputs(
                thread_id=thread.id, run_id=run_obj.id, tool_outputs=outputs,
            )
        elif run_obj.status == "completed":
            msgs = openai_client.beta.threads.messages.list(thread_id=thread.id)
            return msgs.data[0].content[0].text.value
        elif run_obj.status in ("failed", "cancelled", "expired"):
            return f"Run ended with status: {run_obj.status}"
        else:
            time.sleep(0.5)


if __name__ == "__main__":
    print(run("Buy 10 shares of AAPL for me."))
