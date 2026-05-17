# Gateway API Reference

The A1 gateway exposes a REST/JSON API on `http://localhost:8080` (default). All endpoints accept and return `application/json`. Admin endpoints require an `Authorization: Bearer <A1_ADMIN_SECRET>` header when `A1_ADMIN_SECRET` is set.

---

## Start the gateway

```bash
git clone https://github.com/dyologician/A1
cd A1
./setup.sh
```

---

## Authorization

### `POST /v1/authorize`

Authorize a single agent intent against a delegation chain.

**Request**

```json
{
  "chain": { /* SignedChain wire format */ },
  "intent_name": "trade.equity",
  "intent_params": { "symbol": "AAPL" },
  "executor_pk_hex": "4a1b2c..."
}
```

**Response 200**

```json
{
  "authorized": true,
  "chain_depth": 2,
  "chain_fingerprint": "a3b2c1...",
  "namespace": "trading-bot",
  "capability_mask": "00ff..."
}
```

**Response 403**

```json
{
  "error_code": "PASSPORT_NARROWING_VIOLATION",
  "message": "agent does not hold capability trade.equity"
}
```

---

### `POST /v1/authorize/batch`

Authorize up to 256 intents in a single request. Returns one result per intent.

**Request**

```json
{
  "chain": { /* SignedChain */ },
  "executor_pk_hex": "4a1b2c...",
  "intents": [
    { "intent_name": "trade.equity", "intent_params": { "symbol": "AAPL" } },
    { "intent_name": "portfolio.read" }
  ]
}
```

**Response 200**

```json
{
  "results": [
    { "authorized": true, "intent_name": "trade.equity", "chain_depth": 2 },
    { "authorized": true, "intent_name": "portfolio.read", "chain_depth": 2 }
  ]
}
```

---

### `POST /v1/passport/authorize`

Passport-level authorization. Metrics are tracked separately from chain authorization.

Same request/response shape as `/v1/authorize`. Use this endpoint when you want to separate passport-specific metrics from raw chain metrics in your observability stack.

---

## Certificate management (admin)

### `POST /v1/cert/issue`

Issue a delegation certificate.

**Request**

```json
{
  "delegate_pk_hex": "4a1b2c...",
  "capabilities": ["trade.equity", "portfolio.read"],
  "ttl_seconds": 3600,
  "namespace": "trading-bot",
  "idempotency_key": "optional-dedup-key"
}
```

**Response 200**

```json
{
  "cert": { /* DelegationCert wire format */ },
  "fingerprint": "a3b2c1..."
}
```

---

### `POST /v1/cert/issue-batch`

Issue multiple delegation certs atomically.

```json
{
  "certs": [
    { "delegate_pk_hex": "...", "capabilities": ["trade.equity"], "ttl_seconds": 3600 },
    { "delegate_pk_hex": "...", "capabilities": ["portfolio.read"], "ttl_seconds": 1800 }
  ],
  "namespace": "trading-bot"
}
```

---

### `POST /v1/cert/revoke`

Revoke a certificate by fingerprint. Immediate effect.

```json
{ "fingerprint": "a3b2c1..." }
```

---

### `POST /v1/cert/revoke-batch`

Bulk revocation.

```json
{ "fingerprints": ["a3b2c1...", "d4e5f6..."] }
```

---

### `GET /v1/cert/:fingerprint`

Check revocation status.

**Response 200**

```json
{
  "fingerprint": "a3b2c1...",
  "revoked": false,
  "revoked_at": null
}
```

---

## Passport management (admin)

### `GET /v1/passports/list`

List all passport files known to the gateway.

### `POST /v1/passports/issue`

Issue a new root passport.

```json
{
  "namespace": "trading-bot",
  "capabilities": ["trade.equity", "portfolio.read"],
  "ttl_seconds": 2592000
}
```

### `POST /v1/passports/renew`

Re-issue a passport with a new TTL.

```json
{ "namespace": "trading-bot", "ttl_seconds": 2592000 }
```

### `GET /v1/passports/read`

Read a passport by namespace.

```
GET /v1/passports/read?namespace=trading-bot
```

### `POST /v1/passports/revoke-by-namespace`

Revoke all certs under a namespace.

```json
{ "namespace": "trading-bot" }
```

---

## Token verification

### `POST /v1/token/verify`

Verify a `VerifiedToken` HMAC receipt produced by the gateway.

```json
{ "token": "a1.v1.base64encodedhmac..." }
```

---

## DID and Verifiable Credentials

### `GET /v1/did/gateway`

The gateway's own W3C DID Document.

