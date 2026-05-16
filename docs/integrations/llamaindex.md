# A1 + LlamaIndex

Every LlamaIndex tool call authorized before execution. Zero architecture changes required.

## Install

```bash
pip install a1 llama-index-core
```

## One-line guard

```python
from a1.llamaindex_tool import a1_llamaindex_guard

@a1_llamaindex_guard(passport_path="passport.json", capability="research.web_search")
async def web_search(query: str) -> str:
    return await your_search_backend(query)
```

That is the entire integration. Every call to `web_search` now:
1. Verifies the passport cryptographic signature.
2. Checks that the `research.web_search` capability is in the passport's `NarrowingMatrix`.
3. Emits a tamper-evident `AuditRecord`.
4. Executes the function — or raises `PassportError` if any check fails.

## With FunctionTool

```python
from llama_index.core.tools import FunctionTool

guarded_tool = FunctionTool.from_defaults(async_fn=web_search)
agent = ReActAgent.from_tools([guarded_tool], llm=llm)
```

## Full example

See `examples/integrations/llamaindex_example.py`.
