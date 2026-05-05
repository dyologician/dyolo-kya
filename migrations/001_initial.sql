-- dyolo-kya v2.0.0
-- Enterprise KYA Migration: Initial Schema

CREATE TABLE IF NOT EXISTS dyolo_kya_revocations (
    tenant_id   VARCHAR NOT NULL DEFAULT '',
    fingerprint BYTEA NOT NULL,
    revoked_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, fingerprint)
);

CREATE INDEX IF NOT EXISTS idx_kya_revocations_revoked_at ON dyolo_kya_revocations(revoked_at);

CREATE TABLE IF NOT EXISTS dyolo_kya_nonces (
    tenant_id   VARCHAR NOT NULL DEFAULT '',
    nonce       BYTEA NOT NULL,
    consumed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, nonce)
);

CREATE INDEX IF NOT EXISTS idx_kya_nonces_consumed_at ON dyolo_kya_nonces(consumed_at);