### `GET /v1/did/:pk_hex`

Resolve any Ed25519 public key to a W3C DID Document.

```
GET /v1/did/4a1b2c3d...
```

### `POST /v1/vc/issue` (admin)

Issue a W3C Verifiable Credential.

```json
{
  "subject_pk_hex": "4a1b2c...",
  "claims": { "role": "trader", "clearance": "level-2" }
}
```

### `POST /v1/vc/verify`

Verify a W3C VC and extract claims.

```json
{ "vc": { /* VerifiableCredential JSON-LD */ } }
```

---

## Swarm

### `POST /v1/swarm/create` (admin)

Create a new agent swarm.

```json
{
  "name": "trading-swarm",
  "capabilities": ["trade.equity"],
  "ttl_days": 30
}
```

### `POST /v1/swarm/member/add` (admin)

Add an agent to a swarm.

```json
{
  "swarm_id": "swarm-abc123",
  "agent_pk_hex": "4a1b2c...",
  "role": "worker",
  "capabilities": ["trade.equity"],
  "ttl_seconds": 3600
}
```

### `POST /v1/swarm/member/remove` (admin)

Remove an agent from a swarm.

### `GET /v1/swarm/:swarm_id/members`

List active swarm members.

---

## Governance

### `GET /v1/governance/policy`

Return the active delegation policy.

### `POST /v1/governance/approval/verify`

Verify a governance approval record against its on-chain hash.

### `POST /v1/governance/audit-report` (admin)

Generate a structured audit report.

---

## Other endpoints

| Method | Path | Description |
|---|---|---|
| `POST` | `/v1/negotiate` | Agent-to-agent capability negotiation handshake |
| `POST` | `/v1/jwt/exchange` | Exchange a JWKS-verified JWT for a scoped DelegationCert |
| `POST` | `/v1/anchor` | Anchor a ZkChainCommitment to a transparency log |
| `POST` | `/v1/debug/explain-error` | Translate an A1 error code to plain English |
| `GET`  | `/v1/tenant/info` | Active tenant context |
| `GET`  | `/v1/tenant/config` | Per-tenant capability allowlist |
| `GET`  | `/v1/webhook/status` | Webhook delivery status |
| `POST` | `/v1/webhook/test` | Send a test webhook event |
| `GET`  | `/healthz` | Health check — returns `{"status":"ok"}` |
| `GET`  | `/.well-known/a1-configuration` | Service discovery document |
| `GET`  | `/.well-known/schema.json` | Wire format JSON Schema |
| `GET`  | `/studio` | A1 Studio web dashboard |

---

## Error codes

| Code | HTTP | Meaning |
|---|---|---|
| `PASSPORT_NARROWING_VIOLATION` | 403 | Agent requests a capability not in its cert |
| `CERT_EXPIRED` | 403 | Delegation cert has passed its `expires_at` |
| `CERT_REVOKED` | 403 | Cert fingerprint found in RevocationStore |
| `NONCE_REPLAY` | 403 | Intent nonce already consumed |
| `NAMESPACE_MISMATCH` | 403 | Chain namespace does not match request |
| `SIGNATURE_INVALID` | 403 | Ed25519 signature verification failed |
| `CHAIN_DEPTH_EXCEEDED` | 403 | Chain exceeds configured max depth |
| `POLICY_VIOLATION` | 403 | Policy rule (TTL, depth, namespace) violated |
| `UNAUTHORIZED` | 401 | Missing or invalid `A1_ADMIN_SECRET` |
| `INTERNAL_ERROR` | 500 | Gateway internal error |

---

## Production configuration

| Variable | Default | Description |
|---|---|---|
| `A1_SIGNING_KEY_HEX` | auto | 32-byte hex Ed25519 seed |
| `A1_MAC_KEY_HEX` | auto | 32-byte hex HMAC key |
| `A1_ADMIN_SECRET` | none | Bearer token for admin endpoints |
| `A1_REDIS_URL` | none | Redis URL for persistent nonce/revocation store |
| `A1_PG_URL` | none | Postgres URL for persistent store |
| `A1_RATE_LIMIT_RPS` | 500 | Per-IP requests per second |
| `A1_CORS_ALLOWED_ORIGIN` | none | CORS origin header |
| `GATEWAY_ADDR` | 0.0.0.0:8080 | Bind address |
| `A1_PUBLIC_BASE_URL` | http://localhost:8080 | Used in service discovery doc |
| `RUST_LOG` | a1_gateway=info | Log filter |

---

*Source: `a1-gateway/src/routes/` · [Back to wiki home](Home)*
