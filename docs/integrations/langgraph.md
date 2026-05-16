# A1 + LangGraph

Authorize individual graph nodes — not just the agent entry point. Each node in your state graph can require a different capability scope.

## Install

```bash
pip install a1 langgraph
```

## Guard a node

```python
from a1.langgraph_tool import a1_node

@a1_node(passport_path="passport.json", capability="trade.equity")
async def execute_trade_node(state: TradingState) -> dict:
    return {**state, "result": await trade_service.execute(state)}
```

## Add to your graph

```python
from langgraph.graph import StateGraph

graph = (
    StateGraph(TradingState)
    .add_node("execute_trade", execute_trade_node)
    .add_node("read_portfolio", read_portfolio_node)
    .set_entry_point("read_portfolio")
    .add_edge("read_portfolio", "execute_trade")
    .compile()
)
```

Each node's capability is checked independently at runtime. A passport that holds `portfolio.read` but not `trade.equity` will be approved at `read_portfolio` and denied at `execute_trade` — no single over-privileged gate.

## Full example

See `examples/integrations/langgraph_example.py`.
