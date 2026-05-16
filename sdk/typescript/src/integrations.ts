/**
 * a1 integrations for LangChain.js and the OpenAI Agents SDK.
 *
 * Guards every tool call so that no delegated action executes unless the full
 * cryptographic chain-of-custody is verified first. Both single-intent and
 * multi-intent (batch) authorization patterns are supported.
 */

import { A1Client, A1Error, SignedChain, AuthorizeResult, BatchAuthorizeResult } from "./index.js";

// ── Shared types ──────────────────────────────────────────────────────────────

/** Context attached to every guarded tool invocation. */
export interface A1GuardContext {
  /** The agent's delegation chain in wire format. */
  chain: SignedChain;
  /** The agent's Ed25519 public key (hex). */
  executorPkHex: string;
  /** Optional intent parameter overrides. */
  intentParams?: Record<string, string>;
}

export type GuardedToolFn<TArgs, TReturn> = (
  args: TArgs,
  a1: AuthorizeResult,
) => Promise<TReturn>;

// ── LangChain.js — single intent ──────────────────────────────────────────────

export interface LangChainA1ToolOptions {
  name: string;
  description: string;
  intentName: string;
  client: A1Client;
  resolveContext: (rawInput: string) => A1GuardContext;
  run: (rawInput: string, auth: AuthorizeResult) => Promise<string>;
}

/**
 * Wrap a LangChain tool with a a1 single-intent authorization gate.
 *
 * ```ts
 * const tool = buildLangChainA1Tool({
 *   name: "execute_trade",
 *   description: "Execute an equity trade",
 *   intentName: "trade.equity",
 *   client: a1Client,
 *   resolveContext: (input) => ({ chain: agentChain, executorPkHex: agentPk }),
 *   run: async (input, auth) => {
 *     const { symbol, qty } = JSON.parse(input);
 *     await broker.trade(symbol, qty);
 *     return `Executed. Chain depth: ${auth.chainDepth}`;
 *   },
 * });
 * ```
 */
export function buildLangChainA1Tool(opts: LangChainA1ToolOptions) {
  return {
    name: opts.name,
    description: opts.description,

    async call(rawInput: string): Promise<string> {
      const ctx = opts.resolveContext(rawInput);

      let auth: AuthorizeResult;
      try {
        auth = await opts.client.authorize({
          chain: ctx.chain,
          intentName: opts.intentName,
          intentParams: ctx.intentParams,
          executorPkHex: ctx.executorPkHex,
        });
      } catch (err) {
        if (err instanceof A1Error) {
          throw new Error(
            `[a1] Authorization denied for intent "${opts.intentName}": ${err.message} (${err.code ?? "unknown"})`,
          );
        }
        throw err;
      }

      return opts.run(rawInput, auth);
    },
  };
}

// ── LangChain.js — batch intents ──────────────────────────────────────────────

export interface LangChainA1BatchToolOptions {
  name: string;
  description: string;
  /** All intents that must be authorized before the tool runs. */
  intentNames: string[];
  client: A1Client;
  resolveContext: (rawInput: string) => A1GuardContext;
  run: (rawInput: string, auth: BatchAuthorizeResult) => Promise<string>;
}

/**
 * Wrap a LangChain tool with a a1 batch-intent authorization gate.
 *
 * All listed intents are verified atomically in a single round-trip. If any
 * intent fails, the tool does not execute.
 *
 * ```ts
 * const tool = buildLangChainA1BatchTool({
 *   name: "portfolio_rebalance",
 *   description: "Query portfolio and execute trades",
 *   intentNames: ["query.portfolio", "trade.equity"],
 *   client: a1Client,
 *   resolveContext: (input) => ({ chain: agentChain, executorPkHex: agentPk }),
 *   run: async (input, auth) => {
 *     if (!auth.allAuthorized) throw new Error("Not all intents authorized");
 *     return "Rebalanced.";
 *   },
 * });
 * ```
 */
