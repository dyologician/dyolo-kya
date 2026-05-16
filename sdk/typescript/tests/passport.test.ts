import { describe, it, expect, vi, beforeEach } from "@jest/globals";
import {
  PassportClient,
  PassportError,
  PassportReceipt,
  withA1Passport,
  PassportGuard,
} from "../src/passport.js";

// ── Shared fixtures ───────────────────────────────────────────────────────────

const SAMPLE_RECEIPT: PassportReceipt = {
  passport_namespace: "test-agent",
  fingerprint_hex: "de".repeat(32),
  capability_mask_hex: "ff".repeat(32),
  narrowing_commitment_hex: "ab".repeat(32),
  chain_depth: 1,
};

const WRAPPED_RECEIPT = { receipt: SAMPLE_RECEIPT };

function mockFetch(response: object, status = 200): typeof globalThis.fetch {
  return vi.fn().mockResolvedValue({
    ok: status < 400,
    status,
    json: async () => response,
  } as Response);
}

// ── PassportClient ────────────────────────────────────────────────────────────

describe("PassportClient.authorize", () => {
  let client: PassportClient;

  beforeEach(() => {
    client = new PassportClient("http://localhost:8080");
  });

  it("returns a PassportReceipt on 200 with nested receipt", async () => {
    globalThis.fetch = mockFetch(WRAPPED_RECEIPT);

    const receipt = await client.authorize({
      chain: { certs: [] },
      intent_name: "trade.equity",
      executor_pk_hex: "aa".repeat(32),
    });

    expect(receipt.passport_namespace).toBe("test-agent");
    expect(receipt.chain_depth).toBe(1);
    expect(receipt.fingerprint_hex).toBe("de".repeat(32));
  });

  it("returns a PassportReceipt on 200 with flat receipt", async () => {
    globalThis.fetch = mockFetch(SAMPLE_RECEIPT);

    const receipt = await client.authorize({
      chain: {},
      intent_name: "read.data",
      executor_pk_hex: "",
    });

    expect(receipt.passport_namespace).toBe("test-agent");
    expect(receipt.chain_depth).toBe(1);
  });

  it("throws PassportError on non-200 with structured error body", async () => {
    globalThis.fetch = mockFetch(
      { error: "scope violation", error_code: "SCOPE_VIOLATION" },
      403
    );

    await expect(
      client.authorize({ chain: {}, intent_name: "trade.equity", executor_pk_hex: "" })
    ).rejects.toMatchObject({
      errorCode: "SCOPE_VIOLATION",
      httpStatus: 403,
      message: "scope violation",
    });
  });

  it("throws PassportError on non-200 with no JSON body", async () => {
    globalThis.fetch = vi.fn().mockResolvedValue({
      ok: false,
      status: 503,
      json: async () => { throw new Error("not json"); },
    } as unknown as Response);

    await expect(
      client.authorize({ chain: {}, intent_name: "trade.equity", executor_pk_hex: "" })
    ).rejects.toBeInstanceOf(PassportError);
  });

  it("includes intent_params in the request body when provided", async () => {
    let captured: unknown;
    globalThis.fetch = vi.fn().mockImplementation(async (_url: unknown, init: RequestInit) => {
      captured = JSON.parse(init.body as string);
      return { ok: true, status: 200, json: async () => WRAPPED_RECEIPT } as Response;
    });

    await client.authorize({
      chain: {},
      intent_name: "trade.equity",
      executor_pk_hex: "pk",
      intent_params: { symbol: "AAPL" },
    });

    expect((captured as Record<string, unknown>)["intent_params"]).toEqual({ symbol: "AAPL" });
    expect((captured as Record<string, unknown>)["executor_pk_hex"]).toBe("pk");
  });

  it("respects custom headers and timeoutMs constructor options", () => {
    const custom = new PassportClient("http://host", {
      headers: { Authorization: "Bearer tok" },
      timeoutMs: 5000,
    });
    expect(custom).toBeDefined();
  });
});

// ── PassportError ─────────────────────────────────────────────────────────────

