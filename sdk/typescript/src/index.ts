/**
 * a1 — TypeScript/Node.js client for the a1 AI agent
 * authorization protocol.
 *
 * Wraps the a1-gateway REST API. No Rust toolchain required.
 *
 * @example
 * ```ts
 * import { A1Client } from "a1";
 *
 * const client = new A1Client("http://localhost:8080");
 *
 * const result = await client.authorize({
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
    issuer:                      str(o["issuer"], "issuer"),
    gateway_signing_pk_hex:      str(o["gateway_signing_pk_hex"], "gateway_signing_pk_hex"),
    gateway_did:                 optStr(o["gateway_did"], "gateway_did") ?? "",
    authorization_endpoint:      str(o["authorization_endpoint"], "authorization_endpoint"),
    batch_authorize_endpoint:    str(o["batch_authorize_endpoint"], "batch_authorize_endpoint"),
    passport_authorize_endpoint: optStr(o["passport_authorize_endpoint"], "passport_authorize_endpoint") ?? "",
    cert_issuance_endpoint:      str(o["cert_issuance_endpoint"], "cert_issuance_endpoint"),
    cert_revoke_endpoint:        str(o["cert_revoke_endpoint"], "cert_revoke_endpoint"),
    cert_revoke_batch_endpoint:  str(o["cert_revoke_batch_endpoint"], "cert_revoke_batch_endpoint"),
    token_verify_endpoint:       str(o["token_verify_endpoint"], "token_verify_endpoint"),
    crl_endpoint:                optStr(o["crl_endpoint"], "crl_endpoint") ?? "",
    did_resolve_endpoint:        optStr(o["did_resolve_endpoint"], "did_resolve_endpoint") ?? "",
    did_gateway_endpoint:        optStr(o["did_gateway_endpoint"], "did_gateway_endpoint") ?? "",
    vc_issue_endpoint:           optStr(o["vc_issue_endpoint"], "vc_issue_endpoint") ?? "",
    vc_verify_endpoint:          optStr(o["vc_verify_endpoint"], "vc_verify_endpoint") ?? "",
    anchor_endpoint:             optStr(o["anchor_endpoint"], "anchor_endpoint") ?? "",
    negotiate_endpoint:          optStr(o["negotiate_endpoint"], "negotiate_endpoint") ?? "",
    jwt_exchange_endpoint:       optStr(o["jwt_exchange_endpoint"], "jwt_exchange_endpoint") ?? "",
    webhook_status_endpoint:     optStr(o["webhook_status_endpoint"], "webhook_status_endpoint") ?? "",
    tenant_info_endpoint:        optStr(o["tenant_info_endpoint"], "tenant_info_endpoint") ?? "",
    a1_version:                  str(o["a1_version"], "a1_version"),
    protocol_enforcer:           optStr(o["protocol_enforcer"], "protocol_enforcer") ?? "dyolo_v2.8.0",
    supported_algorithms:        arr(o["supported_algorithms"], "supported_algorithms", (v, c) => str(v, c)),
    supported_features:          Array.isArray(o["supported_features"]) ? arr(o["supported_features"], "supported_features", (v, c) => str(v, c)) : [],
    supported_networks:          Array.isArray(o["supported_networks"])  ? arr(o["supported_networks"],  "supported_networks",  (v, c) => str(v, c)) : [],
    jwt_exchange_enabled:        optBool(o["jwt_exchange_enabled"],  "jwt_exchange_enabled")  ?? false,
    webhook_enabled:             optBool(o["webhook_enabled"],       "webhook_enabled")       ?? false,
    multi_tenant_enabled:        optBool(o["multi_tenant_enabled"],  "multi_tenant_enabled")  ?? false,
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

export class A1Error extends Error {
  readonly code: string | undefined;
  readonly status: number | undefined;

  constructor(message: string, code?: string, status?: number) {
    super(message);
    this.name = "A1Error";
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
  gateway_did: string;
  authorization_endpoint: string;
  batch_authorize_endpoint: string;
  passport_authorize_endpoint: string;
  cert_issuance_endpoint: string;
  cert_revoke_endpoint: string;
  cert_revoke_batch_endpoint: string;
  token_verify_endpoint: string;
  crl_endpoint: string;
  did_resolve_endpoint: string;
  did_gateway_endpoint: string;
  vc_issue_endpoint: string;
  vc_verify_endpoint: string;
  anchor_endpoint: string;
  negotiate_endpoint: string;
  jwt_exchange_endpoint: string;
  webhook_status_endpoint: string;
  tenant_info_endpoint: string;
  a1_version: string;
  protocol_enforcer: string;
  supported_algorithms: string[];
  supported_features: string[];
  supported_networks: string[];
  jwt_exchange_enabled: boolean;
  webhook_enabled: boolean;
  multi_tenant_enabled: boolean;
}

// ── DID + Verifiable Credentials ─────────────────────────────────────────────

export interface DidVerificationMethod {
  id: string;
  type: string;
  controller: string;
  publicKeyHex: string;
}

export interface DidDocument {
  "@context": string[];
  id: string;
  verificationMethod: DidVerificationMethod[];
  authentication: string[];
  assertionMethod: string[];
  capabilityDelegation: string[];
  a1PassportNamespace?: string;
  a1CapabilityMaskHex?: string;
  a1Version: string;
}

export interface VcProof {
  type: string;
  created: string;
  verificationMethod: string;
  proofPurpose: string;
  proofValue: string;
}

export interface VcCredentialSubject {
  id: string;
  a1PassportNamespace: string;
  a1Capabilities: string[];
  a1ChainFingerprint: string;
  a1Version: string;
}

export interface VerifiableCredential {
  "@context": string[];
  type: string[];
  id: string;
  issuer: string;
  issuanceDate: string;
  expirationDate?: string;
  credentialSubject: VcCredentialSubject;
  proof: VcProof;
}

export interface IssueVcOptions {
  subjectPkHex: string;
  passportNamespace: string;
  capabilities: string[];
  ttlSeconds?: number;
  chainFingerprintHex?: string;
}

export interface IssueVcResult {
  credential: VerifiableCredential;
  subject_did: string;
  issuer_did: string;
}

export interface VerifyVcResult {
  valid: boolean;
  issuerDid: string;
  subjectDid: string;
  passportNamespace: string;
  capabilities: string[];
  error?: string;
}

// ── On-chain anchoring ────────────────────────────────────────────────────────

export type AnchorNetwork =
  | "ethereum"
  | "ethereum-sepolia"
  | "polygon"
  | "base"
  | "arbitrum"
  | "solana"
  | { custom: { chain_id: number; name: string } };

export interface AnchorReceiptOptions {
  /** The ZkChainCommitment object (from `authorize` response). */
  commitment: Record<string, unknown>;
  /** `did:a1:` identifier of the passport holder. */
  passportDid: string;
  /** Target network. Default: "ethereum". */
  network?: AnchorNetwork;
}

