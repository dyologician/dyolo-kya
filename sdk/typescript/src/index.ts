/**
 * dyolo-kya — TypeScript/Node.js client for the dyolo-kya AI agent
 * authorization protocol.
 *
 * Wraps the dyolo-kya-gateway REST API. No Rust toolchain required.
 *
 * @example
 * ```ts
 * import { KyaClient } from "dyolo-kya";
 *
 * const kya = new KyaClient("http://localhost:8080");
 *
 * const result = await kya.authorize({
 *   chain: signedChain,
 *   intentName: "trade.equity",
 *   intentParams: { symbol: "AAPL" },
 *   executorPkHex: "...",
 * });
 * ```
 */

// ── Internal runtime validators ───────────────────────────────────────────────

type Validator<T> = (raw: unknown) => T;

function obj(raw: unknown, ctx: string): Record<string, unknown> {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw))
    throw new Error(`${ctx}: expected object, got ${typeof raw}`);
  return raw as Record<string, unknown>;
}
function str(raw: unknown, ctx: string): string {
  if (typeof raw !== "string") throw new Error(`${ctx}: expected string`);
  return raw;
}
function num(raw: unknown, ctx: string): number {
  if (typeof raw !== "number") throw new Error(`${ctx}: expected number`);
  return raw;
}
function bool(raw: unknown, ctx: string): boolean {
  if (typeof raw !== "boolean") throw new Error(`${ctx}: expected boolean`);
  return raw;
}
function arr<T>(raw: unknown, ctx: string, item: (v: unknown, c: string) => T): T[] {
  if (!Array.isArray(raw)) throw new Error(`${ctx}: expected array`);
  return raw.map((v, i) => item(v, `${ctx}[${i}]`));
}
function optStr(raw: unknown, _ctx: string): string | undefined {
  return typeof raw === "string" ? raw : undefined;
}
function optBool(raw: unknown, _ctx: string): boolean | undefined {
  return typeof raw === "boolean" ? raw : undefined;
}

// ── Validators ────────────────────────────────────────────────────────────────

const WellKnownConfigSchema: Validator<WellKnownConfig> = (raw) => {
  const o = obj(raw, "WellKnownConfig");
  return {
    issuer:                     str(o["issuer"], "issuer"),
    gateway_signing_pk_hex:     str(o["gateway_signing_pk_hex"], "gateway_signing_pk_hex"),
    authorization_endpoint:     str(o["authorization_endpoint"], "authorization_endpoint"),
    batch_authorize_endpoint:   str(o["batch_authorize_endpoint"], "batch_authorize_endpoint"),
    cert_issuance_endpoint:     str(o["cert_issuance_endpoint"], "cert_issuance_endpoint"),
    cert_revoke_endpoint:       str(o["cert_revoke_endpoint"], "cert_revoke_endpoint"),
    cert_revoke_batch_endpoint: str(o["cert_revoke_batch_endpoint"], "cert_revoke_batch_endpoint"),
    token_verify_endpoint:      str(o["token_verify_endpoint"], "token_verify_endpoint"),
    kya_version:                str(o["kya_version"], "kya_version"),
    supported_algorithms:       arr(o["supported_algorithms"], "supported_algorithms", (v, c) => str(v, c)),
  };
};

const IssueCertRawResultSchema: Validator<{ fingerprint_hex: string; scope_root_hex: string }> = (raw) => {
  const o = obj(raw, "IssueCertResult");
  return {
    fingerprint_hex: str(o["fingerprint_hex"], "fingerprint_hex"),
    scope_root_hex:  str(o["scope_root_hex"], "scope_root_hex"),
  };
};

const RevokeBatchRawResultSchema: Validator<{ revoked_count: number; failed: string[] }> = (raw) => {
  const o = obj(raw, "RevokeBatchResult");
  return {
    revoked_count: num(o["revoked_count"], "revoked_count"),
    failed:        arr(o["failed"], "failed", (v, c) => str(v, c)),
  };
};

const CertStatusSchema: Validator<CertStatus> = (raw) => {
  const o = obj(raw, "CertStatus");
  return {
    fingerprint: str(o["fingerprint"], "fingerprint"),
    revoked:     bool(o["revoked"], "revoked"),
  };
};