export function buildLangChainA1BatchTool(opts: LangChainA1BatchToolOptions) {
  return {
    name: opts.name,
    description: opts.description,

    async call(rawInput: string): Promise<string> {
      const ctx = opts.resolveContext(rawInput);

      let auth: BatchAuthorizeResult;
      try {
        auth = await opts.client.authorizeBatch({
          chain: ctx.chain,
          executorPkHex: ctx.executorPkHex,
          intents: opts.intentNames.map((name) => ({
            name,
            params: ctx.intentParams,
          })),
        });
      } catch (err) {
        if (err instanceof A1Error) {
          throw new Error(
            `[a1] Batch authorization denied for intents [${opts.intentNames.join(", ")}]: ${err.message}${err.code ? ` (${err.code})` : ""}`,
          );
        }
        throw err;
      }

      if (!auth.allAuthorized) {
        const denied = auth.results
          .filter((r) => !r.authorized)
          .map((r) => `${r.intentName}: ${r.error ?? "denied"}`)
          .join("; ");
        throw new Error(`[a1] Batch authorization failed — ${denied}`);
      }

      return opts.run(rawInput, auth);
    },
  };
}

// ── OpenAI Agents SDK — single intent ─────────────────────────────────────────

export interface OpenAIA1FunctionOptions<TArgs extends object> {
  name: string;
  description: string;
  parameters: Record<string, unknown>;
  intentName: string;
  client: A1Client;
  resolveContext: (args: TArgs) => A1GuardContext;
  execute: (args: TArgs, auth: AuthorizeResult) => Promise<unknown>;
}

export interface OpenAIA1Function<TArgs extends object> {
  definition: {
    type: "function";
    function: {
      name: string;
      description: string;
      parameters: Record<string, unknown>;
    };
  };
  handler: (argsJson: string) => Promise<string>;
}

/**
 * Build a guarded OpenAI function tool (single-intent variant).
 *
 * ```ts
 * const tradeFn = buildOpenAIA1Function({
 *   name: "execute_trade",
 *   description: "Execute an equity trade on behalf of the user",
 *   parameters: { type: "object", properties: { symbol: { type: "string" } } },
 *   intentName: "trade.equity",
 *   client: a1Client,
 *   resolveContext: (args) => ({ chain: agentChain, executorPkHex: agentPk }),
 *   execute: async (args, auth) => ({ ok: true, chain_depth: auth.chainDepth }),
 * });
 * const output = await tradeFn.handler(toolCall.function.arguments);
 * ```
 */
export function buildOpenAIA1Function<TArgs extends object>(
  opts: OpenAIA1FunctionOptions<TArgs>,
): OpenAIA1Function<TArgs> {
  return {
    definition: {
      type: "function",
      function: {
        name: opts.name,
        description: opts.description,
        parameters: opts.parameters,
      },
    },

    async handler(argsJson: string): Promise<string> {
      let args: TArgs;
      try {
        args = JSON.parse(argsJson) as TArgs;
      } catch {
        return JSON.stringify({ error: "Invalid arguments JSON" });
      }

      const ctx = opts.resolveContext(args);

      let auth: AuthorizeResult;
      try {
        auth = await opts.client.authorize({
          chain: ctx.chain,
          intentName: opts.intentName,
          intentParams: ctx.intentParams,
          executorPkHex: ctx.executorPkHex,
        });
      } catch (err) {
        if (err instanceof A1Error) {
          return JSON.stringify({ error: `Authorization denied: ${err.message}`, code: err.code });
        }
        return JSON.stringify({ error: "Authorization check failed" });
      }

      try {
        const result = await opts.execute(args, auth);
        return JSON.stringify(result);
      } catch (err) {
        return JSON.stringify({ error: (err as Error).message });
      }
    },
  };
}

// ── OpenAI Agents SDK — batch intents ─────────────────────────────────────────

export interface OpenAIA1BatchFunctionOptions<TArgs extends object> {
  name: string;
  description: string;
  parameters: Record<string, unknown>;
  /** All intents that must be authorized before the function executes. */
  intentNames: string[];
  client: A1Client;
  resolveContext: (args: TArgs) => A1GuardContext;
  execute: (args: TArgs, auth: BatchAuthorizeResult) => Promise<unknown>;
}