export interface SubmissionGuide {
  contract_function: string;
  contract_interface: string;
  ethersjs_snippet?: string;
  viemjs_snippet?: string;
}

export interface AnchorReceiptResult {
  anchored_receipt: Record<string, unknown>;
  anchor_hash_hex: string;
  network: string;
  evm_calldata?: string;
  solana_instruction_data?: string;
  submission_guide: SubmissionGuide;
}

// ── Agent negotiation ─────────────────────────────────────────────────────────

export interface NegotiateOptions {
  /** Ed25519 signing key of the requesting agent (hex-encoded 32-byte seed). */
  requesterSigningKeyHex: string;
  requestedCapabilities: string[];
  intentName: string;
  ttlSeconds?: number;
}

export interface NegotiationResult {
  cert: Record<string, unknown>;
  offer: Record<string, unknown>;
  fingerprint_hex: string;
  offerer_did: string;
  requester_did: string;
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

export interface A1ClientOptions {
  /** Request timeout in milliseconds. Default: 10000. */
  timeoutMs?: number;
  /** Static headers added to every request (e.g. Authorization). */
  headers?: Record<string, string>;
  /** Retry configuration for transient errors. */
  retry?: RetryOptions;
  /** Enable circuit breaker to prevent cascade failures. */
  enableCircuitBreaker?: boolean;
}

// ── A1Client ─────────────────────────────────────────────────────────────────

export class A1Client {
  private readonly base: string;
  private readonly timeoutMs: number;
  private readonly extraHeaders: Record<string, string>;
  private readonly retry: Required<RetryOptions>;
  
  // Minimal Circuit Breaker state
  private cbFailureCount = 0;
  private cbLastFailureTime = 0;
  private readonly CB_THRESHOLD = 5;
  private readonly CB_RESET_TIMEOUT_MS = 30_000;

