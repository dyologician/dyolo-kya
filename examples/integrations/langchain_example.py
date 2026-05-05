"""
examples/integrations/langchain_example.py

Complete LangChain integration showing how to gate a trading tool with
dyolo-kya authorization.  Run the gateway first:

    docker run -p 8080:8080 ghcr.io/dyologician/dyolo-kya-gateway:2

Install deps:

    pip install dyolo-kya langchain langchain-openai
"""
from __future__ import annotations

import json
import os

from dyolo_kya import KyaClient, IntentSpec
from dyolo_kya.langchain_tool import kya_tool

# ── Setup ─────────────────────────────────────────────────────────────────────

kya = KyaClient(os.getenv("DYOLO_GATEWAY_URL", "http://localhost:8080"))

# In production these come from your identity provider / secrets manager.
AGENT_PK_HEX: str = os.environ["AGENT_PK_HEX"]
# The signed chain is obtained from the previous delegation step and passed
# through your agent's session context.
SIGNED_CHAIN: dict = json.loads(os.environ["AGENT_SIGNED_CHAIN"])

# ── Define a guarded LangChain tool ──────────────────────────────────────────

@kya_tool(
    name="execute_trade",
    description="Execute an equity trade on the user's behalf. Input: JSON with symbol and qty.",
    intent_name="trade.equity",
    client=kya,
    chain=SIGNED_CHAIN,
    executor_pk_hex=AGENT_PK_HEX,
)
def execute_trade(tool_input: str) -> str:
    args = json.loads(tool_input)
    symbol: str = args["symbol"]
    qty: int    = int(args["qty"])

    # Replace with your real broker call.
    print(f"[broker] BUY {qty} × {symbol}")
    return json.dumps({"status": "filled", "symbol": symbol, "qty": qty})


# ── Wire up to a LangChain agent ─────────────────────────────────────────────

def main() -> None:
    from langchain.agents import AgentExecutor, create_openai_tools_agent
    from langchain_openai import ChatOpenAI
    from langchain_core.prompts import ChatPromptTemplate, MessagesPlaceholder

    llm = ChatOpenAI(model="gpt-4o", temperature=0)

    prompt = ChatPromptTemplate.from_messages([
        ("system", "You are a trading assistant. Use execute_trade to place orders."),
        ("human", "{input}"),
        MessagesPlaceholder("agent_scratchpad"),
    ])

    agent = create_openai_tools_agent(llm, [execute_trade], prompt)
    executor = AgentExecutor(agent=agent, tools=[execute_trade], verbose=True)

    result = executor.invoke({"input": "Buy 10 shares of AAPL for me."})
    print(result["output"])


if __name__ == "__main__":
    main()
