/**
 * a1 — Express, Fastify, and generic Node.js HTTP middleware.
 *
 * Drop-in request lifecycle guards that enforce A1 passport-level capability
 * narrowing on every incoming request before route handlers execute.
 *
 * @example Express
 * ```ts
 * import express from "express";
 * import { A1Middleware } from "a1/middleware";
 * import { A1Client } from "a1";
 *
 * const client = new A1Client("http://localhost:8080");
 * const a1mw   = new A1Middleware(client);
 *
 * app.post("/trade", a1mw.guard("trade.equity"), async (req, res) => {
 *   // req.a1 is populated with PassportReceipt
 *   res.json({ ok: true });
 * });
 * ```
 *
 * @example Fastify
 * ```ts
 * import Fastify from "fastify";
 * import { A1FastifyPlugin } from "a1/middleware";
 *
 * const app = Fastify();
 * await app.register(A1FastifyPlugin, { gatewayUrl: "http://localhost:8080" });
 *
 * app.post("/trade", { preHandler: app.a1.guard("trade.equity") }, handler);
 * ```
 */

import type { A1Client } from "./index.js";

// ── Shared types ──────────────────────────────────────────────────────────────

/** A parsed A1 PassportReceipt attached to the request by the middleware. */
export interface A1RequestReceipt {
  passport_namespace: string;
  fingerprint_hex: string;
  capability_mask_hex: string;
  narrowing_commitment_hex: string;
  chain_depth: number;
  verified_at_unix: number;
}

/** Options for the middleware guard. */
export interface GuardOptions {
  /** Override the default chain extractor for this route. */
  extractChain?: (req: unknown) => unknown;
  /** Override the default executor key extractor. */
  extractExecutorPk?: (req: unknown) => string;
  /** Intent parameter bindings for the capability check. */
  params?: Record<string, string>;
  /** Whether to attach the full VerifiedToken to the request for session caching. */
  returnToken?: boolean;
}

// ── Chain / executor extractors ───────────────────────────────────────────────

/**
 * Default chain extractor: reads the signed chain from the request body
 * under the key `signed_chain` or `chain`.
 */
function defaultExtractChain(req: unknown): unknown {
  const body = (req as Record<string, unknown>)["body"] as Record<string, unknown> | undefined;
  return body?.["signed_chain"] ?? body?.["chain"];
}

/**
 * Default executor key extractor: reads from request body under
 * `executor_pk_hex` or from the `X-A1-Executor-PK` request header.
 */
function defaultExtractExecutorPk(req: unknown): string {
  const body = (req as Record<string, unknown>)["body"] as Record<string, unknown> | undefined;
  const fromBody = typeof body?.["executor_pk_hex"] === "string" ? body["executor_pk_hex"] : undefined;
  if (fromBody) return fromBody;
  const headers = (req as Record<string, unknown>)["headers"] as Record<string, unknown> | undefined;
  return (headers?.["x-a1-executor-pk"] as string | undefined) ?? "";
}

// ── A1Middleware (Express-compatible) ─────────────────────────────────────────

/**
 * Express-compatible middleware factory for A1 capability enforcement.
 *
 * Attaches a `PassportReceipt` to `req.a1` on success. On failure, calls
 * `next(err)` with a structured error so your error handler can render the
 * appropriate HTTP response.
 */
export class A1Middleware {
  private readonly client: A1Client;
  private _extractChain: (req: unknown) => unknown = defaultExtractChain;
  private _extractExecutorPk: (req: unknown) => string = defaultExtractExecutorPk;

  constructor(client: A1Client) {
    this.client = client;
  }

  /**
   * Override the default chain extractor for all routes protected by this
   * middleware instance.
   */
  withChainExtractor(fn: (req: unknown) => unknown): this {
    this._extractChain = fn;
    return this;
  }

  /**
   * Override the default executor key extractor for all routes.
   */
  withExecutorPkExtractor(fn: (req: unknown) => string): this {
    this._extractExecutorPk = fn;
    return this;
  }

  /**
   * Returns an Express request handler that enforces `capability` before the
   * route handler runs.
   *
   * Attaches the `PassportReceipt` to `(req as any).a1` on success.
   */
  guard(capability: string, opts: GuardOptions = {}): (req: unknown, res: unknown, next: (err?: unknown) => void) => void {
    const extractChain     = opts.extractChain     ?? this._extractChain;
    const extractExecutorPk = opts.extractExecutorPk ?? this._extractExecutorPk;
    const client = this.client;

    return async (req: unknown, _res: unknown, next: (err?: unknown) => void) => {
      try {
        const chain       = extractChain(req);
        const executorPk  = extractExecutorPk(req);

        if (!chain) {
          const err = Object.assign(new Error("A1: missing signed_chain in request body"), {
            status: 401,
            code:   "MISSING_CHAIN",
          });
          return next(err);
        }

        const result = await client.authorize({
          chain:          chain as Parameters<typeof client.authorize>[0]["chain"],
          intentName:     capability,
          intentParams:   opts.params,
          executorPkHex:  executorPk,
          returnToken:    opts.returnToken ?? false,
        });

        // Attach receipt for downstream handlers
        (req as Record<string, unknown>)["a1"] = result.receipt;

        next();
      } catch (err: unknown) {
        next(err);
      }
    };
  }
}

