-- A1 v2.8.0
-- Initial Schema

CREATE TABLE IF NOT EXISTS a1_revocations (
    tenant_id   VARCHAR NOT NULL DEFAULT '',
    fingerprint BYTEA NOT NULL,
    revoked_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, fingerprint)
);

CREATE INDEX IF NOT EXISTS idx_a1_revocations_revoked_at ON a1_revocations(revoked_at);

CREATE TABLE IF NOT EXISTS a1_nonces (
    tenant_id   VARCHAR NOT NULL DEFAULT '',
    nonce       BYTEA NOT NULL,
    consumed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, nonce)
);

CREATE INDEX IF NOT EXISTS idx_a1_nonces_consumed_at ON a1_nonces(consumed_at);