const VerifiedTokenSchema: Validator<VerifiedToken> = (raw) => {
  const o = obj(raw, "VerifiedToken");
  const r = obj(o["receipt"], "receipt");
  return {
    receipt: {
      chain_depth:          num(r["chain_depth"], "receipt.chain_depth"),
      verified_scope_root:  str(r["verified_scope_root"], "receipt.verified_scope_root"),
      intent:               str(r["intent"], "receipt.intent"),
      verified_at_unix:     num(r["verified_at_unix"], "receipt.verified_at_unix"),
      chain_fingerprint:    str(r["chain_fingerprint"], "receipt.chain_fingerprint"),
    },
    mac: str(o["mac"], "mac"),
  };
};

const AuthorizeRawResultSchema: Validator<{
  authorized: boolean;
  chain_depth: number;
  chain_fingerprint: string;
  verified_at_unix: number;
  token?: VerifiedToken;
}> = (raw) => {
  const o = obj(raw, "AuthorizeResult");
  const result: ReturnType<typeof AuthorizeRawResultSchema> = {
    authorized:       bool(o["authorized"], "authorized"),
    chain_depth:      num(o["chain_depth"], "chain_depth"),
    chain_fingerprint: str(o["chain_fingerprint"], "chain_fingerprint"),
    verified_at_unix: num(o["verified_at_unix"], "verified_at_unix"),
  };
  if (o["token"] !== undefined && o["token"] !== null) {
    result.token = VerifiedTokenSchema(o["token"]);
  }
  return result;
};

const BatchItemRawSchema: Validator<{
  intent_name: string;
  authorized: boolean;
  chain_fingerprint?: string;
  error?: string;
  error_code?: string;
}> = (raw) => {
  const o = obj(raw, "BatchItem");
  return {
    intent_name:       str(o["intent_name"], "intent_name"),
    authorized:        bool(o["authorized"], "authorized"),
    chain_fingerprint: optStr(o["chain_fingerprint"], "chain_fingerprint"),
    error:             optStr(o["error"], "error"),
    error_code:        optStr(o["error_code"], "error_code"),
  };
};

const BatchAuthorizeRawResultSchema: Validator<{
  all_authorized: boolean;
  authorized_count: number;
  total_count: number;
  results: ReturnType<typeof BatchItemRawSchema>[];
}> = (raw) => {
  const o = obj(raw, "BatchAuthorizeResult");
  return {
    all_authorized:   bool(o["all_authorized"], "all_authorized"),
    authorized_count: num(o["authorized_count"], "authorized_count"),
    total_count:      num(o["total_count"], "total_count"),
    results:          arr(o["results"], "results", (v, c) => BatchItemRawSchema(v) as ReturnType<typeof BatchItemRawSchema>),
  };
};

const VerifyTokenRawResultSchema: Validator<{
  valid: boolean;
  chain_depth: number;
  chain_fingerprint: string;
  verified_at_unix: number;
}> = (raw) => {
  const o = obj(raw, "VerifyTokenResult");
  return {
    valid:             bool(o["valid"], "valid"),
    chain_depth:       num(o["chain_depth"], "chain_depth"),
    chain_fingerprint: str(o["chain_fingerprint"], "chain_fingerprint"),
    verified_at_unix:  num(o["verified_at_unix"], "verified_at_unix"),
  };
};

const HealthRawResultSchema: Validator<{
  status: string;
  signing_pk_hex?: string;
  version: string;
}> = (raw) => {
  const o = obj(raw, "HealthResult");
  return {
    status:         str(o["status"], "status"),
    signing_pk_hex: optStr(o["signing_pk_hex"], "signing_pk_hex"),
    version:        str(o["version"], "version"),
  };
};

const EmptyResponseSchema: Validator<Record<string, unknown>> = (raw) => {
  return obj(raw, "EmptyResponse");
};

export class KyaError extends Error {
  readonly code: string | undefined;
  readonly status: number | undefined;

  constructor(message: string, code?: string, status?: number) {
    super(message);
    this.name = "KyaError";
    this.code = code;
    this.status = status;
  }
}

// ── Wire types ────────────────────────────────────────────────────────────────

export interface DelegationCert {
  version: number;
  delegator_pk: string;
  delegate_pk: string;
  scope_root: string;
  nonce: string;
  issued_at: number;
  expiration_unix: number;
  max_depth: number;
  extensions?: Record<string, unknown>;
  signature: string;
}

