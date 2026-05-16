import {
  buildLangChainA1Tool,
  buildLangChainA1BatchTool,
  buildOpenAIA1Function,
  buildOpenAIA1BatchFunction,
  withA1Guard,
  withA1BatchGuard,
} from "../src/integrations.js";
import {
  A1Client,
  A1Error,
  SignedChain,
  AuthorizeResult,
  BatchAuthorizeResult,
} from "../src/index.js";

// ── Shared fixtures ───────────────────────────────────────────────────────────

const MOCK_CHAIN: SignedChain = {
  version: 1,
  principal_pk: "aa".repeat(32),
  principal_scope: "bb".repeat(32),
  certs: [],
};

const MOCK_EXECUTOR_PK = "cc".repeat(32);

const MOCK_AUTH_RESULT: AuthorizeResult = {
  authorized: true,
  chainDepth: 1,
  chainFingerprint: "dd".repeat(32),
  verifiedAtUnix: 1_700_000_000,
};

const MOCK_BATCH_RESULT: BatchAuthorizeResult = {
  allAuthorized: true,
  authorizedCount: 2,
  totalCount: 2,
  results: [
    { intentName: "query.portfolio", authorized: true },
    { intentName: "trade.equity", authorized: true },
  ],
};

function makeClient(overrides: Partial<A1Client> = {}): A1Client {
  const client = Object.create(A1Client.prototype) as A1Client;
  Object.assign(
    client,
    {
      authorize: jest.fn().mockResolvedValue(MOCK_AUTH_RESULT),
      authorizeBatch: jest.fn().mockResolvedValue(MOCK_BATCH_RESULT),
    },
    overrides,
  );
  return client;
}

// ── buildLangChainA1Tool ─────────────────────────────────────────────────────

describe("buildLangChainA1Tool", () => {
  const INPUT = JSON.stringify({ symbol: "AAPL", qty: 10 });

  it("calls authorize and returns the run result on success", async () => {
    const client = makeClient();
    const run = jest.fn().mockResolvedValue("trade executed");

    const tool = buildLangChainA1Tool({
      name: "execute_trade",
      description: "Execute a trade",
      intentName: "trade.equity",
      client,
      resolveContext: () => ({ chain: MOCK_CHAIN, executorPkHex: MOCK_EXECUTOR_PK }),
      run,
    });

    const result = await tool.call(INPUT);

    expect(client.authorize).toHaveBeenCalledWith(
      expect.objectContaining({ intentName: "trade.equity", executorPkHex: MOCK_EXECUTOR_PK }),
    );
    expect(run).toHaveBeenCalledWith(INPUT, MOCK_AUTH_RESULT);
    expect(result).toBe("trade executed");
  });

  it("wraps A1Error with intent context and rethrows", async () => {
    const a1Err = new A1Error("signature mismatch", "SIG_INVALID", 403);
    const client = makeClient({ authorize: jest.fn().mockRejectedValue(a1Err) } as never);

    const tool = buildLangChainA1Tool({
      name: "execute_trade",
      description: "Execute a trade",
      intentName: "trade.equity",
      client,
      resolveContext: () => ({ chain: MOCK_CHAIN, executorPkHex: MOCK_EXECUTOR_PK }),
      run: jest.fn(),
    });

    await expect(tool.call(INPUT)).rejects.toThrow(
      /Authorization denied for intent "trade\.equity"/,
    );
    await expect(tool.call(INPUT)).rejects.toThrow(/SIG_INVALID/);
  });

  it("propagates non-A1Error exceptions unchanged", async () => {
    const netErr = new Error("ECONNREFUSED");
    const client = makeClient({ authorize: jest.fn().mockRejectedValue(netErr) } as never);

    const tool = buildLangChainA1Tool({
      name: "execute_trade",
      description: "Execute a trade",
      intentName: "trade.equity",
      client,
      resolveContext: () => ({ chain: MOCK_CHAIN, executorPkHex: MOCK_EXECUTOR_PK }),
      run: jest.fn(),
    });

    await expect(tool.call(INPUT)).rejects.toThrow("ECONNREFUSED");
  });

  it("passes intentParams from context to authorize", async () => {
    const client = makeClient();

    const tool = buildLangChainA1Tool({
      name: "execute_trade",
      description: "Execute a trade",
      intentName: "trade.equity",
      client,
      resolveContext: () => ({
        chain: MOCK_CHAIN,
        executorPkHex: MOCK_EXECUTOR_PK,
        intentParams: { market: "NYSE" },
      }),
      run: async () => "ok",
    });

    await tool.call(INPUT);
    expect(client.authorize).toHaveBeenCalledWith(
      expect.objectContaining({ intentParams: { market: "NYSE" } }),
    );
  });

  it("exposes correct name and description", () => {
    const tool = buildLangChainA1Tool({
      name: "my_tool",
      description: "Does something",
      intentName: "some.intent",
      client: makeClient(),
      resolveContext: () => ({ chain: MOCK_CHAIN, executorPkHex: MOCK_EXECUTOR_PK }),
      run: async () => "ok",
    });
    expect(tool.name).toBe("my_tool");
    expect(tool.description).toBe("Does something");
  });
});