  constructor(baseUrl: string, opts: A1ClientOptions = {}) {
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
    return this.get("/.well-known/a1-configuration", WellKnownConfigSchema);
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

  // ── DID + Verifiable Credentials ──────────────────────────────────────────

  /** Resolve a W3C DID Document from an Ed25519 public key (hex). */
  async resolveDid(pkHex: string): Promise<DidDocument> {
    return this.get(`/v1/did/${pkHex}`, (v: unknown) => v as DidDocument);
  }

  /** Return the W3C DID Document for the gateway's own signing identity. */
  async gatewayDid(): Promise<DidDocument> {
    return this.get("/v1/did/gateway", (v: unknown) => v as DidDocument);
  }

  /**
   * Issue a W3C Verifiable Credential asserting an agent's authorized capabilities.
   * Requires `Authorization: Bearer <A1_ADMIN_SECRET>` header.
   */
  async issueVc(opts: IssueVcOptions): Promise<IssueVcResult> {
    const body: Record<string, unknown> = {
      subject_pk_hex: opts.subjectPkHex,
      passport_namespace: opts.passportNamespace,
      capabilities: opts.capabilities,
      ttl_seconds: opts.ttlSeconds ?? 86400,
    };
    if (opts.chainFingerprintHex) body["chain_fingerprint_hex"] = opts.chainFingerprintHex;
    return this.post("/v1/vc/issue", (v: unknown) => v as IssueVcResult, body);
  }

  /**
   * Verify a W3C Verifiable Credential. Works for any `did:a1:` issuer.
   */
  async verifyVc(credential: VerifiableCredential): Promise<VerifyVcResult> {
    return this.post(
      "/v1/vc/verify",
      (v: unknown) => v as VerifyVcResult,
      { credential },
    );
  }

  // ── On-chain anchoring ────────────────────────────────────────────────────

  /**
   * Prepare on-chain anchor calldata for a ZK chain commitment.
   *
   * Returns ABI-encoded EVM calldata or Solana instruction data.
   * Submit via ethers.js, viem, web3.py, or `a1 anchor <receipt.json>`.
   *
   * ```typescript
   * const anchor = await client.anchorReceipt({
   *   commitment: authorizeResult.commitment,
   *   passportDid: "did:a1:...",
   *   network: "ethereum",
   * });
   *
   * // Submit with viem:
   * await walletClient.sendTransaction({
   *   to: A1_ANCHOR_CONTRACT,
   *   data: `0x${anchor.evm_calldata}`,
   * });
   * ```
   */
  async anchorReceipt(opts: AnchorReceiptOptions): Promise<AnchorReceiptResult> {
    return this.post(
      "/v1/anchor",
      (v: unknown) => v as AnchorReceiptResult,
      {
        commitment: opts.commitment,
        passport_did: opts.passportDid,
        network: opts.network ?? "ethereum",
      },
    );
  }

  // ── Agent negotiation ─────────────────────────────────────────────────────

  /**
   * Request a delegation certificate from this gateway (AIP negotiation protocol).
   *
   * Builds and signs a `CapabilityRequest`, sends it to `/v1/negotiate`,
   * and returns the `NegotiationResult` with a ready-to-use cert.
   *
   * ```typescript
   * const result = await client.negotiateDelegation({
   *   requesterSigningKeyHex: myPrivateKeyHex,
   *   requestedCapabilities: ["trade.equity"],
   *   intentName: "trade.equity",
   *   ttlSeconds: 3600,
   * });
   * // result.cert is ready to push onto a DyoloChain
   * ```
   */
  async negotiateDelegation(opts: NegotiateOptions): Promise<NegotiationResult> {
    const { requesterSigningKeyHex, requestedCapabilities, intentName, ttlSeconds = 3600 } = opts;

    const skBytes = hexToBytes(requesterSigningKeyHex);
    // @noble/ed25519 is an optional peer dependency used only by negotiateDelegation.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const noble = await import("@noble/ed25519" as any).catch(() => {
      throw new Error(
        "negotiateDelegation requires @noble/ed25519. Install it: npm install @noble/ed25519"
      );
    }) as { sign: (m: Uint8Array, sk: Uint8Array) => Promise<Uint8Array>; getPublicKey: (sk: Uint8Array) => Promise<Uint8Array> };
    const { sign, getPublicKey } = noble;
    const pkBytes = await getPublicKey(skBytes);
    const pkHex = bytesToHex(pkBytes);
    const requesterDid = `did:a1:${pkHex}`;

    const nonce = crypto.getRandomValues(new Uint8Array(16));
    const nonceHex = bytesToHex(nonce);
    const timestamp = Math.floor(Date.now() / 1000);

    const msgBytes = buildRequestSignable(
      requesterDid, nonce, timestamp, ttlSeconds, intentName, requestedCapabilities,
    );
    const sigBytes = await sign(msgBytes, skBytes);
    const sigHex = bytesToHex(sigBytes);

    return this.post(
      "/v1/negotiate",
      (v: unknown) => v as NegotiationResult,
      {
        requester_did: requesterDid,
        requester_pk_hex: pkHex,
        requested_capabilities: requestedCapabilities,
        intent_name: intentName,
        ttl_secs: ttlSeconds,
        nonce: nonceHex,
        timestamp_unix: timestamp,
        signature: sigHex,
      },
    );
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
          throw new A1Error("Circuit breaker is open", "CIRCUIT_BREAKER_OPEN", 503);
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
          throw new A1Error(`Gateway returned non-JSON response (status ${response.status})`);
        }

