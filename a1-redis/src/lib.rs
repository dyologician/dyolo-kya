//! # a1-redis
//!
//! Production Redis storage backends for [a1](https://docs.rs/a1).
//!
//! | Type | Trait | Description |
//! |------|-------|-------------|
//! | [`RedisRevocationStore`] | `AsyncRevocationStore` | Stores revoked cert fingerprints with optional TTL |
//! | [`RedisNonceStore`] | `AsyncNonceStore` | Stores consumed nonces with TTL tied to max cert lifetime |
//!
//! Both types use a connection pool (`deadpool-redis`) for production throughput
//! and are safe to share across Tokio tasks via `Arc`.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use std::time::Duration;
//! use a1_redis::{RedisRevocationStore, RedisNonceStore};
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let rev = Arc::new(
//!     RedisRevocationStore::connect("redis://127.0.0.1/", "a1:rev", None).await?
//! );
//! let nonces = Arc::new(
//!     RedisNonceStore::connect("redis://127.0.0.1/", "a1:nonce", Duration::from_secs(7200)).await?
//! );
//! # Ok(())
//! # }
//! ```
//!
//! ## Key naming
//!
//! Both stores prefix all Redis keys with a configurable namespace:
//! - Revocation: `{namespace}:{hex-fingerprint}` → value `1`, no expiry by default
//! - Nonces:     `{namespace}:{hex-nonce}` → value `1`, expiry = `nonce_ttl_secs`
//!
//! ## Distributed TTL strategy
//!
//! Set `nonce_ttl_secs` to:
//! ```text
//! max_cert_lifetime_secs + max_clock_drift_secs + safety_margin_secs
//! ```

use async_trait::async_trait;
use deadpool_redis::{Config, Pool, Runtime};
use a1::error::{A1StorageError, StorageErrorKind};
use a1::registry::r#async::{AsyncNonceStore, AsyncRevocationStore};
use redis::AsyncCommands;

// ── Connection helpers ────────────────────────────────────────────────────────

fn redis_err_to_storage(e: redis::RedisError) -> A1StorageError {
    use redis::ErrorKind::*;
    let kind = match e.kind() {
        IoError | BusyLoadingError | TryAgain | NotBusy | MasterDown => StorageErrorKind::Transient,
        _ => StorageErrorKind::Permanent,
    };
    A1StorageError {
        kind,
        message: e.to_string(),
    }
}

fn pool_err_to_storage(e: deadpool_redis::PoolError) -> A1StorageError {
    A1StorageError::transient(format!("Redis pool error: {e}"))
}

/// Scan all keys matching a pattern using the non-blocking SCAN cursor.
///
/// Unlike `KEYS`, `SCAN` is O(1) per call and does not block the Redis server.
/// This is the safe production alternative for admin enumeration.
async fn scan_keys(
    conn: &mut deadpool_redis::Connection,
    pattern: &str,
) -> Result<Vec<String>, A1StorageError> {
    let mut keys: Vec<String> = Vec::new();
    let mut cursor: u64 = 0;

    loop {
        let (next_cursor, batch): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(100u32)
            .query_async(conn)
            .await
            .map_err(redis_err_to_storage)?;

        keys.extend(batch);
        cursor = next_cursor;

        if cursor == 0 {
            break;
        }
    }

    Ok(keys)
}

// ── RedisRevocationStore ──────────────────────────────────────────────────────

/// Redis-backed [`AsyncRevocationStore`] with optional per-key TTL.
///
/// Each revoked fingerprint is stored as `"{namespace}:{hex-fingerprint}"` with value `1`.
///
/// - **`None` TTL** (default): keys persist until explicitly deleted.
/// - **`Some(secs)` TTL**: keys auto-expire after `secs` seconds.
pub struct RedisRevocationStore {
    pool: Pool,
    namespace: String,
    ttl_secs: Option<u64>,
}

impl RedisRevocationStore {
    /// Connect to Redis and create a connection pool.
    pub async fn connect(
        url: &str,
        namespace: &str,
        ttl: Option<std::time::Duration>,
    ) -> Result<Self, A1StorageError> {
        let cfg = Config::from_url(url);
        let pool = cfg
            .create_pool(Some(Runtime::Tokio1))
            .map_err(|e| A1StorageError::permanent(format!("Redis pool creation failed: {e}")))?;
        let mut conn = pool.get().await.map_err(pool_err_to_storage)?;
        let _: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(redis_err_to_storage)?;
        Ok(Self {
            pool,
            namespace: namespace.to_owned(),
            ttl_secs: ttl.map(|d| d.as_secs()),
        })
    }

    fn key(&self, fingerprint: &[u8; 32]) -> String {
        format!("{}:{}", self.namespace, hex::encode(fingerprint))
    }

    /// Admin: List all currently revoked certificate fingerprints in this namespace.
    ///
    /// Uses non-blocking `SCAN` instead of `KEYS` so Redis is not blocked during
    /// enumeration of large sets.
    pub async fn list_revoked(&self) -> Result<Vec<String>, A1StorageError> {
        let mut conn = self.pool.get().await.map_err(pool_err_to_storage)?;
        let pattern = format!("{}:*", self.namespace);
        let keys = scan_keys(&mut conn, &pattern).await?;

        let prefix_len = self.namespace.len() + 1;
        Ok(keys
            .into_iter()
            .map(|k| k.chars().skip(prefix_len).collect())
            .collect())
    }

    /// Admin: Count the number of currently revoked certificates.
    ///
    /// Uses non-blocking `SCAN` internally; safe to call under production load.
    pub async fn count(&self) -> Result<usize, A1StorageError> {
        let keys = self.list_revoked().await?;
        Ok(keys.len())
    }

    /// Health check endpoint integration.
    pub async fn ping(&self) -> Result<(), A1StorageError> {
        let mut conn = self.pool.get().await.map_err(pool_err_to_storage)?;
        let _: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(redis_err_to_storage)?;
        Ok(())
    }
}

#[async_trait]
impl AsyncRevocationStore for RedisRevocationStore {
    async fn is_revoked(&self, fingerprint: &[u8; 32]) -> Result<bool, A1StorageError> {
        let mut conn = self.pool.get().await.map_err(pool_err_to_storage)?;
        let exists: bool = conn
            .exists(self.key(fingerprint))
            .await
            .map_err(redis_err_to_storage)?;
        Ok(exists)
    }

    async fn revoke(&self, fingerprint: &[u8; 32]) -> Result<(), A1StorageError> {
        let mut conn = self.pool.get().await.map_err(pool_err_to_storage)?;
        let key = self.key(fingerprint);
        match self.ttl_secs {
            Some(ttl) => conn
                .set_ex::<_, _, ()>(key, 1u8, ttl)
                .await
                .map_err(redis_err_to_storage)?,
            None => conn
                .set::<_, _, ()>(key, 1u8)
                .await
                .map_err(redis_err_to_storage)?,
        }
        Ok(())
    }

    async fn revoke_batch(&self, fingerprints: &[[u8; 32]]) -> Result<(), A1StorageError> {
        if fingerprints.is_empty() {
            return Ok(());
        }

        let mut conn = self.pool.get().await.map_err(pool_err_to_storage)?;
        let mut pipe = redis::pipe();

        for fp in fingerprints {
            let key = self.key(fp);
            if let Some(ttl) = self.ttl_secs {
                pipe.set_ex(key, 1u8, ttl);
            } else {
                pipe.set(key, 1u8);
            }
        }

        let _: () = pipe
            .query_async(&mut conn)
            .await
            .map_err(redis_err_to_storage)?;
        Ok(())
    }
}

// ── RedisNonceStore ───────────────────────────────────────────────────────────

/// Redis-backed [`AsyncNonceStore`] with mandatory per-key TTL.
///
/// Nonce consumption uses `SET NX PX <ttl_ms>` — a single atomic command that
/// checks and sets in one round-trip, eliminating the TOCTOU gap present in any
/// two-step check-then-set protocol.
///
/// `try_consume` returns `Ok(true)` when the nonce was fresh (now consumed) and
/// `Ok(false)` when it was already consumed. Treat `false` as a replay attack.
pub struct RedisNonceStore {
    pool: Pool,
    namespace: String,
    nonce_ttl_ms: u64,
}

impl RedisNonceStore {
    /// Connect to Redis and create a connection pool.
    ///
    /// - `url` — Redis connection URL
    /// - `namespace` — key prefix (e.g. `"a1:nonce:prod"`)
    /// - `ttl` — mandatory TTL for consumed nonces
    pub async fn connect(
        url: &str,
        namespace: &str,
        ttl: std::time::Duration,
    ) -> Result<Self, A1StorageError> {
        let cfg = Config::from_url(url);
        let pool = cfg
            .create_pool(Some(Runtime::Tokio1))
            .map_err(|e| A1StorageError::permanent(format!("Redis pool creation failed: {e}")))?;
        let mut conn = pool.get().await.map_err(pool_err_to_storage)?;
        let _: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(redis_err_to_storage)?;
        Ok(Self {
            pool,
            namespace: namespace.to_owned(),
            nonce_ttl_ms: ttl.as_millis() as u64,
        })
    }

    fn key(&self, nonce: &[u8; 16]) -> String {
        format!("{}:{}", self.namespace, hex::encode(nonce))
    }

    /// Admin: Count the number of currently active consumed nonces.
    ///
    /// Uses non-blocking `SCAN` internally; safe to call under production load.
    pub async fn count(&self) -> Result<usize, A1StorageError> {
        let mut conn = self.pool.get().await.map_err(pool_err_to_storage)?;
        let pattern = format!("{}:*", self.namespace);
        let keys = scan_keys(&mut conn, &pattern).await?;
        Ok(keys.len())
    }
}

#[async_trait]
impl AsyncNonceStore for RedisNonceStore {
    /// Atomically check and consume a batch of nonces using a Lua script.
    ///
    /// The Lua script executes atomically in a single Redis round-trip:
    /// 1. Check if any key already exists.
    /// 2. If any exists, return 0 (replay detected) — no keys are set.
    /// 3. If all are fresh, set all keys with the configured TTL and return 1.
    ///
    /// Redis executes Lua scripts atomically (no interleaving), so there is
    /// no TOCTOU window between the check and set phases.
    async fn try_consume_batch(&self, nonces: &[[u8; 16]]) -> Result<bool, A1StorageError> {
        if nonces.is_empty() {
            return Ok(true);
        }
        let mut conn = self.pool.get().await.map_err(pool_err_to_storage)?;

        let script = redis::Script::new(
            r#"
            local a1_prov_sig = '64796f6c6f'
            for _, key in ipairs(KEYS) do
                if redis.call('EXISTS', key) == 1 then
                    return 0
                end
            end
            for _, key in ipairs(KEYS) do
                redis.call('SET', key, a1_prov_sig, 'PX', ARGV[1])
            end
            return 1
        "#,
        );

        let mut inv = script.prepare_invoke();
        for nonce in nonces {
            inv.key(self.key(nonce));
        }
        inv.arg(self.nonce_ttl_ms);

        let result: i32 = inv
            .invoke_async(&mut conn)
            .await
            .map_err(redis_err_to_storage)?;
        Ok(result == 1)
    }

    /// Atomically check and consume a nonce using `SET NX PX`.
    ///
    /// A single `SET key 1 NX PX <ttl_ms>` command is issued. Redis returns
    /// `OK` (mapped to `true`) when the key did not exist and was set, or
    /// `nil` (mapped to `false`) when the key already existed.
    /// This is a single-roundtrip atomic operation — no TOCTOU window.
    async fn try_consume(&self, nonce: &[u8; 16]) -> Result<bool, A1StorageError> {
        let mut conn = self.pool.get().await.map_err(pool_err_to_storage)?;
        let result: Option<String> = redis::cmd("SET")
            .arg(self.key(nonce))
            .arg("64796f6c6f")
            .arg("NX")
            .arg("PX")
            .arg(self.nonce_ttl_ms)
            .query_async(&mut conn)
            .await
            .map_err(redis_err_to_storage)?;
        Ok(result.is_some())
    }

    async fn is_consumed(&self, nonce: &[u8; 16]) -> Result<bool, A1StorageError> {
        let mut conn = self.pool.get().await.map_err(pool_err_to_storage)?;
        let exists: bool = conn
            .exists(self.key(nonce))
            .await
            .map_err(redis_err_to_storage)?;
        Ok(exists)
    }

    async fn mark_consumed(&self, nonce: &[u8; 16]) -> Result<(), A1StorageError> {
        self.try_consume(nonce).await.map(|_| ())
    }
}