export interface SignedChain {
  version: number;
  principal_pk: string;
  principal_scope: string;
  certs: DelegationCert[];
}

export interface VerifiedToken {
  receipt: {
    chain_depth: number;
    verified_scope_root: string;
    intent: string;
    verified_at_unix: number;
    chain_fingerprint: string;
  };
  mac: string;
}

export interface IntentSpec {
  name: string;
  params?: Record<string, string>;
}

// ── Request / response shapes ─────────────────────────────────────────────────

export interface IssueCertOptions {
  delegatePkHex: string;
  intents: IntentSpec[];
  ttlSeconds?: number;
  maxDepth?: number;
  extensions?: Record<string, unknown>;
}

export interface IssueCertResult {
  fingerprintHex: string;
  scopeRootHex: string;
}

export interface AuthorizeOptions {
  chain: SignedChain;
  intentName: string;
  intentParams?: Record<string, string>;
  executorPkHex: string;
  returnToken?: boolean;
  requestId?: string;
}

export interface AuthorizeResult {
  authorized: boolean;
  chainDepth: number;
  chainFingerprint: string;
  verifiedAtUnix: number;
  token?: VerifiedToken;
}

export interface BatchIntentSpec {
  name: string;
  params?: Record<string, string>;
}

export interface BatchAuthorizeOptions {
  chain: SignedChain;
  executorPkHex: string;
  intents: BatchIntentSpec[];
}

export interface BatchItem {
  intentName: string;
  authorized: boolean;
  chainFingerprint?: string;
  error?: string;
  errorCode?: string;
}

export interface BatchAuthorizeResult {
  allAuthorized: boolean;
  authorizedCount: number;
  totalCount: number;
  results: BatchItem[];
}

export interface RevokeBatchResult {
  revokedCount: number;
  failed: string[];
}

export interface CertStatus {
  fingerprint: string;
  revoked: boolean;
}

export interface VerifyTokenOptions {
  token: VerifiedToken;
}

export interface VerifyTokenResult {
  valid: boolean;
  chainDepth: number;
  chainFingerprint: string;
  verifiedAtUnix: number;
}

export interface HealthResult {
  status: string;
  signing_pk_hex: string;
  version: string;
}

export interface WellKnownConfig {
  issuer: string;
  gateway_signing_pk_hex: string;
  authorization_endpoint: string;
  batch_authorize_endpoint: string;
  cert_issuance_endpoint: string;
  cert_revoke_endpoint: string;
  cert_revoke_batch_endpoint: string;
  token_verify_endpoint: string;
  kya_version: string;
  supported_algorithms: string[];
}

// ── Client options ────────────────────────────────────────────────────────────

export interface RetryOptions {
  /** Maximum number of retry attempts. Default: 3. */
  maxRetries?: number;
  /** Initial delay in ms. Default: 500. */
  initialDelayMs?: number;
  /** Exponential factor. Default: 2. */
  backoffFactor?: number;
}

export interface KyaClientOptions {
  /** Request timeout in milliseconds. Default: 10000. */
  timeoutMs?: number;
  /** Static headers added to every request (e.g. Authorization). */
  headers?: Record<string, string>;
  /** Retry configuration for transient errors. */
  retry?: RetryOptions;
  /** Enable circuit breaker to prevent cascade failures. */
  enableCircuitBreaker?: boolean;
}

// ── KyaClient ─────────────────────────────────────────────────────────────────

export class KyaClient {
  private readonly base: string;
  private readonly timeoutMs: number;
  private readonly extraHeaders: Record<string, string>;
  private readonly retry: Required<RetryOptions>;
  
  // Minimal Circuit Breaker state
  private cbFailureCount = 0;
  private cbLastFailureTime = 0;
  private readonly CB_THRESHOLD = 5;
  private readonly CB_RESET_TIMEOUT_MS = 30_000;

  constructor(baseUrl: string, opts: KyaClientOptions = {}) {
    this.base = baseUrl.replace(/\/$/, "");
    this.timeoutMs = opts.timeoutMs ?? 10_000;
    this.extraHeaders = opts.headers ?? {};
    this.retry = {
      maxRetries: opts.retry?.maxRetries ?? 3,
      initialDelayMs: opts.retry?.initialDelayMs ?? 500,
      backoffFactor: opts.retry?.backoffFactor ?? 2,
    };
  }

