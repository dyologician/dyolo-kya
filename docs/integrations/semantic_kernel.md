# A1 + Microsoft Semantic Kernel

Guards every `KernelFunction` so that unauthorized capabilities are rejected before execution. Compatible with `semantic-kernel >= 1.0`.

## Install

```bash
pip install a1 semantic-kernel
```

## Guard a kernel function

```python
from a1.semantic_kernel_tool import a1_sk_guard
from semantic_kernel.functions import kernel_function

class TradingPlugin:
    @a1_sk_guard(passport_path="passport.json", capability="trade.equity")
    @kernel_function(name="execute_trade", description="Execute an equity trade")
    async def execute_trade(self, symbol: str, quantity: int) -> str:
        return await trade_service.execute(symbol, quantity)
```

## Wrap an entire plugin

```python
from a1.semantic_kernel_tool import DyoloKernelPlugin

guarded_plugin = DyoloKernelPlugin(
    plugin_instance=TradingPlugin(),
    passport_path="passport.json",
)
kernel.add_plugin(guarded_plugin, plugin_name="Trading")
```

`DyoloKernelPlugin` wraps every callable method on the plugin instance automatically. You do not need to decorate each method individually.

## TypeScript

```ts
import { withDyoloSkFunction } from "a1";

const guardedExecuteTrade = withDyoloSkFunction({
  intentName: "trade.equity",
  client: a1Client,
  resolveContext: (args) => ({ chain: args.chain, executorPkHex: agentPk }),
  fn: async (args, auth) => tradeService.execute(args),
});
```

## Full example

See `examples/integrations/semantic_kernel_example.py`.