// ── buildLangChainA1BatchTool ────────────────────────────────────────────────

describe("buildLangChainA1BatchTool", () => {
  const INTENT_NAMES = ["query.portfolio", "trade.equity"];

  it("calls authorizeBatch and returns run result when all authorized", async () => {
    const client = makeClient();
    const run = jest.fn().mockResolvedValue("rebalanced");

    const tool = buildLangChainA1BatchTool({
      name: "rebalance",
      description: "Rebalance portfolio",
      intentNames: INTENT_NAMES,
      client,
      resolveContext: () => ({ chain: MOCK_CHAIN, executorPkHex: MOCK_EXECUTOR_PK }),
      run,
    });

    const result = await tool.call("{}");

    expect(client.authorizeBatch).toHaveBeenCalledWith(
      expect.objectContaining({ executorPkHex: MOCK_EXECUTOR_PK }),
    );
    expect(run).toHaveBeenCalledWith("{}", MOCK_BATCH_RESULT);
    expect(result).toBe("rebalanced");
  });

  it("throws a descriptive error listing denied intents when not all authorized", async () => {
    const partialBatch: BatchAuthorizeResult = {
      allAuthorized: false,
      authorizedCount: 1,
      totalCount: 2,
      results: [
        { intentName: "query.portfolio", authorized: true },
        { intentName: "trade.equity", authorized: false, error: "insufficient scope" },
      ],
    };
    const client = makeClient({ authorizeBatch: jest.fn().mockResolvedValue(partialBatch) } as never);

    const tool = buildLangChainA1BatchTool({
      name: "rebalance",
      description: "Rebalance portfolio",
      intentNames: INTENT_NAMES,
      client,
      resolveContext: () => ({ chain: MOCK_CHAIN, executorPkHex: MOCK_EXECUTOR_PK }),
      run: jest.fn(),
    });

    await expect(tool.call("{}")).rejects.toThrow(/trade\.equity.*insufficient scope/);
  });

  it("wraps A1Error from authorizeBatch", async () => {
    const a1Err = new A1Error("chain expired", "CHAIN_EXPIRED", 403);
    const client = makeClient({ authorizeBatch: jest.fn().mockRejectedValue(a1Err) } as never);

    const tool = buildLangChainA1BatchTool({
      name: "rebalance",
      description: "Rebalance portfolio",
      intentNames: INTENT_NAMES,
      client,
      resolveContext: () => ({ chain: MOCK_CHAIN, executorPkHex: MOCK_EXECUTOR_PK }),
      run: jest.fn(),
    });

    await expect(tool.call("{}")).rejects.toThrow(/Batch authorization denied/);
    await expect(tool.call("{}")).rejects.toThrow(/CHAIN_EXPIRED/);
  });
});

// ── buildOpenAIA1Function ────────────────────────────────────────────────────

