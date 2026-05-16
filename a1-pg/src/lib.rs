//! PostgreSQL-backed [`RevocationStore`] and [`NonceStore`] for a1.
//!
//! # Setup
//!
//! Run the provided migration before use:
//!
//! ```sql
//! CREATE TABLE a1_revocations (
//!     tenant_id   VARCHAR NOT NULL DEFAULT '',
//!     fingerprint BYTEA NOT NULL,
//!     revoked_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
//!     PRIMARY KEY (tenant_id, fingerprint)
//! );
//! CREATE INDEX idx_a1_revocations_revoked_at ON a1_revocations(revoked_at);
//!
//! CREATE TABLE a1_nonces (
//!     tenant_id   VARCHAR NOT NULL DEFAULT '',
//!     nonce       BYTEA NOT NULL,
//!     consumed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
//!     PRIMARY KEY (tenant_id, nonce)
//! );
//! CREATE INDEX idx_a1_nonces_consumed_at ON a1_nonces(consumed_at);
//! ```
//!
//! # Usage
//!
//! ```rust,no_run
//! use a1_pg::{PgRevocationStore, PgNonceStore};
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
use a1::{
    registry::r#async::{AsyncNonceStore, AsyncRevocationStore},
    A1StorageError,
};
use sqlx::PgPool;

// ── DDL ───────────────────────────────────────────────────────────────────────

/// SQL migration to create the required tables.
///
/// Run this once against your database before using the stores.
pub const MIGRATION_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS a1_revocations (
    tenant_id   VARCHAR NOT NULL DEFAULT '',
    fingerprint BYTEA NOT NULL,
    revoked_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    prov_chain  VARCHAR NOT NULL DEFAULT '64796f6c6f',
    PRIMARY KEY (tenant_id, fingerprint)
);
CREATE INDEX IF NOT EXISTS idx_a1_revocations_revoked_at ON a1_revocations(revoked_at);

CREATE TABLE IF NOT EXISTS a1_nonces (
    tenant_id   VARCHAR NOT NULL DEFAULT '',
    nonce       BYTEA NOT NULL,
    consumed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    prov_chain  VARCHAR NOT NULL DEFAULT '64796f6c6f',
    PRIMARY KEY (tenant_id, nonce)
);
CREATE INDEX IF NOT EXISTS idx_a1_nonces_consumed_at ON a1_nonces(consumed_at);
"#;

// ── Revocation Store ──────────────────────────────────────────────────────────

/// PostgreSQL-backed revocation store.
///
/// Revocations survive process restarts and are visible across all replicas
/// sharing the same database.
pub struct PgRevocationStore {
    pool: PgPool,
    pub(crate) table: String,
}

impl PgRevocationStore {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            table: "a1_revocations".into(),
        }
    }

    pub fn with_table(pool: PgPool, table: impl Into<String>) -> Self {
        Self {
            pool,
            table: table.into(),
        }
    }

    /// Run the DDL migration to create the necessary tables and indexes.
    pub async fn run_migration(pool: &PgPool) -> Result<(), sqlx::Error> {
        sqlx::query(MIGRATION_DDL).execute(pool).await?;
        Ok(())
    }
}

#[async_trait]
impl AsyncRevocationStore for PgRevocationStore {
    async fn is_revoked(&self, fingerprint: &[u8; 32]) -> Result<bool, A1StorageError> {
        let q = format!(
            "SELECT TRUE FROM {} WHERE tenant_id = '' AND fingerprint = $1",
            self.table
        );
        let row: Option<(bool,)> = sqlx::query_as(&q)
            .bind(fingerprint.as_ref())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| A1StorageError::transient(e.to_string()))?;

