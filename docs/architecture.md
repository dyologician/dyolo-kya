# A1 Enterprise Architecture

This document maps the cryptographic lifecycle of an AI agent intent, from human authorization to terminal execution. It is designed for enterprise security and architecture review.

## The Recursive Delegation Lifecycle

The core problem A1 solves is the "Recursive Delegation Gap." When a human asks Agent A to perform a task, and Agent A delegates to Agent B, the authorization context is usually lost. A1 fixes this by enforcing a cryptographically verifiable chain-of-custody.
```mermaid
sequenceDiagram
    autonumber
    actor Human as Human Principal (Root)
    participant AuthZ as Orchestrator Agent
    participant Tool as Tool Executor Agent
    participant Gateway as a1-gateway
    participant DB as Postgres/Redis

    Note over Human,AuthZ: 1. Issuance (Root -> Delegated Scope)
    Human->>Gateway: Issue DelegationCert (Scope: "trade.*", Max Depth: 8)
    Gateway-->>Human: Return Cert C1
    Human->>AuthZ: Transfer C1 (State/Session)

    Note over AuthZ,Tool: 2. Sub-Delegation (Scope Narrowing)
    AuthZ->>Gateway: Issue Sub-Cert (Scope: "trade.equity/NYSE", Max Depth: 0)
    Gateway-->>AuthZ: Return Cert C2
    AuthZ->>Tool: Transfer [C1, C2] (SignedChain)

    Note over Tool,Gateway: 3. Execution & Authorization
    Tool->>Gateway: POST /v1/authorize (Chain: [C1, C2], Intent: "trade.equity/NYSE")
    
    rect rgb(200, 220, 240)
        Note over Gateway,DB: 4. Cryptographic Validation
        Gateway->>Gateway: Verify Ed25519 Batch Signatures
        Gateway->>Gateway: Verify Merkle SubScopeProofs
        Gateway->>Gateway: Enforce Temporal Validity (TTL)
        Gateway->>DB: Check RevocationStore
        Gateway->>DB: Atomic try_consume(Nonce) -> Prevent Replay
    end

    Gateway-->>Tool: 200 OK (AuthorizedAction Receipt)
    Tool->>Tool: Execute External API Call

    Note over DB,Gateway: 5. Audit & Revocation
    Gateway->>DB: Emit Structured AuditEvent (NDJSON)
    Human->>Gateway: POST /v1/cert/revoke (Fingerprint of C1 or C2)
    Gateway->>DB: Update RevocationStore (Immediate propagation)
Security Invariants
Impossibility of Scope Escalation: An agent cannot execute or delegate an intent outside the Merkle root of the scope defined by its parent.

Impossibility of Replay: Nonce consumption is strictly atomic at the database level.

Cryptographic Irrefutability: Every hop requires a valid Ed25519 signature.

Audit Strategy
Every POST /v1/authorize request results in an AuditEvent. In production, these should be streamed to your SIEM (e.g., Datadog, Splunk) via the LogAuditSink or a custom AsyncAuditSink.


### `docs/enterprise-deployment.md`

```markdown
# Enterprise Deployment Guide

Deploying `a1-gateway` in a production environment requires strict adherence to security and high-availability (HA) best practices. This document covers the required configurations for enterprise architecture review.

## 1. Network & TLS Termination

The `a1-gateway` runs over HTTP. It **must** be deployed behind a secure reverse proxy or service mesh that handles TLS termination.

*   **Kubernetes / Service Mesh:** Deploy as a sidecar container or within a strictly isolated namespace. Use Istio/Linkerd for mTLS between the agent execution environment and the gateway.
*   **Load Balancing:** Terminate TLS at the ALB/NLB. Forward traffic to the gateway on port `8080`.

## 2. Storage & High Availability

The gateway is completely stateless. All state (nonces, revocations) is offloaded to the configured storage backend. You must deploy at least two gateway replicas for HA.

### PostgreSQL (Recommended for Persistence)

Use Postgres if you require strict ACID guarantees and persistent revocation logs.

*   **Connection Pooling:** Use PgBouncer. Do not connect the gateway directly to the core database cluster without a pooler.
*   **Configuration:** 
    ```env
    A1_PG_URL="postgres://a1_user:a1_pass@pgbouncer.internal:6432/a1_db"
    ```

### Redis (Recommended for High Throughput)

Use Redis if authorization latency is the primary constraint.

*   **HA Setup:** Deploy Redis Sentinel or Redis Cluster.
*   **Configuration:**
    
```env
    A1_REDIS_URL="redis://a1_redis_primary.internal:6379/0"
    ```

## 3. Key Management & Rotation

The gateway requires two critical keys. These **must not** be hardcoded.

*   **`A1_SIGNING_KEY_HEX`**: The Ed25519 private key used to sign `DelegationCert` issuance requests.
*   **`A1_MAC_KEY_HEX`**: The 32-byte Blake3 key used to authenticate `VerifiedToken` receipts.

**Rotation Strategy:**
1. Generate new keys using `a1 keygen`.
2. Update the secret manager (e.g., AWS Secrets Manager, HashiCorp Vault).
3. Perform a rolling restart of the gateway pods.
*(Note: Existing delegation chains remain valid after rotation; however, `VerifiedToken` receipts signed by the old MAC key will fail verification. Client SDKs should be configured to retry full authorization on MAC failure.)*

## 4. Observability & OpenTelemetry (OTEL)

If compiled with the `telemetry` feature, the gateway natively emits metrics.

*   **Metrics:** Connect Prometheus to the `/metrics` endpoint (if configured) to track authorization latency, nonce conflicts (replay attempts), and rejection rates.
*   **Audit Logs:** Ensure standard output (stderr/stdout) is ingested by Fluentd/Vector to capture NDJSON structured audit events.