  /** Fetch the gateway's OIDC-style discovery document. */
  async wellKnown(): Promise<WellKnownConfig> {
    return this.get("/.well-known/kya-configuration", WellKnownConfigSchema);
  }

  /** Issue a delegation certificate from the gateway's signing key. */
  async issueCert(opts: IssueCertOptions): Promise<IssueCertResult> {
    const body = {
      delegate_pk_hex: opts.delegatePkHex,
      intents: opts.intents,
      ...(opts.ttlSeconds !== undefined ? { ttl_seconds: opts.ttlSeconds } : {}),
      ...(opts.maxDepth !== undefined ? { max_depth: opts.maxDepth } : {}),
      ...(opts.extensions ? { extensions: opts.extensions } : {}),
    };
    const raw = await this.post("/v1/cert/issue", IssueCertRawResultSchema, body);
    return {
      fingerprintHex: raw.fingerprint_hex,
      scopeRootHex: raw.scope_root_hex,
    };
  }

  /** Revoke a single certificate by its fingerprint. */
  async revokeCert(fingerprintHex: string): Promise<void> {
    await this.post("/v1/cert/revoke", EmptyResponseSchema, { fingerprint_hex: fingerprintHex });
  }

  /** Revoke multiple certificates in a single round-trip. */
  async revokeCertsBatch(fingerprints: string[]): Promise<RevokeBatchResult> {
    const raw = await this.post(
      "/v1/cert/revoke-batch",
      RevokeBatchRawResultSchema,
      { fingerprints },
    );
    return { revokedCount: raw.revoked_count, failed: raw.failed };
  }

  /** Inspect the revocation status of a certificate. */
  async inspectCert(fingerprintHex: string): Promise<CertStatus> {
    return this.get(
      `/v1/cert/${encodeURIComponent(fingerprintHex)}`,
      CertStatusSchema
    );
  }

  /** Authorize a single agent intent against a delegation chain. */
  async authorize(opts: AuthorizeOptions): Promise<AuthorizeResult> {
    const body = {
      chain: opts.chain,
      intent_name: opts.intentName,
      executor_pk_hex: opts.executorPkHex,
      ...(opts.intentParams ? { intent_params: opts.intentParams } : {}),
      ...(opts.returnToken !== undefined ? { return_token: opts.returnToken } : {}),
      ...(opts.requestId ? { request_id: opts.requestId } : {}),
    };
    const raw = await this.post("/v1/authorize", AuthorizeRawResultSchema, body);
    return {
      authorized: raw.authorized,
      chainDepth: raw.chain_depth,
      chainFingerprint: raw.chain_fingerprint,
      verifiedAtUnix: raw.verified_at_unix,
      token: raw.token,
    };
  }

  /**
   * Authorize multiple intents atomically against a single delegation chain.
   *
   * If any intent fails verification, no nonces are consumed and the full
   * batch is rejected. Check `allAuthorized` before acting on results.
   */
  async authorizeBatch(opts: BatchAuthorizeOptions): Promise<BatchAuthorizeResult> {
    const body = {
      chain: opts.chain,
      executor_pk_hex: opts.executorPkHex,
      intents: opts.intents.map((i) => ({
        name: i.name,
        ...(i.params ? { params: i.params } : {}),
      })),
    };
    const raw = await this.post("/v1/authorize/batch", BatchAuthorizeRawResultSchema, body);

    return {
      allAuthorized: raw.all_authorized,
      authorizedCount: raw.authorized_count,
      totalCount: raw.total_count,
      results: raw.results.map((r) => ({
        intentName: r.intent_name,
        authorized: r.authorized,
        chainFingerprint: r.chain_fingerprint,
        error: r.error,
        errorCode: r.error_code,
      })),
    };
  }

  /** Verify the HMAC on a VerifiedToken issued by a previous authorize call. */
  async verifyToken(opts: VerifyTokenOptions): Promise<VerifyTokenResult> {
    const raw = await this.post("/v1/token/verify", VerifyTokenRawResultSchema, opts.token);
    return {
      valid: raw.valid,
      chainDepth: raw.chain_depth,
      chainFingerprint: raw.chain_fingerprint,
      verifiedAtUnix: raw.verified_at_unix,
    };
  }

