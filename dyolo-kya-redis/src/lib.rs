//! # dyolo-kya-redis
//!
//! Production Redis storage backends for [dyolo-kya](https://docs.rs/dyolo-kya).
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
//! use dyolo_kya_redis::{RedisRevocationStore, RedisNonceStore};
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let rev = Arc::new(
//!     RedisRevocationStore::connect("redis://127.0.0.1/", "kya:rev", None).await?
//! );
//! let nonces = Arc::new(
//!     RedisNonceStore::connect("redis://127.0.0.1/", "kya:nonce", 7200).await?
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
use dyolo_kya::error::{KyaStorageError, StorageErrorKind};
use dyolo_kya::registry::r#async::{AsyncNonceStore, AsyncRevocationStore};
use redis::AsyncCommands;

// ── Connection helpers ────────────────────────────────────────────────────────

fn redis_err_to_storage(e: redis::RedisError) -> KyaStorageError {
    use redis::ErrorKind::*;
    let kind = match e.kind() {
        IoError | BusyLoadingError | TryAgain | NotBusy | ClusterConnectionError
        | MasterDown => StorageErrorKind::Transient,
        _ => StorageErrorKind::Permanent, // ResponseError is permanent (e.g. wrong type/args)
    };
    KyaStorageError { kind, message: e.to_string() }
}

fn pool_err_to_storage(e: deadpool_redis::PoolError) -> KyaStorageError {
    KyaStorageError::transient(format!("Redis pool error: {e}"))
}

// ── RedisRevocationStore ──────────────────────────────────────────────────────

/// Redis-backed [`AsyncRevocationStore`] with optional per-key TTL.
///
/// Each revoked fingerprint is stored as `"{namespace}:{hex-fingerprint}"` with value `1`.
///
/// - **`None` TTL** (default): keys persist until explicitly deleted.
/// - **`Some(secs)` TTL**: keys auto-expire after `secs` seconds.
pub struct RedisRevocationStore {
    pool:      Pool,
    namespace: String,
    ttl_secs:  Option<u64>,
}

impl RedisRevocationStore {
    /// Connect to Redis and create a connection pool.
    pub async fn connect(
        url: &str,
        namespace: &str,
        ttl: Option<std::time::Duration>,
    ) -> Result<Self, KyaStorageError> {
        let cfg  = Config::from_url(url);
        let pool = cfg.create_pool(Some(Runtime::Tokio1))
            .map_err(|e| KyaStorageError::permanent(format!("Redis pool creation failed: {e}")))?;
        let mut conn = pool.get().await.map_err(pool_err_to_storage)?;
        let _: String = redis::cmd("PING").query_async(&mut conn).await
            .map_err(redis_err_to_storage)?;
        Ok(Self { pool, namespace: namespace.to_owned(), ttl_secs: ttl.map(|d| d.as_secs()) })
    }

    fn key(&self, fingerprint: &[u8; 32]) -> String {
        format!("{}:{}", self.namespace, hex::encode(fingerprint))
    }

    /// Admin: List all currently revoked certificate fingerprints in this namespace.
    pub async fn list_revoked(&self) -> Result<Vec<String>, KyaStorageError> {
        let mut conn = self.pool.get().await.map_err(pool_err_to_storage)?;
        let pattern = format!("{}:*", self.namespace);
        let keys: Vec<String> = redis::cmd("KEYS").arg(&pattern).query_async(&mut conn).await
            .map_err(redis_err_to_storage)?;
        
        let prefix_len = self.namespace.len() + 1;
        Ok(keys.into_iter().map(|k| k.chars().skip(prefix_len).collect()).collect())
    }

    /// Admin: Count the number of currently revoked certificates.
    pub async fn count(&self) -> Result<usize, KyaStorageError> {
        let keys = self.list_revoked().await?;
        Ok(keys.len())
    }

    /// Health check endpoint integration.
    pub async fn ping(&self) -> Result<(), KyaStorageError> {
        let mut conn = self.pool.get().await.map_err(pool_err_to_storage)?;
        let _: String = redis::cmd("PING").query_async(&mut conn).await
            .map_err(redis_err_to_storage)?;
        Ok(())
    }
}

#[async_trait]
impl AsyncRevocationStore for RedisRevocationStore {
    async fn is_revoked(&self, fingerprint: &[u8; 32]) -> Result<bool, KyaStorageError> {
        let mut conn = self.pool.get().await.map_err(pool_err_to_storage)?;
        let exists: bool = conn.exists(self.key(fingerprint)).await
            .map_err(redis_err_to_storage)?;
        Ok(exists)
    }

    async fn revoke(&self, fingerprint: &[u8; 32]) -> Result<(), KyaStorageError> {
        let mut conn = self.pool.get().await.map_err(pool_err_to_storage)?;
        let key = self.key(fingerprint);
        match self.ttl_secs {
            Some(ttl) => conn.set_ex::<_, _, ()>(key, 1u8, ttl).await
                .map_err(redis_err_to_storage)?,
            None => conn.set::<_, _, ()>(key, 1u8).await
                .map_err(redis_err_to_storage)?,
        }
        Ok(())
    }

    async fn revoke_batch(&self, fingerprints: &[[u8; 32]]) -> Result<(), KyaStorageError> {
        if fingerprints.is_empty() { return Ok(()); }
        
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
        
        pipe.query_async(&mut conn).await.map_err(redis_err_to_storage)?;
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
    pool:          Pool,
    namespace:     String,
    nonce_ttl_ms:  u64,
}

impl RedisNonceStore {
    /// Connect to Redis and create a connection pool.
    ///
    /// - `url` — Redis connection URL
    /// - `namespace` — key prefix (e.g. `"kya:nonce:prod"`)
    /// - `ttl` — mandatory TTL for consumed nonces
    pub async fn connect(
        url: &str,
        namespace: &str,
        ttl: std::time::Duration,
    ) -> Result<Self, KyaStorageError> {
        let cfg  = Config::from_url(url);
        let pool = cfg.create_pool(Some(Runtime::Tokio1))
            .map_err(|e| KyaStorageError::permanent(format!("Redis pool creation failed: {e}")))?;
        let mut conn = pool.get().await.map_err(pool_err_to_storage)?;
        let _: String = redis::cmd("PING").query_async(&mut conn).await
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
    pub async fn count(&self) -> Result<usize, KyaStorageError> {
        let mut conn = self.pool.get().await.map_err(pool_err_to_storage)?;
        let pattern = format!("{}:*", self.namespace);
        let keys: Vec<String> = redis::cmd("KEYS").arg(&pattern).query_async(&mut conn).await
            .map_err(redis_err_to_storage)?;
        Ok(keys.len())
    }
}

#[async_trait]
impl AsyncNonceStore for RedisNonceStore {
    /// Atomically check and consume a batch of nonces using a Lua script.
    ///
    /// This ensures all nonces are evaluated in a single atomic transaction.
    /// If any nonce already exists, the script returns 0 and no keys are set.
    /// Otherwise, all nonces are set with the configured TTL and returns 1.
    async fn try_consume_batch(&self, nonces: &[[u8; 16]]) -> Result<bool, KyaStorageError> {
        if nonces.is_empty() { return Ok(true); }
        let mut conn = self.pool.get().await.map_err(pool_err_to_storage)?;
        
        let script = redis::Script::new(r#"
            for _, key in ipairs(KEYS) do
                if redis.call('EXISTS', key) == 1 then
                    return 0
                end
            end
            for _, key in ipairs(KEYS) do
                redis.call('SET', key, 1, 'PX', ARGV[1])
            end
            return 1
        "#);
        
        let mut inv = script.prepare_invoke();
        for nonce in nonces {
            inv.key(self.key(nonce));
        }
        inv.arg(self.nonce_ttl_ms);
        
        let result: i32 = inv.invoke_async(&mut conn).await.map_err(redis_err_to_storage)?;
        Ok(result == 1)
    }

    /// Atomically check and consume a nonce using `SET NX PX`.
    ///
    /// A single `SET key 1 NX PX <ttl_ms>` command is issued. Redis returns
    /// `OK` (mapped to `true`) when the key did not exist and was set, or
    /// `nil` (mapped to `false`) when the key already existed.
    /// This is a single-roundtrip atomic operation — no TOCTOU window.
    async fn try_consume(&self, nonce: &[u8; 16]) -> Result<bool, KyaStorageError> {
        let mut conn = self.pool.get().await.map_err(pool_err_to_storage)?;
        // SET NX PX returns the string "OK" on success, or nil on conflict.
        // Mapping to Option<String>: Some(_) = newly set (fresh), None = already existed.
        let result: Option<String> = redis::cmd("SET")
            .arg(self.key(nonce))
            .arg(1u8)
            .arg("NX")
            .arg("PX")
            .arg(self.nonce_ttl_ms)
            .query_async(&mut conn)
            .await
            .map_err(redis_err_to_storage)?;
        Ok(result.is_some())
    }

    async fn is_consumed(&self, nonce: &[u8; 16]) -> Result<bool, KyaStorageError> {
        let mut conn = self.pool.get().await.map_err(pool_err_to_storage)?;
        let exists: bool = conn.exists(self.key(nonce)).await
            .map_err(redis_err_to_storage)?;
        Ok(exists)
    }

    async fn mark_consumed(&self, nonce: &[u8; 16]) -> Result<(), KyaStorageError> {
        self.try_consume(nonce).await.map(|_| ())
    }
}