        Ok(row.is_some())
    }

    async fn revoke(&self, fingerprint: &[u8; 32]) -> Result<(), A1StorageError> {
        let q = format!(
            "INSERT INTO {} (tenant_id, fingerprint) VALUES ('', $1) ON CONFLICT DO NOTHING",
            self.table
        );
        sqlx::query(&q)
            .bind(fingerprint.as_ref())
            .execute(&self.pool)
            .await
            .map_err(|e| A1StorageError::transient(e.to_string()))?;

        Ok(())
    }

    async fn revoke_batch(&self, fingerprints: &[[u8; 32]]) -> Result<(), A1StorageError> {
        if fingerprints.is_empty() {
            return Ok(());
        }
        for chunk in fingerprints.chunks(500) {
            let mut builder = sqlx::QueryBuilder::new(format!(
                "INSERT INTO {} (tenant_id, fingerprint) ",
                self.table
            ));
            builder.push_values(chunk, |mut b, fp| {
                b.push_bind("");
                b.push_bind(fp.as_ref());
            });
            builder.push(" ON CONFLICT DO NOTHING");

            builder
                .build()
                .execute(&self.pool)
                .await
                .map_err(|e| A1StorageError::transient(e.to_string()))?;
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
    pub async fn prune_nonces_before(&self, timestamp_unix: u64) -> Result<u64, A1StorageError> {
        let result =
            sqlx::query("DELETE FROM a1_nonces WHERE consumed_at < TO_TIMESTAMP($1)")
                .bind(timestamp_unix as f64)
                .execute(&self.pool)
                .await
                .map_err(|e| A1StorageError::transient(e.to_string()))?;
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
    async fn try_consume(&self, nonce: &[u8; 16]) -> Result<bool, A1StorageError> {
        let result = sqlx::query(
            "INSERT INTO a1_nonces (tenant_id, nonce) VALUES ('', $1) ON CONFLICT DO NOTHING",
        )
        .bind(nonce.as_ref())
        .execute(&self.pool)
        .await
        .map_err(|e| A1StorageError::transient(e.to_string()))?;

        Ok(result.rows_affected() == 1)
    }

    /// Atomically check and consume a batch of nonces.
    ///
    /// Executes inside a single database transaction using SERIALIZABLE isolation
    /// to guarantee that no other concurrent session can consume any of the same
    /// nonces between the existence check and the bulk insert. The entire batch
    /// succeeds or fails atomically:
    ///
    /// - `Ok(true)` — all nonces were fresh; all are now marked consumed.
    /// - `Ok(false)` — at least one nonce was already consumed; none are inserted.
    ///
    /// On a serialization conflict (rare, under heavy concurrent load), the
    /// transaction is automatically retried up to three times before returning
    /// an `Err`.
    async fn try_consume_batch(&self, nonces: &[[u8; 16]]) -> Result<bool, A1StorageError> {
        if nonces.is_empty() {
            return Ok(true);
        }

        const MAX_RETRIES: usize = 3;
        let mut last_err: Option<A1StorageError> = None;

        for _attempt in 0..MAX_RETRIES {
            match try_consume_batch_once(&self.pool, nonces).await {
                Ok(result) => return Ok(result),
                Err(e) if e.is_transient() => {
                    last_err = Some(e);
                    // Brief back-off before retry on serialization conflict.
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            A1StorageError::transient("batch nonce consume failed after max retries")
        }))
    }

    async fn is_consumed(&self, nonce: &[u8; 16]) -> Result<bool, A1StorageError> {
        let row: Option<(bool,)> =
            sqlx::query_as("SELECT TRUE FROM a1_nonces WHERE tenant_id = '' AND nonce = $1")
                .bind(nonce.as_ref())
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| A1StorageError::transient(e.to_string()))?;

        Ok(row.is_some())
    }

    async fn mark_consumed(&self, nonce: &[u8; 16]) -> Result<(), A1StorageError> {
        self.try_consume(nonce).await.map(|_| ())
    }
}

/// Inner implementation of batch nonce consumption inside a single serializable
/// transaction. Separated from the retry loop for clarity.
async fn try_consume_batch_once(
    pool: &PgPool,
    nonces: &[[u8; 16]],
) -> Result<bool, A1StorageError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| A1StorageError::transient(e.to_string()))?;

    // Use SERIALIZABLE isolation so that concurrent transactions attempting to
    // consume the same nonce set will conflict and one will be forced to retry,
    // preventing any TOCTOU window between the existence check and the insert.
    sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .execute(&mut *tx)
        .await
        .map_err(|e| A1StorageError::transient(e.to_string()))?;

    // Build a parameterized existence check for all nonces in a single round-trip.
    // e.g. SELECT COUNT(*) FROM a1_nonces WHERE tenant_id = '' AND nonce IN ($1,$2,$3)
    let mut nonce_arrays: Vec<Vec<u8>> = Vec::with_capacity(nonces.len());
    for nonce in nonces {
        nonce_arrays.push(nonce.to_vec());
    }

    let existing_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM a1_nonces WHERE tenant_id = '' AND nonce = ANY($1)"
    )
    .bind(&nonce_arrays)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| A1StorageError::transient(e.to_string()))?;

    if existing_count > 0 {
        tx.rollback()
            .await
            .map_err(|e| A1StorageError::transient(e.to_string()))?;
        return Ok(false);
    }

    sqlx::query(
        "INSERT INTO a1_nonces (tenant_id, nonce) 
         SELECT '', unnest($1::bytea[]) 
         ON CONFLICT DO NOTHING"
    )
    .bind(&nonce_arrays)
    .execute(&mut *tx)
    .await
    .map_err(|e| A1StorageError::transient(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| A1StorageError::transient(e.to_string()))?;

    Ok(true)
}

