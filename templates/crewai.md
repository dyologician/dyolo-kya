# Connecting A1 to CrewAI

---

## Install

```bash
pip install "a1[crewai]"
```

---

## Quick integration

```python
from a1.crewai_tool import A1AuthorizationTool
from crewai import Agent, Task, Crew

# Create A1-protected tool
tool = A1AuthorizationTool(
    func=execute_trade_fn,           # your existing function
    intent_name="trade.equity",
    gateway_url="http://localhost:8080",
    passport_path="./passport.json",
    chain=agent_chain,
    executor_pk_hex=agent_pk,
)

# Use with CrewAI as normal
trader = Agent(
    role="Equity Trader",
    goal="Execute authorized trades",
    backstory="An AI trader with cryptographic authorization.",
    tools=[tool],
)

task = Task(description="Buy 100 shares of AAPL", agent=trader)
crew = Crew(agents=[trader], tasks=[task])
result = crew.kickoff()
```

---

## What A1 adds to CrewAI

Without A1, CrewAI agents can call any tool they have access to. With A1:
- Each tool checks a cryptographic delegation chain before executing
- The original human authorization is provably traceable
- Every tool call produces a `ProvableReceipt` for audit

---

## Full example

See [examples/integrations/autogen_example.py](../examples/integrations/autogen_example.py) for a complete working example.

---

## Need help?

- A1 Studio: http://localhost:8080/studio
- Full docs: [CAPABILITIES.md](../CAPABILITIES.md)
- GitHub: https://github.com/dyologician/a1