/**
 * Build a guarded OpenAI function tool that requires multiple intents to be
 * authorized atomically in a single round-trip (batch variant).
 *
 * Use this when a single tool action spans multiple intent domains — for
 * example a rebalancing tool that must be authorized for both `query.portfolio`
 * and `trade.equity` before either read or write is permitted.
 *
 * ```ts
 * const rebalanceFn = buildOpenAIA1BatchFunction({
 *   name: "rebalance_portfolio",
 *   description: "Query and rebalance a portfolio",
 *   parameters: { type: "object", properties: { risk_level: { type: "string" } } },
 *   intentNames: ["query.portfolio", "trade.equity"],
 *   client: a1Client,
 *   resolveContext: (args) => ({ chain: agentChain, executorPkHex: agentPk }),
 *   execute: async (args, auth) => ({ rebalanced: true, intents: auth.authorizedCount }),
 * });
 * ```
 */
export function buildOpenAIA1BatchFunction<TArgs extends object>(
  opts: OpenAIA1BatchFunctionOptions<TArgs>,
): OpenAIA1Function<TArgs> {
  return {
    definition: {
      type: "function",
      function: {
        name: opts.name,
        description: opts.description,
        parameters: opts.parameters,
      },
    },

    async handler(argsJson: string): Promise<string> {
      let args: TArgs;
      try {
        args = JSON.parse(argsJson) as TArgs;
      } catch {
        return JSON.stringify({ error: "Invalid arguments JSON" });
      }

      const ctx = opts.resolveContext(args);

      let auth: BatchAuthorizeResult;
      try {
        auth = await opts.client.authorizeBatch({
          chain: ctx.chain,
          executorPkHex: ctx.executorPkHex,
          intents: opts.intentNames.map((name) => ({
            name,
            params: ctx.intentParams,
          })),
        });
      } catch (err) {
        if (err instanceof A1Error) {
          return JSON.stringify({ error: `Batch authorization denied: ${err.message}`, code: err.code });
        }
        return JSON.stringify({ error: "Batch authorization check failed" });
      }

      if (!auth.allAuthorized) {
        const denied = auth.results
          .filter((r) => !r.authorized)
          .map((r) => `${r.intentName}: ${r.error ?? "denied"}`)
          .join("; ");
        return JSON.stringify({ error: `Batch authorization failed — ${denied}` });
      }

      try {
        const result = await opts.execute(args, auth);
        return JSON.stringify(result);
      } catch (err) {
        return JSON.stringify({ error: (err as Error).message });
      }
    },
  };
}

// ── AutoGen (ag2) — single intent ─────────────────────────────────────────────

/**
 * Middleware factory for Microsoft AutoGen agents — single intent.
 *
 * ```ts
 * const guardedTrade = withA1Guard({
 *   intentName: "trade.equity",
 *   client: a1Client,
 *   resolveContext: (args) => ({ chain: agentChain, executorPkHex: agentPk }),
 *   fn: async (args) => broker.trade(args.symbol, args.qty),
 * });
 * ```
 */
export function withA1Guard<TArgs extends Record<string, unknown>, TReturn>(opts: {
  intentName: string;
  client: A1Client;
  resolveContext: (args: TArgs) => A1GuardContext;
  fn: (args: TArgs, auth: AuthorizeResult) => Promise<TReturn>;
}): (args: TArgs) => Promise<TReturn> {
  return async (args: TArgs): Promise<TReturn> => {
    const ctx = opts.resolveContext(args);
    const auth = await opts.client.authorize({
      chain: ctx.chain,
      intentName: opts.intentName,
      intentParams: ctx.intentParams,
      executorPkHex: ctx.executorPkHex,
    });
    return opts.fn(args, auth);
  };
}

// ── AutoGen (ag2) — batch intents ─────────────────────────────────────────────

/**
 * Middleware factory for Microsoft AutoGen agents — batch intents.
 *
 * All listed intents are verified atomically. The wrapped function only
 * receives a `BatchAuthorizeResult` with `allAuthorized === true`.
 *
 * ```ts
 * const guardedRebalance = withA1BatchGuard({
 *   intentNames: ["query.portfolio", "trade.equity"],
 *   client: a1Client,
 *   resolveContext: (args) => ({ chain: agentChain, executorPkHex: agentPk }),
 *   fn: async (args, auth) => rebalancer.run(args),
 * });
 * ```
 */
