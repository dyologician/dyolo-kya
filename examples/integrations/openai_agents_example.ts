/**
 * examples/integrations/openai_agents_example.ts
 *
 * TypeScript example: guarding an OpenAI Agents SDK tool with dyolo-kya.
 * Every tool_call is authorized before the function executes.
 *
 * Run the gateway:
 *   docker run -p 8080:8080 ghcr.io/dyologician/dyolo-kya-gateway:2
 *
 * Install:
 *   npm install dyolo-kya openai
 */

import OpenAI from "openai";
import { KyaClient, type SignedChain } from "dyolo-kya";
import { buildOpenAIKyaFunction } from "dyolo-kya/integrations";

const openai = new OpenAI();
const kya    = new KyaClient(process.env.DYOLO_GATEWAY_URL ?? "http://localhost:8080");

const AGENT_PK_HEX: string  = process.env.AGENT_PK_HEX!;
const SIGNED_CHAIN: SignedChain = JSON.parse(process.env.AGENT_SIGNED_CHAIN!);

// ── Guarded tool ──────────────────────────────────────────────────────────────

interface TradeArgs { symbol: string; qty: number }

const tradeTool = buildOpenAIKyaFunction<TradeArgs>({
  name: "execute_trade",
  description: "Execute an equity trade on behalf of the authorized user.",
  parameters: {
    type: "object",
    properties: {
      symbol: { type: "string" },
      qty:    { type: "integer" },
    },
    required: ["symbol", "qty"],
  },
  intentName: "trade.equity",
  client: kya,
  resolveContext: (args) => ({
    chain: SIGNED_CHAIN,
    executorPkHex: AGENT_PK_HEX,
    intentParams: { symbol: args.symbol },
  }),
  execute: async (args, auth) => {
    console.log(`[broker] BUY ${args.qty} × ${args.symbol}`);
    return { status: "filled", symbol: args.symbol, qty: args.qty, chain_depth: auth.chainDepth };
  },
});

const TOOL_REGISTRY = new Map([
  [tradeTool.definition.function.name, tradeTool.handler],
]);

// ── Run loop ──────────────────────────────────────────────────────────────────

async function run(userMessage: string): Promise<string> {
  const thread = await openai.beta.threads.create();

  await openai.beta.threads.messages.create(thread.id, {
    role: "user", content: userMessage,
  });

  const assistant = await openai.beta.assistants.create({
    model: "gpt-4o",
    instructions: "You are a trading assistant. Use execute_trade to place orders.",
    tools: [tradeTool.definition],
  });

  let run = await openai.beta.threads.runs.create(thread.id, {
    assistant_id: assistant.id,
  });

  while (true) {
    run = await openai.beta.threads.runs.retrieve(thread.id, run.id);

    if (run.status === "requires_action") {
      const toolOutputs = await Promise.all(
        run.required_action!.submit_tool_outputs.tool_calls.map(async (tc) => {
          const handler = TOOL_REGISTRY.get(tc.function.name);
          const output  = handler
            ? await handler(tc.function.arguments)
            : JSON.stringify({ error: "unknown function" });
          return { tool_call_id: tc.id, output };
        }),
      );
      await openai.beta.threads.runs.submitToolOutputs(thread.id, run.id, { tool_outputs: toolOutputs });

    } else if (run.status === "completed") {
      const messages = await openai.beta.threads.messages.list(thread.id);
      const last = messages.data[0]?.content[0];
      return last && last.type === "text" ? last.text.value : "";

    } else if (["failed", "cancelled", "expired"].includes(run.status)) {
      return `Run ended: ${run.status}`;

    } else {
      await new Promise((r) => setTimeout(r, 500));
    }
  }
}

run("Buy 10 shares of AAPL for me.").then(console.log);