describe("buildOpenAIA1Function", () => {
  type TradeArgs = { symbol: string; qty: number };
  const PARAMS_SCHEMA = { type: "object", properties: { symbol: { type: "string" } } };

  it("returns a correct definition block", () => {
    const fn = buildOpenAIA1Function<TradeArgs>({
      name: "execute_trade",
      description: "Execute a trade",
      parameters: PARAMS_SCHEMA,
      intentName: "trade.equity",
      client: makeClient(),
      resolveContext: () => ({ chain: MOCK_CHAIN, executorPkHex: MOCK_EXECUTOR_PK }),
      execute: async () => ({ ok: true }),
    });

    expect(fn.definition.type).toBe("function");
    expect(fn.definition.function.name).toBe("execute_trade");
    expect(fn.definition.function.parameters).toEqual(PARAMS_SCHEMA);
  });

  it("authorizes, executes, and returns JSON-stringified result", async () => {
    const client = makeClient();
    const fn = buildOpenAIA1Function<TradeArgs>({
      name: "execute_trade",
      description: "Execute a trade",
      parameters: PARAMS_SCHEMA,
      intentName: "trade.equity",
      client,
      resolveContext: () => ({ chain: MOCK_CHAIN, executorPkHex: MOCK_EXECUTOR_PK }),
      execute: async (_args, auth) => ({ ok: true, depth: auth.chainDepth }),
    });

    const output = await fn.handler(JSON.stringify({ symbol: "AAPL", qty: 10 }));
    expect(JSON.parse(output)).toEqual({ ok: true, depth: 1 });
    expect(client.authorize).toHaveBeenCalledTimes(1);
  });

  it("returns error JSON on invalid arguments input", async () => {
    const fn = buildOpenAIA1Function<TradeArgs>({
      name: "execute_trade",
      description: "Execute a trade",
      parameters: PARAMS_SCHEMA,
      intentName: "trade.equity",
      client: makeClient(),
      resolveContext: () => ({ chain: MOCK_CHAIN, executorPkHex: MOCK_EXECUTOR_PK }),
      execute: async () => ({ ok: true }),
    });

    const output = await fn.handler("not valid json {{{");
    expect(JSON.parse(output)).toHaveProperty("error", "Invalid arguments JSON");
  });

  it("returns error JSON with code when A1Error is thrown from authorize", async () => {
    const a1Err = new A1Error("expired", "CHAIN_EXPIRED", 403);
    const client = makeClient({ authorize: jest.fn().mockRejectedValue(a1Err) } as never);

    const fn = buildOpenAIA1Function<TradeArgs>({
      name: "execute_trade",
      description: "Execute a trade",
      parameters: PARAMS_SCHEMA,
      intentName: "trade.equity",
      client,
      resolveContext: () => ({ chain: MOCK_CHAIN, executorPkHex: MOCK_EXECUTOR_PK }),
      execute: async () => ({ ok: true }),
    });

    const output = await fn.handler(JSON.stringify({ symbol: "MSFT" }));
    const parsed = JSON.parse(output);
    expect(parsed.error).toMatch(/Authorization denied/);
    expect(parsed.code).toBe("CHAIN_EXPIRED");
  });

  it("returns error JSON when execute throws", async () => {
    const fn = buildOpenAIA1Function<TradeArgs>({
      name: "execute_trade",
      description: "Execute a trade",
      parameters: PARAMS_SCHEMA,
      intentName: "trade.equity",
      client: makeClient(),
      resolveContext: () => ({ chain: MOCK_CHAIN, executorPkHex: MOCK_EXECUTOR_PK }),
      execute: async () => { throw new Error("broker offline"); },
    });

    const output = await fn.handler(JSON.stringify({ symbol: "AAPL" }));
    expect(JSON.parse(output)).toEqual({ error: "broker offline" });
  });
});

// ── buildOpenAIA1BatchFunction ───────────────────────────────────────────────

describe("buildOpenAIA1BatchFunction", () => {
  type RebalanceArgs = { risk_level: string };

  it("authorizes batch, executes, and returns JSON result", async () => {
    const client = makeClient();
    const fn = buildOpenAIA1BatchFunction<RebalanceArgs>({
      name: "rebalance_portfolio",
      description: "Rebalance",
      parameters: {},
      intentNames: ["query.portfolio", "trade.equity"],
      client,
      resolveContext: () => ({ chain: MOCK_CHAIN, executorPkHex: MOCK_EXECUTOR_PK }),
      execute: async (_args, auth) => ({ rebalanced: true, intents: auth.authorizedCount }),
    });

    const output = await fn.handler(JSON.stringify({ risk_level: "medium" }));
    expect(JSON.parse(output)).toEqual({ rebalanced: true, intents: 2 });
    expect(client.authorizeBatch).toHaveBeenCalledTimes(1);
  });

  it("returns error JSON when batch is not fully authorized", async () => {
    const partial: BatchAuthorizeResult = {
      allAuthorized: false,
      authorizedCount: 1,
      totalCount: 2,
      results: [
        { intentName: "query.portfolio", authorized: true },
        { intentName: "trade.equity", authorized: false, error: "scope too narrow" },
      ],
    };
    const client = makeClient({ authorizeBatch: jest.fn().mockResolvedValue(partial) } as never);

    const fn = buildOpenAIA1BatchFunction<RebalanceArgs>({
      name: "rebalance_portfolio",
      description: "Rebalance",
      parameters: {},
      intentNames: ["query.portfolio", "trade.equity"],
      client,
      resolveContext: () => ({ chain: MOCK_CHAIN, executorPkHex: MOCK_EXECUTOR_PK }),
      execute: async () => ({ ok: true }),
    });

    const output = await fn.handler(JSON.stringify({ risk_level: "low" }));
    const parsed = JSON.parse(output);
    expect(parsed.error).toMatch(/trade\.equity.*scope too narrow/);
  });

  it("returns error JSON with code on A1Error from authorizeBatch", async () => {
    const a1Err = new A1Error("revoked", "CERT_REVOKED", 403);
    const client = makeClient({ authorizeBatch: jest.fn().mockRejectedValue(a1Err) } as never);

    const fn = buildOpenAIA1BatchFunction<RebalanceArgs>({
      name: "rebalance_portfolio",
      description: "Rebalance",
      parameters: {},
      intentNames: ["query.portfolio", "trade.equity"],
      client,
      resolveContext: () => ({ chain: MOCK_CHAIN, executorPkHex: MOCK_EXECUTOR_PK }),
      execute: async () => ({ ok: true }),
    });

    const output = await fn.handler(JSON.stringify({ risk_level: "high" }));
    const parsed = JSON.parse(output);
    expect(parsed.error).toMatch(/Batch authorization denied/);
    expect(parsed.code).toBe("CERT_REVOKED");
  });

  it("returns error JSON on invalid JSON arguments", async () => {
    const fn = buildOpenAIA1BatchFunction<RebalanceArgs>({
      name: "rebalance_portfolio",
      description: "Rebalance",
      parameters: {},
      intentNames: ["query.portfolio", "trade.equity"],
      client: makeClient(),
      resolveContext: () => ({ chain: MOCK_CHAIN, executorPkHex: MOCK_EXECUTOR_PK }),
      execute: async () => ({ ok: true }),
    });

    const output = await fn.handler("<<<invalid");
    expect(JSON.parse(output)).toEqual({ error: "Invalid arguments JSON" });
  });
});