describe("PassportError", () => {
  it("is an instance of Error", () => {
    const e = new PassportError("denied", "DENIED", 403);
    expect(e).toBeInstanceOf(Error);
    expect(e.name).toBe("PassportError");
    expect(e.errorCode).toBe("DENIED");
    expect(e.httpStatus).toBe(403);
    expect(e.message).toBe("denied");
  });

  it("uses defaults when optional args are omitted", () => {
    const e = new PassportError("oops");
    expect(e.errorCode).toBe("PASSPORT_ERROR");
    expect(e.httpStatus).toBe(403);
  });
});

// ── withA1Passport ─────────────────────────────────────────────────────────

describe("withA1Passport", () => {
  type TradeArgs = {
    symbol: string;
    signed_chain: unknown;
    executor_pk_hex?: string;
  };

  it("calls the gateway then executes the wrapped function on success", async () => {
    globalThis.fetch = mockFetch(WRAPPED_RECEIPT);

    const client = new PassportClient("http://localhost:8080");
    const callLog: string[] = [];

    const guarded = withA1Passport(
      async (args: TradeArgs) => {
        callLog.push("executed:" + args.symbol);
        return "filled";
      },
      { client, capability: "trade.equity" }
    );

    const result = await guarded({
      symbol: "AAPL",
      signed_chain: { certs: [] },
      executor_pk_hex: "aa".repeat(32),
    });

    expect(result).toBe("filled");
    expect(callLog).toEqual(["executed:AAPL"]);
  });

  it("blocks the wrapped function when gateway rejects", async () => {
    globalThis.fetch = mockFetch({ error: "denied", error_code: "SCOPE_VIOLATION" }, 403);

    const client = new PassportClient("http://localhost:8080");
    const callLog: string[] = [];

    const guarded = withA1Passport(
      async (_args: TradeArgs) => {
        callLog.push("should-not-run");
        return "bad";
      },
      { client, capability: "trade.equity" }
    );

    await expect(
      guarded({ symbol: "AAPL", signed_chain: {} })
    ).rejects.toBeInstanceOf(PassportError);

    expect(callLog).toHaveLength(0);
  });

  it("throws PassportError when signed_chain is missing", async () => {
    const client = new PassportClient("http://localhost:8080");

    const guarded = withA1Passport(
      async (_args: Record<string, unknown>) => "ok",
      { client, capability: "trade.equity", chainKey: "signed_chain" }
    );

    await expect(guarded({ symbol: "AAPL" })).rejects.toMatchObject({
      errorCode: "MISSING_CHAIN",
    });
  });

  it("respects custom chainKey and executorKey options", async () => {
    let capturedChain: unknown;
    globalThis.fetch = vi.fn().mockImplementation(async (_url: unknown, init: RequestInit) => {
      capturedChain = JSON.parse(init.body as string);
      return { ok: true, status: 200, json: async () => WRAPPED_RECEIPT } as Response;
    });

    const client = new PassportClient("http://localhost:8080");

    const guarded = withA1Passport(
      async (_args: Record<string, unknown>) => "ok",
      { client, capability: "trade.equity", chainKey: "myChain", executorKey: "myPK" }
    );

    await guarded({ myChain: { certs: [] }, myPK: "pk123" });

    const body = capturedChain as Record<string, unknown>;
    expect(body["executor_pk_hex"]).toBe("pk123");
    expect(body["chain"]).toEqual({ certs: [] });
  });

  it("passes the original arguments unchanged to the wrapped function", async () => {
    globalThis.fetch = mockFetch(WRAPPED_RECEIPT);
    const client = new PassportClient("http://localhost:8080");

    let received: TradeArgs | undefined;

    const guarded = withA1Passport(
      async (args: TradeArgs) => {
        received = args;
        return "ok";
      },
      { client, capability: "trade.equity" }
    );

    const input: TradeArgs = { symbol: "GOOG", signed_chain: { v: 1 }, executor_pk_hex: "ee" };
    await guarded(input);

    expect(received).toEqual(input);
  });
});

// ── PassportGuard decorator ───────────────────────────────────────────────────

describe("PassportGuard class-method decorator", () => {
  it("is exported and callable", () => {
    expect(typeof PassportGuard).toBe("function");
  });
});