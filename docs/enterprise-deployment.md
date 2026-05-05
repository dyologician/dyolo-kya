# dyolo-kya Enterprise Deployment & Operations Manual

This document outlines the strict production operations manual for deploying the `dyolo-kya` gateway and its backing infrastructure in a Fortune 500 or high-compliance environment. 

The `dyolo-kya` architecture is designed to be completely **stateless at the gateway layer**, offloading all persistence, atomicity, and concurrency control to battle-tested data stores. By following this guide, you eliminate the common blockers of enterprise adoption: single points of failure, unobservable bottlenecks, and cryptographic key stagnation.

---

## 1. Network Topology & TLS Termination

The `dyolo-kya-gateway` speaks plain HTTP natively to maximize throughput and minimize container bloat. **TLS must be terminated at your infrastructure edge.**

### Ingress & Service Mesh
*   **Kubernetes / Istio:** Deploy the gateway as a dedicated deployment behind an Istio VirtualService. Enforce `STRICT` mTLS between your AI orchestrator agents and the gateway.
*   **AWS ALB / Cloudflare:** Terminate external TLS at the load balancer. Use internal VPC routing to communicate with the gateway on port `8080`.
*   **Rate Limiting Resilience:** The gateway utilizes an internal `governor` token-bucket rate limiter per IP. For this to function securely behind a proxy, configure `DYOLO_TRUSTED_PROXY_MODE`.
    ```env
    # Example: Trust AWS ALB's X-Forwarded-For header
    DYOLO_TRUSTED_PROXY_MODE=x-forwarded-for
    ```

---

## 2. PostgreSQL & PgBouncer (Persistent Topology)

For strict audit compliance and durable revocation logs, PostgreSQL is the primary storage backend. Because `dyolo-kya` relies heavily on `INSERT ... ON CONFLICT DO NOTHING` for TOCTOU-safe nonce consumption, connection churn must be mitigated.

### Connection Pooling Requirement
You **must** place PgBouncer (or RDS Proxy) between the gateway and PostgreSQL. 
*   **Pool Mode:** Configure PgBouncer to `transaction` pooling mode. The gateway's queries are extremely fast, single-statement transactions.
*   **Max Connections:** `dyolo-kya` scales linearly. Cap the gateway's internal `sqlx` pool, and let PgBouncer handle the fan-in.
```ini
; pgbouncer.ini
[databases]
kyadb = host=primary-db port=5432 dbname=kyadb

[pgbouncer]
pool_mode = transaction
max_client_conn = 10000
default_pool_size = 200
3. Redis High Availability (Low-Latency Topology)
If you are running millions of ephemeral agent delegations per hour where PostgreSQL disk I/O becomes a bottleneck, configure dyolo-kya to use Redis.

Sentinel / Cluster Configuration

The dyolo-kya-redis adapter uses deadpool-redis which natively supports clustered/sentinel topologies.

Eviction Policy: Configure your Redis instance with volatile-ttl or allkeys-lru.

HA Connection String: Provide the Sentinel connection URL so the gateway automatically reconnects during master failover.

Code snippet
DYOLO_REDIS_URL="redis+sentinel://:kyapass@sentinel-1:26379,sentinel-2:26379/master-name/0"
4. Cryptographic Key Rotation Strategy
Enterprise security policies mandate regular cryptographic key rotation (e.g., every 90 days). dyolo-kya supports zero-downtime key rotation through architectural coordination.

The Keys

DYOLO_SIGNING_KEY_HEX: Ed25519 private key. Signs new DelegationCerts.

DYOLO_MAC_KEY_HEX: 32-byte Blake3 secret. Authenticates stateless VerifiedToken receipts.

Zero-Downtime Rotation Protocol

If you replace DYOLO_SIGNING_KEY_HEX outright, new certs will be issued under the new key, but previous certs remain cryptographically valid to the protocol (as long as the Root Principal's Ed25519 key didn't change).
However, changing DYOLO_MAC_KEY_HEX immediately invalidates cached VerifiedToken receipts across your network.

The Migration Steps:

Generate a new MAC key and Signing key via dyolo-kya keygen.

Deploy Gateway V2 alongside Gateway V1. V2 has the new keys. V1 retains the old keys.

Route all POST /v1/cert/issue traffic to V2.

Route POST /v1/authorize traffic to V2.

Edge Case Mitigation: If an internal service fails to verify a token against V2 (because it holds a V1 token), the client SDK should catch the 401 MAC_VERIFICATION_FAILED and gracefully fallback to executing a full POST /v1/authorize to get a fresh V2 token.

Once the maximum token TTL expires, decommission Gateway V1.

5. Observability: OpenTelemetry & SIEM
Enterprise AI requires total visibility into agent actions. dyolo-kya acts as the definitive audit chokepoint.

Metrics & Traces (OTEL)

If compiled with features=["telemetry"], the gateway exports natively to your OTEL collector:

Prometheus: Scrape /metrics to monitor kya_authorization_latency_seconds, kya_nonce_conflicts_total, and kya_revocation_checks_total.

Traces: Distributed tracing allows you to correlate an LLM generation span directly to the exact dyolo-kya cryptographic verification span.

SIEM Integration (Datadog / Splunk / ELK)

Every verification attempt emits a structured NDJSON AuditEvent. You do not need to parse text logs.

Deploy a log shipper (Vector, Promtail, Fluent Bit) as a DaemonSet or sidecar to the gateway.

Filter stderr for lines starting with { (valid JSON).

Pipe directly to your SIEM.

Example SIEM Query (Splunk-style):

Plaintext
index=ai_security sourcetype=dyolo_kya outcome="DENIED" error_code="SCOPE_ESCALATION"
| stats count by executor_pk_hex, intent
This instantly identifies an infected or hallucinating AI agent attempting to execute actions outside its cryptographically assigned boundaries.