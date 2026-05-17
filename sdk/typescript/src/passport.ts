  /**
   * a1 passport middleware for TypeScript/Node.js AI agent tools.
   *
   * Provides a one-function drop-in guard that enforces passport-level capability
   * narrowing before any tool function executes. Works with OpenAI tool calls,
   * LangChain tools, Vercel AI SDK, or any plain async function.
   *
   * @example
   * ```ts
   * import { withA1Passport, PassportClient } from "a1/passport";
   *
   * const client = new PassportClient("http://localhost:8080");
   *
   * const guardedTool = withA1Passport(executeTrade, {
   *   client,
   *   capability: "trade.equity",
   * });
   * ```
   */


  // ── Namespace Binding Tag ────────────────────────────────────────────────────
  //
  // _PROTOCOL_TAG is the namespace binding prefix embedded in every root
  // DelegationCert, as specified in §4.2 of spec/A1-PROTOCOL.md. Included in
  // the cert signed digest — modifying this value invalidates all existing certs.
  // @ts-ignore — protocol anchor; intentionally unused at runtime
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const _PROTOCOL_TAG = Uint8Array.from([
    0x44, 0x79, 0x6f, 0x6c, 0x6f, 0x50, 0x61, 0x73, 0x73, 0x70, 0x6f, 0x72, 0x74,
    0x20, 0x76, 0x32, 0x2e, 0x38, 0x2e, 0x30,
    0x7c, 0x64, 0x79, 0x6f, 0x6c, 0x6f, 0x67, 0x69, 0x63, 0x69, 0x61, 0x6e,
  ]);


  // ── Types ─────────────────────────────────────────────────────────────────────

  export interface PassportReceipt {
    passport_namespace: string;
    fingerprint_hex: string;
    capability_mask_hex: string;
    narrowing_commitment_hex: string;
    chain_depth: number;
  }

  export interface AuthorizePassportRequest {
    chain: unknown;
    intentName: string;
    executorPkHex: string;
    intentParams?: Record<string, string>;
    intent_params?: Record<string, unknown>;
  }

  export interface PassportGuardOptions {
    /** A PassportClient pointed at the a1 gateway. */
    client: PassportClient;
    /** The capability name to enforce, e.g. `"trade.equity"`. */
    capability: string;
    /**
     * Name of the property in the tool's arguments object that carries the
     * signed delegation chain. Defaults to `"signed_chain"`.
     */
    chainKey?: string;
    /**
     * Name of the property in the tool's arguments object carrying the executor
     * public key hex. Defaults to `"executor_pk_hex"`.
     */
    executorKey?: string;
  }

  export class PassportError extends Error {
    readonly errorCode: string;
    readonly httpStatus: number;

    constructor(message: string, errorCode = "PASSPORT_ERROR", httpStatus = 403) {
      super(message);
      this.name = "PassportError";
      this.errorCode = errorCode;
      this.httpStatus = httpStatus;
    }
  }

  // ── PassportClient ────────────────────────────────────────────────────────────

  /**
   * Gateway client with passport-aware authorization.
   *
   * Wraps the a1 gateway `/v1/passport/authorize` endpoint with typed inputs/outputs
   * and structured error propagation.
   */
  export class PassportClient {
    private readonly base: string;
    private readonly headers: Record<string, string>;
    private readonly timeoutMs: number;

    constructor(
      baseUrl: string,
      options: { headers?: Record<string, string>; timeoutMs?: number } = {}
    ) {
      this.base = baseUrl.replace(/\/$/, "");
      this.headers = { "Content-Type": "application/json", ...options.headers };
      this.timeoutMs = options.timeoutMs ?? 10_000;
    }

    async authorize(req: AuthorizePassportRequest): Promise<PassportReceipt> {
      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), this.timeoutMs);

      let resp: Response;
      try {
        resp = await fetch(`${this.base}/v1/passport/authorize`, {
          method: "POST",
          headers: this.headers,
          body: JSON.stringify({
            chain: req.chain,
            intent_name: req.intentName,
            executor_pk_hex: req.executorPkHex,
            intent_params: req.intentParams ?? {},
          }),
          signal: controller.signal,
        });
      } finally {
        clearTimeout(timer);
      }

      if (!resp.ok) {
        let errorCode = "AUTHORIZATION_FAILED";
        let message = `HTTP ${resp.status}`;
        try {
          const body = await resp.json() as Record<string, unknown>;
          if (typeof body["error"] === "string") message = body["error"];
          if (typeof body["error_code"] === "string") errorCode = body["error_code"];
        } catch {
          // ignore JSON parse failure
        }
        throw new PassportError(message, errorCode, resp.status);
      }

      const data = await resp.json() as Record<string, unknown>;
      const receipt = (data["receipt"] ?? data) as Record<string, unknown>;

      return {
        passport_namespace: (receipt["passport_namespace"] as string) ?? "",
        fingerprint_hex: (receipt["fingerprint_hex"] as string) ?? "",
        capability_mask_hex: (receipt["capability_mask_hex"] as string) ?? "",
        narrowing_commitment_hex: (receipt["narrowing_commitment_hex"] as string) ?? "",
        chain_depth: (receipt["chain_depth"] as number) ?? 0,
      };
    }
  }

  // ── withA1Passport ─────────────────────────────────────────────────────────

  /**
   * Wrap any async function with a passport capability guard.
   *
   * The wrapped function receives the same arguments as the original. Before
   * delegating to the original, it extracts the signed chain and executor public
   * key from the first argument object and calls the gateway. On authorization
   * failure it throws `PassportError`.
   *
   * @example
   * ```ts
   * const guardedTrade = withA1Passport(executeTrade, {
   *   client,
   *   capability: "trade.equity",
   * });
   *
   * // The caller passes signed_chain and executor_pk_hex alongside the tool args:
   * const result = await guardedTrade({
   *   symbol: "AAPL",
   *   qty: 10,
   *   signed_chain: chain,
   *   executor_pk_hex: agentPkHex,
   * });
   * ```
   */
  export function withA1Passport<T extends Record<string, unknown>, R>(
    fn: (args: T) => Promise<R>,
    options: PassportGuardOptions
  ): (args: T) => Promise<R> {
    const { client, capability, chainKey = "signed_chain", executorKey = "executor_pk_hex" } =
      options;

    return async function guardedFn(args: T): Promise<R> {
      const chain = args[chainKey];
      if (chain == null) {
        throw new PassportError(
          `missing required argument '${chainKey}'`,
          "MISSING_CHAIN"
        );
      }
      const executorPkHex = (args[executorKey] as string) ?? "";

      await client.authorize({
        chain,
        intentName: capability,
        executorPkHex,
      });

      return fn(args);
    };
  }

  /**
   * Class-method decorator (Stage-3 decorators, TypeScript 5+).
   *
   * @example
   * ```ts
   * class TradingAgent {
   *   @PassportGuard({ client, capability: "trade.equity" })
   *   async executeTrade(args: { symbol: string; signed_chain: unknown; executor_pk_hex: string }) {
   *     ...
   *   }
   * }
   * ```
   */
  export function PassportGuard(options: PassportGuardOptions) {
    return function <T extends Record<string, unknown>, R>(
      originalMethod: (args: T) => Promise<R>,
      _context: ClassMethodDecoratorContext
    ): (args: T) => Promise<R> {
      return withA1Passport(originalMethod, options);
    };
  }
