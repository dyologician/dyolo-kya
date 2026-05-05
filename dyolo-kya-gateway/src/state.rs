use std::net::SocketAddr;
use std::num::NonZeroU32;
use std::sync::Arc;

use governor::{
    Quota, RateLimiter,
    clock::DefaultClock,
    state::keyed::DefaultKeyedStateStore,
};

use zeroize::Zeroizing;
use dyolo_kya::registry::r#async::{AsyncRevocationStore, AsyncNonceStore};
use dyolo_kya::{
    DyoloIdentity, MemoryNonceStore, MemoryRevocationStore,
    SyncNonceAdapter, SyncRevocationAdapter,
};

/// Per-IP rate limiter keyed by [`SocketAddr`].
///
/// Uses `governor`'s keyed state store so that each unique client address
/// gets an independent token bucket. Global-bucket limiters allowed any
/// single client to exhaust the entire gateway budget; per-IP bucketing
/// isolates misbehaving clients while honest callers stay unthrottled.
pub type IpLimiter = Arc<RateLimiter<SocketAddr, DefaultKeyedStateStore<SocketAddr>, DefaultClock>>;

pub struct AppState {
    pub signing_identity: DyoloIdentity,
    pub revocation:       Arc<dyn AsyncRevocationStore>,
    pub nonces:           Arc<dyn AsyncNonceStore>,
    pub mac_key:          Zeroizing<[u8; 32]>,
    pub rate_limiter:     IpLimiter,
    pub gateway_pk_hex:   String,
    pub admin_secret:     Option<String>,
}

impl AppState {
    pub async fn from_env() -> anyhow::Result<Self> {
        let signing_identity = match std::env::var("DYOLO_SIGNING_KEY_HEX") {
            Ok(hex_str) => {
                let bytes = hex::decode(&hex_str)?;
                let arr: [u8; 32] = bytes.try_into()
                    .map_err(|_| anyhow::anyhow!("DYOLO_SIGNING_KEY_HEX must be 32 bytes"))?;
                DyoloIdentity::from_signing_bytes(&arr)
            }
            Err(_) => {
                tracing::warn!("DYOLO_SIGNING_KEY_HEX not set — generating ephemeral signing key");
                DyoloIdentity::generate()
            }
        };

        let mac_key = Zeroizing::new(match std::env::var("DYOLO_MAC_KEY_HEX") {
            Ok(hex_str) => {
                let bytes = hex::decode(&hex_str)?;
                bytes.try_into()
                    .map_err(|_| anyhow::anyhow!("DYOLO_MAC_KEY_HEX must be 32 bytes"))?
            }
            Err(_) => {
                tracing::warn!("DYOLO_MAC_KEY_HEX not set — generating ephemeral MAC key");
                let mut key = [0u8; 32];
                rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut key);
                key
            }
        });

        let admin_secret = std::env::var("DYOLO_ADMIN_SECRET").ok();

        let mut revocation: Arc<dyn AsyncRevocationStore> = Arc::new(SyncRevocationAdapter(Arc::new(MemoryRevocationStore::new())));
        let mut nonces: Arc<dyn AsyncNonceStore> = Arc::new(SyncNonceAdapter(Arc::new(MemoryNonceStore::new())));

        if let Ok(redis_url) = std::env::var("DYOLO_REDIS_URL") {
            tracing::info!("Connecting to Redis: {}", redis_url);
            let redis_rev = dyolo_kya_redis::RedisRevocationStore::connect(&redis_url, "kya:rev", None).await?;
            let redis_nonces = dyolo_kya_redis::RedisNonceStore::connect(&redis_url, "kya:nonce", std::time::Duration::from_secs(86400)).await?;
            revocation = Arc::new(redis_rev);
            nonces = Arc::new(redis_nonces);
        } else if let Ok(pg_url) = std::env::var("DYOLO_PG_URL") {
            tracing::info!("Connecting to Postgres: {}", pg_url);
            let pool = sqlx::PgPool::connect(&pg_url).await?;
            revocation = Arc::new(dyolo_kya_pg::PgRevocationStore::new(pool.clone()));
            nonces = Arc::new(dyolo_kya_pg::PgNonceStore::new(pool));
        } else {
            tracing::warn!("No DYOLO_REDIS_URL or DYOLO_PG_URL set — using ephemeral in-memory storage");
        }

        let rps = std::env::var("DYOLO_RATE_LIMIT_RPS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(500);
        let rps = NonZeroU32::new(rps.max(1)).unwrap();

        let rate_limiter = Arc::new(
            RateLimiter::<SocketAddr, DefaultKeyedStateStore<SocketAddr>, DefaultClock>::keyed(
                Quota::per_second(rps),
            ),
        );

        let gateway_pk_hex = hex::encode(signing_identity.verifying_key().as_bytes());

        Ok(Self {
            signing_identity,
            revocation,
            nonces,
            mac_key,
            rate_limiter,
            gateway_pk_hex,
            admin_secret,
        })
    }
}