// ── withA1Guard ─────────────────────────────────────────────────────────────

describe("withA1Guard", () => {
  it("authorizes and passes auth result to the wrapped function", async () => {
    const client = makeClient();
    const fn = jest.fn().mockResolvedValue("done");

    const guarded = withA1Guard({
      intentName: "trade.equity",
      client,
      resolveContext: () => ({ chain: MOCK_CHAIN, executorPkHex: MOCK_EXECUTOR_PK }),
      fn,
    });

    const result = await guarded({ qty: 5 });

    expect(client.authorize).toHaveBeenCalledWith(
      expect.objectContaining({ intentName: "trade.equity" }),
    );
    expect(fn).toHaveBeenCalledWith({ qty: 5 }, MOCK_AUTH_RESULT);
    expect(result).toBe("done");
  });

  it("propagates A1Error from authorize", async () => {
    const a1Err = new A1Error("unauthorized", "UNAUTHORIZED", 403);
    const client = makeClient({ authorize: jest.fn().mockRejectedValue(a1Err) } as never);

    const guarded = withA1Guard({
      intentName: "trade.equity",
      client,
      resolveContext: () => ({ chain: MOCK_CHAIN, executorPkHex: MOCK_EXECUTOR_PK }),
      fn: async () => "should not reach",
    });

    await expect(guarded({})).rejects.toBeInstanceOf(A1Error);
  });
});

// ── withA1BatchGuard ─────────────────────────────────────────────────────────

describe("withA1BatchGuard", () => {
  it("authorizes batch and passes result to wrapped function when all authorized", async () => {
    const client = makeClient();
    const fn = jest.fn().mockResolvedValue("complete");

    const guarded = withA1BatchGuard({
      intentNames: ["query.portfolio", "trade.equity"],
      client,
      resolveContext: () => ({ chain: MOCK_CHAIN, executorPkHex: MOCK_EXECUTOR_PK }),
      fn,
    });

    const result = await guarded({ target: 0.6 });

    expect(client.authorizeBatch).toHaveBeenCalledTimes(1);
    expect(fn).toHaveBeenCalledWith({ target: 0.6 }, MOCK_BATCH_RESULT);
    expect(result).toBe("complete");
  });

  it("throws A1Error listing denied intents when batch is not fully authorized", async () => {
    const partial: BatchAuthorizeResult = {
      allAuthorized: false,
      authorizedCount: 0,
      totalCount: 2,
      results: [
        { intentName: "query.portfolio", authorized: false },
        { intentName: "trade.equity", authorized: false },
      ],
    };
    const client = makeClient({ authorizeBatch: jest.fn().mockResolvedValue(partial) } as never);

    const guarded = withA1BatchGuard({
      intentNames: ["query.portfolio", "trade.equity"],
      client,
      resolveContext: () => ({ chain: MOCK_CHAIN, executorPkHex: MOCK_EXECUTOR_PK }),
      fn: async () => "should not reach",
    });

    await expect(guarded({})).rejects.toBeInstanceOf(A1Error);
    await expect(guarded({})).rejects.toThrow(/query\.portfolio/);
    await expect(guarded({})).rejects.toThrow(/trade\.equity/);
  });
});

// ── A1Error ──────────────────────────────────────────────────────────────────

describe("A1Error", () => {
  it("carries code and status", () => {
    const err = new A1Error("bad chain", "CHAIN_INVALID", 403);
    expect(err.name).toBe("A1Error");
    expect(err.message).toBe("bad chain");
    expect(err.code).toBe("CHAIN_INVALID");
    expect(err.status).toBe(403);
    expect(err).toBeInstanceOf(Error);
  });

  it("can be constructed with optional fields omitted", () => {
    const err = new A1Error("generic");
    expect(err.code).toBeUndefined();
    expect(err.status).toBeUndefined();
  });
});
