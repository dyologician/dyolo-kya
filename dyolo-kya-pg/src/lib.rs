//! PostgreSQL-backed [`RevocationStore`] and [`NonceStore`] for dyolo-kya.
//!
//! # Setup
//!
//! Run the provided migration before use:
//!
//! ```sql
//! CREATE TABLE dyolo_kya_revocations (
//!     tenant_id   VARCHAR NOT NULL DEFAULT '',
//!     fingerprint BYTEA NOT NULL,
//!     revoked_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
//!     PRIMARY KEY (tenant_id, fingerprint)
//! );
//! CREATE INDEX idx_kya_revocations_revoked_at ON dyolo_kya_revocations(revoked_at);
//!
//! CREATE TABLE dyolo_kya_nonces (
//!     tenant_id   VARCHAR NOT NULL DEFAULT '',
//!     nonce       BYTEA NOT NULL,
//!     consumed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
//!     PRIMARY KEY (tenant_id, nonce)
//! );
//! CREATE INDEX idx_kya_nonces_consumed_at ON dyolo_kya_nonces(consumed_at);
//! ```
//!
//! # Usage
//!
//! ```rust,no_run
//! use dyolo_kya_pg::{PgRevocationStore, PgNonceStore};
//! use sqlx::PgPool;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let pool = PgPool::connect("postgres://localhost/mydb").await?;
//!     let rev    = PgRevocationStore::new(pool.clone());
//!     let nonces = PgNonceStore::new(pool);
//!     // Pass to DyoloChain::authorize_async
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use dyolo_kya::{
    KyaStorageError,
    registry::r#async::{AsyncRevocationStore, AsyncNonceStore},
};
use sqlx::PgPool;

// ── DDL ───────────────────────────────────────────────────────────────────────

/// SQL migration to create the required tables.
///
/// Run this once against your database before using the stores.
pub const MIGRATION_DDL: &str = r#"
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
"#;

// ── Revocation Store ──────────────────────────────────────────────────────────

/// PostgreSQL-backed revocation store.
///
/// Revocations survive process restarts and are visible across all replicas
/// sharing the same database.
pub struct PgRevocationStore {
    pool:  PgPool,
    pub(crate) table: String,
}

impl PgRevocationStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool, table: "dyolo_kya_revocations".into() }
    }

    pub fn with_table(pool: PgPool, table: impl Into<String>) -> Self {
        Self { pool, table: table.into() }
    }

    /// Run the DDL migration to create the necessary tables and indexes.
    pub async fn run_migration(pool: &PgPool) -> Result<(), sqlx::Error> {
        sqlx::query(MIGRATION_DDL).execute(pool).await?;
        Ok(())
    }
}

#[async_trait]
impl AsyncRevocationStore for PgRevocationStore {
    async fn is_revoked(&self, fingerprint: &[u8; 32]) -> Result<bool, KyaStorageError> {
        let q = format!("SELECT TRUE FROM {} WHERE tenant_id = '' AND fingerprint = $1", self.table);
        let row: Option<(bool,)> = sqlx::query_as(&q)
            .bind(fingerprint.as_ref())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| KyaStorageError::transient(e.to_string()))?;

        Ok(row.is_some())
    }

    async fn revoke(&self, fingerprint: &[u8; 32]) -> Result<(), KyaStorageError> {
        let q = format!("INSERT INTO {} (tenant_id, fingerprint) VALUES ('', $1) ON CONFLICT DO NOTHING", self.table);
        sqlx::query(&q)
            .bind(fingerprint.as_ref())
            .execute(&self.pool)
            .await
            .map_err(|e| KyaStorageError::transient(e.to_string()))?;

        Ok(())
    }

    async fn revoke_batch(&self, fingerprints: &[[u8; 32]]) -> Result<(), KyaStorageError> {
        if fingerprints.is_empty() { return Ok(()); }
        for chunk in fingerprints.chunks(500) {
            let mut builder = sqlx::QueryBuilder::new(format!("INSERT INTO {} (tenant_id, fingerprint) ", self.table));
            builder.push_values(chunk, |mut b, fp| {
                b.push_bind("");
                b.push_bind(fp.as_ref());
            });
            builder.push(" ON CONFLICT DO NOTHING");
            
            builder.build()
                .execute(&self.pool)
                .await
                .map_err(|e| KyaStorageError::transient(e.to_string()))?;
        }
        Ok(())
    }
}

// ── Nonce Store ───────────────────────────────────────────────────────────────

/// PostgreSQL-backed nonce store.
///
/// Nonces are never evicted by this store. Prune old nonces by deleting rows
/// where `consumed_at < NOW() - INTERVAL '7 days'` on a schedule, coordinated
/// with your maximum certificate lifetime.
pub struct PgNonceStore {
    pool: PgPool,
}