  /** Health check — returns the gateway's signing public key and version. */
  async health(): Promise<HealthResult> {
    const raw = await this.get("/health", HealthRawResultSchema);
    return {
      status: raw.status,
      signing_pk_hex: raw.signing_pk_hex ?? "",
      version: raw.version,
    };
  }

  // ── Private helpers ──────────────────────────────────────────────────────

  private async post<T>(path: string, schema: Validator<T>, body?: unknown): Promise<T> {
    return this.request<T>("POST", path, schema, body);
  }

  private async get<T>(path: string, schema: Validator<T>): Promise<T> {
    return this.request<T>("GET", path, schema, undefined);
  }

  private async request<T>(method: string, path: string, schema: Validator<T>, body: unknown): Promise<T> {
    let attempt = 0;
    
    while (true) {
      // Circuit Breaker Check
      if (this.cbFailureCount >= this.CB_THRESHOLD) {
        if (Date.now() - this.cbLastFailureTime < this.CB_RESET_TIMEOUT_MS) {
          throw new KyaError("Circuit breaker is open", "CIRCUIT_BREAKER_OPEN", 503);
        }
        this.cbFailureCount = 0; // Attempt half-open reset
      }

      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), this.timeoutMs);

      try {
        const response = await fetch(`${this.base}${path}`, {
          method,
          headers: {
            "Content-Type": "application/json",
            Accept: "application/json",
            ...this.extraHeaders,
          },
          signal: controller.signal,
          ...(body !== undefined ? { body: JSON.stringify(body) } : {}),
        });

        const text = await response.text();
        let parsed: unknown;
        try {
          parsed = text ? JSON.parse(text) : {};
        } catch {
          throw new KyaError(`Gateway returned non-JSON response (status ${response.status})`);
        }

        if (!response.ok) {
          const obj = parsed as Record<string, unknown>;
          const error = new KyaError(
            String(obj.error ?? `HTTP ${response.status}`),
            typeof obj.error_code === "string" ? obj.error_code : undefined,
            response.status,
          );

          const isTransient = [429, 502, 503, 504].includes(response.status);

          // Update circuit breaker BEFORE retry logic for critical overload status codes
          if (isTransient) {
            this.recordFailure();
            // Fail fast if the breaker trips *during* the retry loop
            if (this.cbFailureCount >= this.CB_THRESHOLD) {
              throw new KyaError("Circuit breaker tripped during retry loop", "CIRCUIT_BREAKER_OPEN", 503);
            }
          }

          // Retry on transient status codes IF circuit breaker isn't tripped
          if (attempt < this.retry.maxRetries && isTransient) {
            attempt++;
            const delay = this.retry.initialDelayMs * Math.pow(this.retry.backoffFactor, attempt - 1);
            await new Promise(r => setTimeout(r, delay));
            continue;
          }

          // We only record failure for network/server errors, not normal 400/403 rejections
          if (!isTransient && response.status >= 500) {
            this.recordFailure();
          }

          throw error;
        }

        this.cbFailureCount = 0; // Reset on success

        try {
          return schema(parsed);
        } catch (validationErr: unknown) {
          throw new KyaError(
            `Gateway response failed validation: ${(validationErr as Error).message}`,
            "SCHEMA_VALIDATION_ERROR",
          );
        }
      } catch (err: unknown) {
        if (err instanceof Error && err.name === "AbortError") {
          this.recordFailure();
          throw new KyaError(`Request timed out after ${this.timeoutMs}ms`, "TIMEOUT", 408);
        }
        if (attempt < this.retry.maxRetries) {
          attempt++;
          const delay = this.retry.initialDelayMs * Math.pow(this.retry.backoffFactor, attempt - 1);
          await new Promise(r => setTimeout(r, delay));
          continue;
        }
        this.recordFailure();
        if (err instanceof KyaError) throw err;
        throw new KyaError(`Network error: ${(err as Error).message}`);
      } finally {
        clearTimeout(timer);
      }
    }
  }

  private recordFailure() {
    this.cbFailureCount++;
    this.cbLastFailureTime = Date.now();
  }
}

export default KyaClient;