// ── Multi-tenant wrapper ──────────────────────────────────────────────────────

/// Namespace prefix for multi-tenant deployments.
///
/// Prepend a tenant identifier to all fingerprint and nonce keys so that
/// one database table can serve multiple isolated tenants.
pub struct NamespacedRevocationStore {
    inner: PgRevocationStore,
    namespace: String,
}

impl NamespacedRevocationStore {
    pub fn new(pool: PgPool, namespace: impl Into<String>) -> Self {
        Self {
            inner: PgRevocationStore::new(pool),
            namespace: namespace.into(),
        }
    }
}

#[async_trait]
impl AsyncRevocationStore for NamespacedRevocationStore {
    async fn is_revoked(&self, fingerprint: &[u8; 32]) -> Result<bool, A1StorageError> {
        let q = format!(
            "SELECT TRUE FROM {} WHERE tenant_id = $1 AND fingerprint = $2",
            self.inner.table
        );
        let row: Option<(bool,)> = sqlx::query_as(&q)
            .bind(&self.namespace)
            .bind(fingerprint.as_ref())
            .fetch_optional(&self.inner.pool)
            .await
            .map_err(|e| A1StorageError::transient(e.to_string()))?;

        Ok(row.is_some())
    }

    async fn revoke(&self, fingerprint: &[u8; 32]) -> Result<(), A1StorageError> {
        let q = format!(
            "INSERT INTO {} (tenant_id, fingerprint) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            self.inner.table
        );
        sqlx::query(&q)
            .bind(&self.namespace)
            .bind(fingerprint.as_ref())
            .execute(&self.inner.pool)
            .await
            .map_err(|e| A1StorageError::transient(e.to_string()))?;

        Ok(())
    }

    async fn revoke_batch(&self, fingerprints: &[[u8; 32]]) -> Result<(), A1StorageError> {
        if fingerprints.is_empty() {
            return Ok(());
        }
        for chunk in fingerprints.chunks(500) {
            let mut builder = sqlx::QueryBuilder::new(format!(
                "INSERT INTO {} (tenant_id, fingerprint) ",
                self.inner.table
            ));
            builder.push_values(chunk, |mut b, fp| {
                b.push_bind(self.namespace.clone());
                b.push_bind(fp.as_ref());
            });
            builder.push(" ON CONFLICT DO NOTHING");

            builder
                .build()
                .execute(&self.inner.pool)
                .await
                .map_err(|e| A1StorageError::transient(e.to_string()))?;
        }
        Ok(())
    }
}