impl PgNonceStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Prune old nonces from the database.
    ///
    /// Call this periodically to prevent the nonce table from growing indefinitely.
    pub async fn prune_nonces_before(&self, timestamp_unix: u64) -> Result<u64, KyaStorageError> {
        let result = sqlx::query("DELETE FROM dyolo_kya_nonces WHERE consumed_at < TO_TIMESTAMP($1)")
            .bind(timestamp_unix as f64)
            .execute(&self.pool)
            .await
            .map_err(|e| KyaStorageError::transient(e.to_string()))?;
        Ok(result.rows_affected())
    }
}

#[async_trait]
impl AsyncNonceStore for PgNonceStore {
    /// Atomically check and consume a nonce.
    ///
    /// Issues a single `INSERT ... ON CONFLICT DO NOTHING` and reads
    /// `rows_affected()`. Returns `Ok(true)` if the nonce was newly inserted
    /// (fresh), `Ok(false)` if the INSERT was suppressed because the nonce
    /// already existed (replay). This is a single-roundtrip atomic operation
    /// with no TOCTOU window between check and write.
    async fn try_consume(&self, nonce: &[u8; 16]) -> Result<bool, KyaStorageError> {
        let result = sqlx::query(
            "INSERT INTO dyolo_kya_nonces (tenant_id, nonce) VALUES ('', $1) ON CONFLICT DO NOTHING",
        )
        .bind(nonce.as_ref())
        .execute(&self.pool)
        .await
        .map_err(|e| KyaStorageError::transient(e.to_string()))?;

        Ok(result.rows_affected() == 1)
    }

    async fn is_consumed(&self, nonce: &[u8; 16]) -> Result<bool, KyaStorageError> {
        let row: Option<(bool,)> = sqlx::query_as(
            "SELECT TRUE FROM dyolo_kya_nonces WHERE tenant_id = '' AND nonce = $1",
        )
        .bind(nonce.as_ref())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| KyaStorageError::transient(e.to_string()))?;

        Ok(row.is_some())
    }

    async fn mark_consumed(&self, nonce: &[u8; 16]) -> Result<(), KyaStorageError> {
        self.try_consume(nonce).await.map(|_| ())
    }
}

// ── Multi-tenant wrapper ──────────────────────────────────────────────────────

/// Namespace prefix for multi-tenant deployments.
///
/// Prepend a tenant identifier to all fingerprint and nonce keys so that
/// one database table can serve multiple isolated tenants.
pub struct NamespacedRevocationStore {
    inner:     PgRevocationStore,
    namespace: String,
}

impl NamespacedRevocationStore {
    pub fn new(pool: PgPool, namespace: impl Into<String>) -> Self {
        Self { inner: PgRevocationStore::new(pool), namespace: namespace.into() }
    }
}

#[async_trait]
impl AsyncRevocationStore for NamespacedRevocationStore {
    async fn is_revoked(&self, fingerprint: &[u8; 32]) -> Result<bool, KyaStorageError> {
        let q = format!("SELECT TRUE FROM {} WHERE tenant_id = $1 AND fingerprint = $2", self.inner.table);
        let row: Option<(bool,)> = sqlx::query_as(&q)
            .bind(&self.namespace)
            .bind(fingerprint.as_ref())
            .fetch_optional(&self.inner.pool)
            .await
            .map_err(|e| KyaStorageError::transient(e.to_string()))?;

        Ok(row.is_some())
    }

    async fn revoke(&self, fingerprint: &[u8; 32]) -> Result<(), KyaStorageError> {
        let q = format!("INSERT INTO {} (tenant_id, fingerprint) VALUES ($1, $2) ON CONFLICT DO NOTHING", self.inner.table);
        sqlx::query(&q)
            .bind(&self.namespace)
            .bind(fingerprint.as_ref())
            .execute(&self.inner.pool)
            .await
            .map_err(|e| KyaStorageError::transient(e.to_string()))?;

        Ok(())
    }

    async fn revoke_batch(&self, fingerprints: &[[u8; 32]]) -> Result<(), KyaStorageError> {
        if fingerprints.is_empty() { return Ok(()); }
        for chunk in fingerprints.chunks(500) {
            let mut builder = sqlx::QueryBuilder::new(format!("INSERT INTO {} (tenant_id, fingerprint) ", self.inner.table));
            builder.push_values(chunk, |mut b, fp| {
                b.push_bind(self.namespace.clone());
                b.push_bind(fp.as_ref());
            });
            builder.push(" ON CONFLICT DO NOTHING");
            
            builder.build()
                .execute(&self.inner.pool)
                .await
                .map_err(|e| KyaStorageError::transient(e.to_string()))?;
        }
        Ok(())
    }
}
