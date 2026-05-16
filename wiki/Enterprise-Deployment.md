# Enterprise Deployment Guide

This guide covers production deployment of A1 for enterprises with compliance, availability, and security requirements. It complements the detailed operational manual at [`docs/enterprise-deployment.md`](../docs/enterprise-deployment.md).

---

## Architecture overview

```
┌──────────────────────────────────────────────────────┐
│  AI Agent Swarm (any language, any framework)        │
│  Python · TypeScript · Go · Rust                     │
└─────────────────────┬────────────────────────────────┘
                      │  POST /authorize
                      │  (signed chain + intent)
┌─────────────────────▼────────────────────────────────┐
│  a1-gateway (one or more replicas)            │
│  - Axum HTTP server                                  │
│  - Stateless: all state in Redis or Postgres         │
│  - TLS terminated at ingress (ALB, Nginx, Istio)     │
└──────────┬───────────────────────┬───────────────────┘
           │                       │
┌──────────▼──────────┐  ┌─────────▼─────────────────┐
│  Redis              │  │  PostgreSQL + PgBouncer    │
│  - Nonce store      │  │  - Nonce store             │
│  - Revocation list  │  │  - Revocation list         │
│  - Rate limiting    │  │  - Audit log (optional)    │
└─────────────────────┘  └───────────────────────────┘
           │
┌──────────▼──────────┐
│  KMS (optional)     │
│  - AWS KMS          │
│  - GCP KMS          │
│  - HashiCorp Vault  │
│  - Azure Key Vault  │
└─────────────────────┘
```

The gateway is **fully stateless at the application layer**. All state (nonces, revocations, rate limits) lives in the storage backend. This means you can scale horizontally to any number of replicas with no coordination.

---

## Minimal production deployment

### Docker Compose (single node)

```bash
git clone https://github.com/dyologician/a1
cd a1
docker compose up -d
```

The default `docker/docker-compose.yml` starts:
- `a1-gateway` on port `8080`
- `postgres` on port `5432`
- `redis` on port `6379`

### Environment variables (required in production)

```env
# Stable signing key — VerifiedTokens survive gateway restarts
A1_SIGNING_KEY_HEX=<32-byte hex, generate with: a1 keygen>

# Stable HMAC key — VerifiedToken MACs survive restarts
A1_MAC_KEY_HEX=<32-byte hex, generate with: a1 keygen>

# Storage backends (pick one or both)
A1_PG_URL=postgres://a1:password@pgbouncer:5432/a1
A1_REDIS_URL=redis://redis:6379

# Rate limiting
A1_RATE_LIMIT_RPS=100

# Trusted proxy for X-Forwarded-For (AWS ALB, Cloudflare, etc.)
A1_TRUSTED_PROXY_MODE=x-forwarded-for
```

---

## Kubernetes deployment

```yaml
# deployment.yaml (simplified)
apiVersion: apps/v1
kind: Deployment
metadata:
  name: a1-gateway
spec:
  replicas: 3
  selector:
    matchLabels:
      app: a1-gateway
  template:
    spec:
      containers:
      - name: gateway
        image: ghcr.io/dyologician/a1-gateway:2.8.0
        ports:
        - containerPort: 8080
        env:
        - name: A1_SIGNING_KEY_HEX
          valueFrom:
            secretKeyRef:
              name: a1-secrets
              key: signing-key-hex
        - name: A1_MAC_KEY_HEX
          valueFrom:
            secretKeyRef:
              name: a1-secrets
              key: mac-key-hex
        - name: A1_PG_URL
          valueFrom:
            secretKeyRef:
              name: a1-secrets
              key: pg-url
        readinessProbe:
          httpGet:
            path: /healthz
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 10
        livenessProbe:
          httpGet:
            path: /healthz
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 30
```

The `/healthz` endpoint checks storage backend liveness. A degraded storage backend causes the probe to fail, which triggers automatic removal from the load balancer pool.

---

## Key rotation

### Passport rotation

Passport TTLs are set at issuance time. When a passport expires:
1. Generate a new key if needed: `a1 keygen --out new-key.hex`
2. Issue a new passport: `a1 passport issue --namespace <name> --allow "..." --key new-key.hex`
3. Update your agents to use the new passport.
4. Old authorized receipts from the expired passport remain valid for audit replay.

### Gateway signing key rotation

The gateway signing key produces `VerifiedToken` MACs. Rotate by:
1. Generate a new key: `a1 keygen`
2. Update `A1_SIGNING_KEY_HEX` and `A1_MAC_KEY_HEX` in your secrets manager.
3. Rolling-restart the gateway replicas (tokens issued with the old key will fail, but this only affects very short-lived tokens used internally by the gateway).

---

## KMS integration

See [KMS Integration](KMS-Integration) for full provider-specific setup. Summary:

```python
from a1.vault import AwsKmsSigner

signer = AwsKmsSigner(
    key_id="alias/a1-passport-root",
    region="us-east-1",
)
# Pass signer to gateway's /issue endpoint or Rust DyoloPassport::issue
```

At authorization time: **zero KMS calls**. Verification is fully local.

---

## SIEM integration

Every authorization event can be forwarded to your existing SIEM. See [SIEM Integration](SIEM-Integration).

```python
from a1.siem import CompositeExporter, DatadogLogExporter, SplunkHecExporter

exporter = CompositeExporter([
    DatadogLogExporter(api_key=os.environ["DD_API_KEY"], service="ai-agents"),
    SplunkHecExporter(url="https://splunk.corp.com:8088", token="..."),
])
```

---

## High availability checklist

- [ ] Three or more gateway replicas behind a load balancer
- [ ] `A1_SIGNING_KEY_HEX` and `A1_MAC_KEY_HEX` set to stable values from a secrets manager
- [ ] PostgreSQL with PgBouncer in `transaction` pool mode, or Redis Cluster
- [ ] `/healthz` endpoint wired to load balancer health checks
- [ ] TLS terminated at ingress (ALB, Nginx, Istio mTLS)
- [ ] `A1_RATE_LIMIT_RPS` tuned to your expected peak throughput
- [ ] Revocation store on persistent backend (Redis or Postgres) so revocations survive restarts
- [ ] Audit log exporter configured (Datadog, Splunk, or NDJSON file → Filebeat)
- [ ] Passport files stored in secrets manager (AWS Secrets Manager, HashiCorp Vault, Azure Key Vault)
- [ ] Passport TTLs set to ≤ 30 days; sub-cert TTLs set to ≤ task duration

---

## Compliance

A1 ships pre-built compliance mapping documents:

- [`docs/compliance/soc2-mapping.md`](../docs/compliance/soc2-mapping.md) — SOC 2 Type II
- [`docs/compliance/iso27001-mapping.md`](../docs/compliance/iso27001-mapping.md) — ISO/IEC 27001:2022
- [`docs/compliance/sample-audit-report.md`](../docs/compliance/sample-audit-report.md) — Audit report template

See the [Compliance](Compliance) wiki page for how to use these during an audit.

---

## Air-gapped deployments

A1 is designed for air-gapped environments. Every verification is local:

1. No network call at authorization time.
2. No cloud dependency for `guard_local`.
3. No telemetry sent anywhere.
4. Docker images can be mirrored to an internal registry.
5. Rust crates can be vendored: `cargo vendor`.

For air-gapped deployments, use `MemoryNonceStore` and `MemoryRevocationStore`, or a local Postgres instance. Nonce replay protection is limited to the process lifetime with in-memory stores — use persistent storage if the gateway restarts are possible.