export function withA1BatchGuard<TArgs extends Record<string, unknown>, TReturn>(opts: {
  intentNames: string[];
  client: A1Client;
  resolveContext: (args: TArgs) => A1GuardContext;
  fn: (args: TArgs, auth: BatchAuthorizeResult) => Promise<TReturn>;
}): (args: TArgs) => Promise<TReturn> {
  return async (args: TArgs): Promise<TReturn> => {
    const ctx = opts.resolveContext(args);
    const auth = await opts.client.authorizeBatch({
      chain: ctx.chain,
      executorPkHex: ctx.executorPkHex,
      intents: opts.intentNames.map((name) => ({
        name,
        params: ctx.intentParams,
      })),
    });
    if (!auth.allAuthorized) {
      const denied = auth.results
        .filter((r) => !r.authorized)
        .map((r) => r.intentName)
        .join(", ");
      throw new A1Error(`Batch authorization failed for intents: ${denied}`);
    }
    return opts.fn(args, auth);
  };
}

// ── LangGraph — node guard ───────────────────────────────────────────────────

export interface LangGraphA1NodeOptions<TState> {
  intentName: string;
  client: A1Client;
  resolveContext: (state: TState) => A1GuardContext;
  node: (state: TState, auth: AuthorizeResult) => Promise<Partial<TState>>;
}

/**
 * Wraps a LangGraph node so it is authorized before state mutation.
 *
 * ```ts
 * const guardedTradeNode = withDyoloLangGraphNode({
 *   intentName: "trade.equity",
 *   client: a1Client,
 *   resolveContext: (state) => ({ chain: state.chain, executorPkHex: agentPk }),
 *   node: async (state, auth) => ({ ...state, result: await executeTrade(state) }),
 * });
 *
 * const graph = new StateGraph(...)
 *   .addNode("execute_trade", guardedTradeNode);
 * ```
 */
export function withDyoloLangGraphNode<TState extends Record<string, unknown>>(
  opts: LangGraphA1NodeOptions<TState>,
): (state: TState) => Promise<Partial<TState>> {
  return async (state: TState): Promise<Partial<TState>> => {
    const ctx = opts.resolveContext(state);
    const auth = await opts.client.authorize({
      chain: ctx.chain,
      executorPkHex: ctx.executorPkHex,
      intent: { name: opts.intentName, params: ctx.intentParams },
    });
    if (!auth.authorized) {
      throw new A1Error(
        `LangGraph node '${opts.intentName}' authorization denied: ${auth.reason ?? "unknown"}`,
      );
    }
    return opts.node(state, auth);
  };
}

// ── Semantic Kernel — function guard ─────────────────────────────────────────

export interface SemanticKernelA1Options<TArgs, TReturn> {
  intentName: string;
  client: A1Client;
  resolveContext: (args: TArgs) => A1GuardContext;
  fn: (args: TArgs, auth: AuthorizeResult) => Promise<TReturn>;
}

/**
 * Wraps a Semantic Kernel plugin function with a1 authorization.
 *
 * ```ts
 * const guardedFn = withDyoloSkFunction({
 *   intentName: "execute_trade",
 *   client: a1Client,
 *   resolveContext: (args) => ({ chain: args.chain, executorPkHex: agentPk }),
 *   fn: async (args, auth) => tradeService.execute(args),
 * });
 * ```
 */
export function withDyoloSkFunction<TArgs extends Record<string, unknown>, TReturn>(
  opts: SemanticKernelA1Options<TArgs, TReturn>,
): (args: TArgs) => Promise<TReturn> {
  return async (args: TArgs): Promise<TReturn> => {
    const ctx = opts.resolveContext(args);
    const auth = await opts.client.authorize({
      chain: ctx.chain,
      executorPkHex: ctx.executorPkHex,
      intent: { name: opts.intentName, params: ctx.intentParams },
    });
    if (!auth.authorized) {
      throw new A1Error(
        `Semantic Kernel function '${opts.intentName}' authorization denied: ${auth.reason ?? "unknown"}`,
      );
    }
    return opts.fn(args, auth);
  };
}