        if (!response.ok) {
          const obj = parsed as Record<string, unknown>;
          const error = new A1Error(
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
              throw new A1Error("Circuit breaker tripped during retry loop", "CIRCUIT_BREAKER_OPEN", 503);
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
          throw new A1Error(
            `Gateway response failed validation: ${(validationErr as Error).message}`,
            "SCHEMA_VALIDATION_ERROR",
          );
        }
      } catch (err: unknown) {
        if (err instanceof Error && err.name === "AbortError") {
          this.recordFailure();
          throw new A1Error(`Request timed out after ${this.timeoutMs}ms`, "TIMEOUT", 408);
        }
        if (attempt < this.retry.maxRetries) {
          attempt++;
          const delay = this.retry.initialDelayMs * Math.pow(this.retry.backoffFactor, attempt - 1);
          await new Promise(r => setTimeout(r, delay));
          continue;
        }
        this.recordFailure();
        if (err instanceof A1Error) throw err;
        throw new A1Error(`Network error: ${(err as Error).message}`);
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

export default A1Client;

// ── Negotiate helpers (used by negotiateDelegation) ───────────────────────────

function hexToBytes(hex: string): Uint8Array {
  const clean = hex.startsWith("0x") ? hex.slice(2) : hex;
  const arr = new Uint8Array(clean.length / 2);
  for (let i = 0; i < arr.length; i++) {
    arr[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  }
  return arr;
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes).map(b => b.toString(16).padStart(2, "0")).join("");
}

function writeU64LE(n: number, buf: Uint8Array, offset: number): void {
  const lo = n >>> 0;
  const hi = Math.floor(n / 0x100000000) >>> 0;
  buf[offset]     = lo & 0xFF;
  buf[offset + 1] = (lo >>> 8) & 0xFF;
  buf[offset + 2] = (lo >>> 16) & 0xFF;
  buf[offset + 3] = (lo >>> 24) & 0xFF;
  buf[offset + 4] = hi & 0xFF;
  buf[offset + 5] = (hi >>> 8) & 0xFF;
  buf[offset + 6] = (hi >>> 16) & 0xFF;
  buf[offset + 7] = (hi >>> 24) & 0xFF;
}

function buildRequestSignable(
  requesterDid: string,
  nonce: Uint8Array,
  timestamp: number,
  ttl: number,
  intentName: string,
  caps: string[],
): Uint8Array {
  const enc = new TextEncoder();
  const didBytes = enc.encode(requesterDid);
  const intentBytes = enc.encode(intentName);
  const capBuffers = caps.map(c => enc.encode(c));

  const totalLen =
    8 + didBytes.length +
    16 +
    8 + 8 +
    8 + intentBytes.length +
    8 + capBuffers.reduce((a, b) => a + 8 + b.length, 0);

  const buf = new Uint8Array(totalLen);
  let pos = 0;

  writeU64LE(didBytes.length, buf, pos); pos += 8;
  buf.set(didBytes, pos); pos += didBytes.length;
  buf.set(nonce, pos); pos += 16;
  writeU64LE(timestamp, buf, pos); pos += 8;
  writeU64LE(ttl, buf, pos); pos += 8;
  writeU64LE(intentBytes.length, buf, pos); pos += 8;
  buf.set(intentBytes, pos); pos += intentBytes.length;
  writeU64LE(caps.length, buf, pos); pos += 8;
  for (const cb of capBuffers) {
    writeU64LE(cb.length, buf, pos); pos += 8;
    buf.set(cb, pos); pos += cb.length;
  }
  return buf;
}

// ── Passport convenience re-exports ───────────────────────────────────────────
//
// These types and functions are also available under the dedicated subpath
// import `"a1/passport"`. They are re-exported here so integrations
// that import a single entry point (e.g. bundlers without subpath support)
// can access them without a second import statement.
export type {
    PassportReceipt,
    AuthorizePassportRequest,
    PassportGuardOptions,
} from "./passport.js";
export { PassportClient, PassportError, withA1Passport, PassportGuard } from "./passport.js";

export type {
    DidDocument,
    DidVerificationMethod,
    VerifiableCredential,
    VcProof,
    VcCredentialSubject,
    IssueVcOptions,
    IssueVcResult,
    VerifyVcResult,
    AnchorNetwork,
    AnchorReceiptOptions,
    AnchorReceiptResult,
    SubmissionGuide,
    NegotiateOptions,
    NegotiationResult,
};

// ── Middleware and enterprise helpers re-exports ──────────────────────────────
//
// Available under `"a1/middleware"` subpath and directly here for convenience.
export type {
    A1RequestReceipt,
    GuardOptions,
    JwtExchangeOptions,
    JwtExchangeResult,
    WebhookEvent,
} from "./middleware.js";
export { A1Middleware, exchangeJwt, verifyWebhookSignature } from "./middleware.js";
