"""
a1 + LlamaIndex — research agent with cryptographic chain-of-custody.

Every tool call is authorized against a DyoloPassport before execution.
Run: python examples/integrations/llamaindex_example.py
"""

from __future__ import annotations

import asyncio
import json
from pathlib import Path

# Conditional imports so the example is readable without llama-index installed.
try:
    from llama_index.core.tools import FunctionTool
except ImportError:
    print("Install llama-index-core to run this example.")
    raise

from a1.llamaindex_tool import a1_llamaindex_guard


# 1. Define your research tool functions and guard them.

@a1_llamaindex_guard(passport_path="passport.json", capability="research.web_search")
async def web_search(query: str) -> str:
    """Search the web and return a summary."""
    return f"[authorized] Search results for: {query}"


@a1_llamaindex_guard(passport_path="passport.json", capability="research.summarize")
async def summarize_document(url: str) -> str:
    """Fetch and summarize a document at the given URL."""
    return f"[authorized] Summary of document at: {url}"


# 2. Wrap as LlamaIndex FunctionTools.

web_search_tool = FunctionTool.from_defaults(async_fn=web_search)
summarize_tool = FunctionTool.from_defaults(async_fn=summarize_document)


async def main() -> None:
    # 3. Use the tools in your agent.
    result = await web_search(query="AI agent security frameworks 2025")
    print(result)

    summary = await summarize_document(url="https://example.com/whitepaper.pdf")
    print(summary)


if __name__ == "__main__":
    asyncio.run(main())
