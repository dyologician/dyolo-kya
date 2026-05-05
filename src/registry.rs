use std::collections::{HashMap, HashSet};
use std::sync::{
    RwLock,
    atomic::{AtomicU64, Ordering},
};
use std::time::{SystemTime, UNIX_EPOCH};

use rand::{rngs::OsRng, RngCore};

use crate::error::KyaStorageError;

pub fn fresh_nonce() -> [u8; 16] {
    let mut nonce = [0u8; 16];
    OsRng.fill_bytes(&mut nonce);
    nonce
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── RevocationStore ───────────────────────────────────────────────────────────

pub trait RevocationStore: Send + Sync {
    fn is_revoked(&self, fingerprint: &[u8; 32]) -> Result<bool, KyaStorageError>;
    fn revoke(&self, fingerprint: &[u8; 32]) -> Result<(), KyaStorageError>;

    fn revoke_batch(&self, fingerprints: &[[u8; 32]]) -> Result<(), KyaStorageError> {
        for fp in fingerprints {
            self.revoke(fp)?;
        }
        Ok(())
    }

    fn health_check(&self) -> Result<(), KyaStorageError> {
        Ok(())
    }
}

// ── NonceStore ────────────────────────────────────────────────────────────────

pub trait NonceStore: Send + Sync {
    fn try_consume(&self, nonce: &[u8; 16]) -> Result<bool, KyaStorageError>;

    fn try_consume_batch(&self, nonces: &[[u8; 16]]) -> Result<bool, KyaStorageError> {
        for nonce in nonces {
            if self.is_consumed(nonce)? {
                return Ok(false);
            }
        }
        for nonce in nonces {
            self.mark_consumed(nonce)?;
        }
        Ok(true)
    }

    fn is_consumed(&self, nonce: &[u8; 16]) -> Result<bool, KyaStorageError>;
    fn mark_consumed(&self, nonce: &[u8; 16]) -> Result<(), KyaStorageError>;

    fn health_check(&self) -> Result<(), KyaStorageError> {
        Ok(())
    }
}

// ── RateLimitStore ────────────────────────────────────────────────────────────

/// Token-bucket rate limiter storage abstraction.
///
/// `check_and_record` must atomically verify that the key has not exceeded its
/// budget, then record the attempt. Returns `Ok(true)` when the request is
/// within the limit and `Ok(false)` when the limit has been reached.
///
/// Implement this trait over Redis (sliding-window counter with INCR + EXPIRE)
/// or Postgres (per-key row with an atomic UPDATE) for distributed deployments.
pub trait RateLimitStore: Send + Sync {
    fn check_and_record(
        &self,
        key:         &[u8],
        max_per_window: u32,
        window_secs: u64,
    ) -> Result<bool, KyaStorageError>;

    fn health_check(&self) -> Result<(), KyaStorageError> {
        Ok(())
    }
}

// ── MemoryRevocationStore ─────────────────────────────────────────────────────

pub struct MemoryRevocationStore {
    bloom:  Vec<AtomicU64>,
    shards: Vec<RwLock<HashSet<[u8; 32]>>>,
}

impl MemoryRevocationStore {
    #[must_use]
    pub fn new() -> Self {
        let bloom  = (0..64).map(|_| AtomicU64::new(0)).collect();
        let shards = (0..64).map(|_| RwLock::new(HashSet::new())).collect();
        Self { bloom, shards }
    }

    #[inline(always)]
    fn bloom_indices(fp: &[u8; 32]) -> [(usize, u64); 3] {
        let h1 = u64::from_le_bytes(fp[0..8].try_into().unwrap());
        let h2 = u64::from_le_bytes(fp[8..16].try_into().unwrap());
        let h3 = u64::from_le_bytes(fp[16..24].try_into().unwrap());
        [
            ((h1 % 64) as usize, 1u64 << (h1.rotate_right(13) % 64)),
            ((h2 % 64) as usize, 1u64 << (h2.rotate_right(27) % 64)),
            ((h3 % 64) as usize, 1u64 << (h3.rotate_right(41) % 64)),
        ]
    }

    #[inline(always)]
    fn shard_index(fp: &[u8; 32]) -> usize {
        (u64::from_le_bytes(fp[24..32].try_into().unwrap()) % 64) as usize
    }
}

impl Default for MemoryRevocationStore {
    fn default() -> Self { Self::new() }
}

impl RevocationStore for MemoryRevocationStore {
    fn is_revoked(&self, fingerprint: &[u8; 32]) -> Result<bool, KyaStorageError> {
        for (word, bit) in Self::bloom_indices(fingerprint) {
            if (self.bloom[word].load(Ordering::Relaxed) & bit) == 0 {
                return Ok(false);
            }
        }
        let shard = self.shards[Self::shard_index(fingerprint)]
            .read()
            .map_err(|_| KyaStorageError::permanent("revocation shard lock poisoned"))?;
        Ok(shard.contains(fingerprint))
    }

    fn revoke(&self, fingerprint: &[u8; 32]) -> Result<(), KyaStorageError> {
        for (word, bit) in Self::bloom_indices(fingerprint) {
            self.bloom[word].fetch_or(bit, Ordering::SeqCst);
        }
        self.shards[Self::shard_index(fingerprint)]
            .write()
            .map_err(|_| KyaStorageError::permanent("revocation shard lock poisoned"))?
            .insert(*fingerprint);
        Ok(())
    }

    fn revoke_batch(&self, fingerprints: &[[u8; 32]]) -> Result<(), KyaStorageError> {
        for fp in fingerprints {
            for (word, bit) in Self::bloom_indices(fp) {
                self.bloom[word].fetch_or(bit, Ordering::SeqCst);
            }
        }
        for fp in fingerprints {
            self.shards[Self::shard_index(fp)]
                .write()
                .map_err(|_| KyaStorageError::permanent("revocation shard lock poisoned"))?
                .insert(*fp);
        }
        Ok(())
    }
}

// ── MemoryNonceStore ──────────────────────────────────────────────────────────

pub struct MemoryNonceStore {
    bloom:    Vec<AtomicU64>,
    store:    RwLock<HashMap<[u8; 16], u64>>,
    ttl_secs: Option<u64>,
}

impl MemoryNonceStore {
    const BLOOM_WORDS: usize = 1024;

    #[must_use]
    pub fn new() -> Self {
        let bloom = (0..Self::BLOOM_WORDS).map(|_| AtomicU64::new(0)).collect();
        Self { bloom, store: RwLock::new(HashMap::new()), ttl_secs: None }
    }

    pub fn with_ttl_secs(mut self, ttl_secs: u64) -> Self {
        self.ttl_secs = Some(ttl_secs);
        self
    }

    #[inline(always)]
    fn indices(nonce: &[u8; 16]) -> (usize, u64) {
        let e1 = u64::from_le_bytes(nonce[0..8].try_into().unwrap());
        let e2 = u64::from_le_bytes(nonce[8..16].try_into().unwrap());
        let h   = e1.wrapping_mul(0x9E3779B185EBCA87).wrapping_add(e2.rotate_left(23));
        let word = (h as usize) % Self::BLOOM_WORDS;
        let bit  = 1u64 << (h.rotate_right(11) % 64);
        (word, bit)
    }
}

impl Default for MemoryNonceStore {
    fn default() -> Self { Self::new() }
}

impl NonceStore for MemoryNonceStore {
    fn try_consume(&self, nonce: &[u8; 16]) -> Result<bool, KyaStorageError> {
        let (word, bit) = Self::indices(nonce);
        let mut guard = self.store
            .write()
            .map_err(|_| KyaStorageError::permanent("nonce store lock poisoned"))?;

        let now = unix_now();

        if (self.bloom[word].load(Ordering::Acquire) & bit) != 0 {
            if let Some(&exp) = guard.get(nonce) {
                if exp >= now {
                    return Ok(false);
                }
            }
        }

        if let Some(ttl) = self.ttl_secs {
            guard.retain(|_, exp| *exp >= now);
            guard.insert(*nonce, now.saturating_add(ttl));
        } else {
            guard.insert(*nonce, u64::MAX);
        }

        self.bloom[word].fetch_or(bit, Ordering::Release);
        Ok(true)
    }

    fn try_consume_batch(&self, nonces: &[[u8; 16]]) -> Result<bool, KyaStorageError> {
        let mut guard = self.store
            .write()
            .map_err(|_| KyaStorageError::permanent("nonce store lock poisoned"))?;

        let now = unix_now();

        for nonce in nonces {
            let (word, bit) = Self::indices(nonce);
            if (self.bloom[word].load(Ordering::Acquire) & bit) != 0 {
                if let Some(&exp) = guard.get(nonce) {
                    if exp >= now {
                        return Ok(false);
                    }
                }
            }
        }

        if let Some(ttl) = self.ttl_secs {
            guard.retain(|_, exp| *exp >= now);
            for nonce in nonces {
                let (word, bit) = Self::indices(nonce);
                guard.insert(*nonce, now.saturating_add(ttl));
                self.bloom[word].fetch_or(bit, Ordering::Release);
            }
        } else {
            for nonce in nonces {
                let (word, bit) = Self::indices(nonce);
                guard.insert(*nonce, u64::MAX);
                self.bloom[word].fetch_or(bit, Ordering::Release);
            }
        }

        Ok(true)
    }

    fn is_consumed(&self, nonce: &[u8; 16]) -> Result<bool, KyaStorageError> {
        let (word, bit) = Self::indices(nonce);
        if (self.bloom[word].load(Ordering::Acquire) & bit) == 0 {
            return Ok(false);
        }
        let guard = self.store
            .read()
            .map_err(|_| KyaStorageError::permanent("nonce store lock poisoned"))?;
        let now = unix_now();
        if let Some(&exp) = guard.get(nonce) {
            Ok(exp >= now)
        } else {
            Ok(false)
        }
    }

    fn mark_consumed(&self, nonce: &[u8; 16]) -> Result<(), KyaStorageError> {
        self.try_consume(nonce).map(|_| ())
    }
}

// ── MemoryRateLimitStore ──────────────────────────────────────────────────────

struct RateBucket {
    window_start_secs: u64,
    count:             u32,
}

/// In-process sliding-window rate limiter.
///
/// Each unique key (e.g. principal public key bytes, or IP address bytes) gets
/// an independent bucket. Buckets reset at the start of each window and entries
/// are evicted lazily on the next write after they expire.
pub struct MemoryRateLimitStore {
    buckets: RwLock<HashMap<Vec<u8>, RateBucket>>,
}

impl MemoryRateLimitStore {
    #[must_use]
    pub fn new() -> Self {
        Self { buckets: RwLock::new(HashMap::new()) }
    }
}

impl Default for MemoryRateLimitStore {
    fn default() -> Self { Self::new() }
}

impl RateLimitStore for MemoryRateLimitStore {
    fn check_and_record(
        &self,
        key:            &[u8],
        max_per_window: u32,
        window_secs:    u64,
    ) -> Result<bool, KyaStorageError> {
        let now = unix_now();
        let window_start = (now / window_secs.max(1)) * window_secs.max(1);

        let mut buckets = self.buckets
            .write()
            .map_err(|_| KyaStorageError::permanent("rate limit store lock poisoned"))?;

        buckets.retain(|_, v| {
            now.saturating_sub(v.window_start_secs) < window_secs.max(1) * 2
        });

        let bucket = buckets.entry(key.to_vec()).or_insert(RateBucket {
            window_start_secs: window_start,
            count: 0,
        });

        if bucket.window_start_secs != window_start {
            bucket.window_start_secs = window_start;
            bucket.count = 0;
        }

        if bucket.count >= max_per_window {
            return Ok(false);
        }

        bucket.count += 1;
        Ok(true)
    }
}

// ── Async storage traits ──────────────────────────────────────────────────────

#[cfg(feature = "async")]
pub mod r#async {
    use std::sync::Arc;
    use async_trait::async_trait;
    use crate::error::KyaStorageError;

    #[async_trait]
    pub trait AsyncRevocationStore: Send + Sync {
        async fn is_revoked(&self, fingerprint: &[u8; 32]) -> Result<bool, KyaStorageError>;
        async fn revoke(&self, fingerprint: &[u8; 32]) -> Result<(), KyaStorageError>;

        async fn revoke_batch(&self, fingerprints: &[[u8; 32]]) -> Result<(), KyaStorageError> {
            for fp in fingerprints { self.revoke(fp).await?; }
            Ok(())
        }

        async fn health_check(&self) -> Result<(), KyaStorageError> {
            Ok(())
        }
    }

    #[async_trait]
    pub trait AsyncNonceStore: Send + Sync {
        async fn try_consume(&self, nonce: &[u8; 16]) -> Result<bool, KyaStorageError>;

        async fn try_consume_batch(&self, nonces: &[[u8; 16]]) -> Result<bool, KyaStorageError> {
            for nonce in nonces {
                if self.is_consumed(nonce).await? {
                    return Ok(false);
                }
            }
            for nonce in nonces {
                self.mark_consumed(nonce).await?;
            }
            Ok(true)
        }

        async fn is_consumed(&self, nonce: &[u8; 16]) -> Result<bool, KyaStorageError>;
        async fn mark_consumed(&self, nonce: &[u8; 16]) -> Result<(), KyaStorageError>;

        async fn health_check(&self) -> Result<(), KyaStorageError> {
            Ok(())
        }
    }

    #[async_trait]
    pub trait AsyncRateLimitStore: Send + Sync {
        async fn check_and_record(
            &self,
            key:            &[u8],
            max_per_window: u32,
            window_secs:    u64,
        ) -> Result<bool, KyaStorageError>;

        async fn health_check(&self) -> Result<(), KyaStorageError> {
            Ok(())
        }
    }

    pub struct SyncRevocationAdapter<S>(pub Arc<S>);

    #[async_trait]
    impl<S: super::RevocationStore + 'static> AsyncRevocationStore for SyncRevocationAdapter<S> {
        async fn is_revoked(&self, fingerprint: &[u8; 32]) -> Result<bool, KyaStorageError> {
            let store = Arc::clone(&self.0);
            let fp = *fingerprint;
            tokio::task::spawn_blocking(move || store.is_revoked(&fp))
                .await.map_err(|e| KyaStorageError::transient(e.to_string()))?
        }

        async fn revoke(&self, fingerprint: &[u8; 32]) -> Result<(), KyaStorageError> {
            let store = Arc::clone(&self.0);
            let fp = *fingerprint;
            tokio::task::spawn_blocking(move || store.revoke(&fp))
                .await.map_err(|e| KyaStorageError::transient(e.to_string()))?
        }

        async fn revoke_batch(&self, fingerprints: &[[u8; 32]]) -> Result<(), KyaStorageError> {
            let store = Arc::clone(&self.0);
            let fps: Vec<[u8; 32]> = fingerprints.to_vec();
            tokio::task::spawn_blocking(move || store.revoke_batch(&fps))
                .await.map_err(|e| KyaStorageError::transient(e.to_string()))?
        }

        async fn health_check(&self) -> Result<(), KyaStorageError> {
            let store = Arc::clone(&self.0);
            tokio::task::spawn_blocking(move || store.health_check())
                .await.map_err(|e| KyaStorageError::transient(e.to_string()))?
        }
    }

    pub struct SyncNonceAdapter<S>(pub Arc<S>);

    #[async_trait]
    impl<S: super::NonceStore + 'static> AsyncNonceStore for SyncNonceAdapter<S> {
        async fn try_consume(&self, nonce: &[u8; 16]) -> Result<bool, KyaStorageError> {
            let store = Arc::clone(&self.0);
            let n = *nonce;
            tokio::task::spawn_blocking(move || store.try_consume(&n))
                .await.map_err(|e| KyaStorageError::transient(e.to_string()))?
        }

        async fn try_consume_batch(&self, nonces: &[[u8; 16]]) -> Result<bool, KyaStorageError> {
            let store = Arc::clone(&self.0);
            let ns = nonces.to_vec();
            tokio::task::spawn_blocking(move || store.try_consume_batch(&ns))
                .await.map_err(|e| KyaStorageError::transient(e.to_string()))?
        }

        async fn is_consumed(&self, nonce: &[u8; 16]) -> Result<bool, KyaStorageError> {
            let store = Arc::clone(&self.0);
            let n = *nonce;
            tokio::task::spawn_blocking(move || store.is_consumed(&n))
                .await.map_err(|e| KyaStorageError::transient(e.to_string()))?
        }

        async fn mark_consumed(&self, nonce: &[u8; 16]) -> Result<(), KyaStorageError> {
            let store = Arc::clone(&self.0);
            let n = *nonce;
            tokio::task::spawn_blocking(move || store.mark_consumed(&n))
                .await.map_err(|e| KyaStorageError::transient(e.to_string()))?
        }

        async fn health_check(&self) -> Result<(), KyaStorageError> {
            let store = Arc::clone(&self.0);
            tokio::task::spawn_blocking(move || store.health_check())
                .await.map_err(|e| KyaStorageError::transient(e.to_string()))?
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_consume_is_atomic_and_idempotent() {
        let store = MemoryNonceStore::new();
        let nonce = fresh_nonce();
        assert!(store.try_consume(&nonce).unwrap());
        assert!(!store.try_consume(&nonce).unwrap());
        assert!(store.is_consumed(&nonce).unwrap());
    }

    #[test]
    fn try_consume_concurrent_exactly_one_winner() {
        use std::{sync::Arc, thread};
        let store = Arc::new(MemoryNonceStore::new());
        let nonce = fresh_nonce();
        let handles: Vec<_> = (0..32)
            .map(|_| { let s = Arc::clone(&store); thread::spawn(move || s.try_consume(&nonce).unwrap()) })
            .collect();
        let wins: usize = handles.into_iter().map(|h| h.join().unwrap() as usize).sum();
        assert_eq!(wins, 1);
    }

    #[test]
    fn revoke_batch_marks_all_fingerprints() {
        let store = MemoryRevocationStore::new();
        let fps: Vec<[u8; 32]> = (0..8u8).map(|i| { let mut f = [0u8; 32]; f[0] = i; f }).collect();
        store.revoke_batch(&fps).unwrap();
        for fp in &fps { assert!(store.is_revoked(fp).unwrap()); }
    }

    #[test]
    fn health_check_returns_ok_for_memory_stores() {
        assert!(MemoryRevocationStore::new().health_check().is_ok());
        assert!(MemoryNonceStore::new().health_check().is_ok());
        assert!(MemoryRateLimitStore::new().health_check().is_ok());
    }

    #[test]
    fn rate_limit_enforces_window() {
        let store = MemoryRateLimitStore::new();
        let key = b"test-principal";
        for _ in 0..5 {
            assert!(store.check_and_record(key, 5, 60).unwrap(), "should be allowed");
        }
        assert!(!store.check_and_record(key, 5, 60).unwrap(), "should be blocked");
    }
}