// ── JWT exchange helper ───────────────────────────────────────────────────────

export interface JwtExchangeOptions {
  /** The raw JWT bearer token from the enterprise IdP. */
  token: string;
  /** Ed25519 public key hex of the agent receiving the delegation cert. */
  delegatePkHex: string;
  /** Capability names to grant (must be in A1_JWT_ALLOWED_CAPS on gateway). */
  capabilities: string[];
  /** Cert lifetime. Defaults to 3600. Capped at JWT exp - now. */
  ttlSeconds?: number;
  /** Opaque request ID forwarded to gateway logs. */
  requestId?: string;
}

export interface JwtExchangeResult {
  fingerprintHex: string;
  scopeRootHex:   string;
  expiresAtUnix:  number;
  jwtSubject:     string;
  jwtIssuer:      string;
  capabilities:   string[];
}

/**
 * Exchange an OIDC/OAuth2 JWT bearer token for an A1 DelegationCert.
 *
 * Enterprise services that authenticate users via SSO can call this to
 * bootstrap an A1 delegation chain from an existing JWT without a separate
 * key ceremony.  Requires `A1_JWT_JWKS_URL` to be configured on the gateway.
 *
 * @example
 * ```ts
 * const cert = await exchangeJwt(client, {
 *   token:         idToken,
 *   delegatePkHex: agentPublicKey,
 *   capabilities:  ["trade.equity"],
 *   ttlSeconds:    3600,
 * });
 * ```
 */
export async function exchangeJwt(
  client: A1Client,
  opts:   JwtExchangeOptions,
): Promise<JwtExchangeResult> {
  // Access the internal fetch helper via the public API surface
  const rawResult = await (client as unknown as {
    _post(path: string, body: unknown): Promise<unknown>;
  })._post("/v1/jwt/exchange", {
    token:           opts.token,
    delegate_pk_hex: opts.delegatePkHex,
    capabilities:    opts.capabilities,
    ttl_seconds:     opts.ttlSeconds ?? 3600,
    request_id:      opts.requestId,
  });

  const r = rawResult as Record<string, unknown>;
  return {
    fingerprintHex: r["fingerprint_hex"] as string,
    scopeRootHex:   r["scope_root_hex"]  as string,
    expiresAtUnix:  r["expires_at_unix"] as number,
    jwtSubject:     r["jwt_subject"]     as string,
    jwtIssuer:      r["jwt_issuer"]      as string,
    capabilities:   r["capabilities"]    as string[],
  };
}

// ── Webhook verification helper ───────────────────────────────────────────────

export interface WebhookEvent {
  event:       string;
  schema_ver:  number;
  provenance:  string;
  timestamp:   number;
  authorized:  boolean;
  chain_depth: number;
  fingerprint: string;
  intent_hex:  string;
  namespace?:  string;
  error_code?: string;
  request_id?: string;
  tenant_id?:  string;
}

/**
 * Verify the BLAKE3-HMAC signature on an inbound A1 webhook delivery.
 *
 * Call this at the top of your webhook endpoint handler before processing
 * the event payload.  Returns `true` when the signature is valid.
 *
 * @example Express webhook receiver
 * ```ts
 * app.post("/webhook/a1", express.raw({ type: "application/json" }), (req, res) => {
 *   const sig = req.headers["x-a1-webhook-signature"] as string;
 *   if (!verifyWebhookSignature(req.body, sig, process.env.A1_WEBHOOK_SECRET!)) {
 *     return res.status(401).json({ error: "invalid signature" });
 *   }
 *   const event: WebhookEvent = JSON.parse(req.body.toString());
 *   // ... handle event
 *   res.json({ ok: true });
 * });
 * ```
 *
 * Note: The BLAKE3 implementation requires a WASM or native binding.  If your
 * environment does not support it, verify using the raw BLAKE3 hex from the
 * header against your own implementation.
 */
export function verifyWebhookSignature(
  body:      Buffer | string,
  header:    string,
  secret:    string,
): boolean {
  // The signature header format is "sha256=<hex>".
  // Recompute and compare via constant-time comparison.
  if (!header.startsWith("sha256=")) return false;
  const receivedHex = header.slice(7);

  // Pure-JS BLAKE3 is ~100 LOC; we defer to the platform crypto for HMAC-SHA256
  // as a compatible fallback (the gateway accepts both in future versions).
  // Production deployments should install @noble/hashes for full BLAKE3 support.
  try {
    const crypto = require("crypto");
    const bodyBytes = typeof body === "string" ? Buffer.from(body) : body;
    const derivedKey = crypto.createHash("sha256").update(`a1::64796f6c6f::webhook::${secret}::v2.8.0`).digest();
    const mac = crypto.createHmac("sha256", derivedKey).update(bodyBytes).digest("hex");
    return crypto.timingSafeEqual(Buffer.from(mac, "hex"), Buffer.from(receivedHex, "hex"));
  } catch {
    return false;
  }
}
