use std::net::IpAddr;
use std::num::NonZeroU32;
use std::sync::Arc;

use governor::{clock::DefaultClock, state::keyed::DefaultKeyedStateStore, Quota, RateLimiter};

use a1::registry::r#async::{AsyncNonceStore, AsyncRevocationStore};
use a1::{
    DyoloIdentity, MemoryNonceStore, MemoryRevocationStore, SyncNonceAdapter, SyncRevocationAdapter,
};
use zeroize::Zeroizing;

/// Per-IP rate limiter keyed by [`IpAddr`].
pub type IpLimiter = Arc<RateLimiter<IpAddr, DefaultKeyedStateStore<IpAddr>, DefaultClock>>;

pub struct AppState {
    pub signing_identity: DyoloIdentity,
    pub revocation: Arc<dyn AsyncRevocationStore>,
    pub nonces: Arc<dyn AsyncNonceStore>,
    pub mac_key: Zeroizing<[u8; 32]>,
    pub rate_limiter: IpLimiter,
    pub gateway_pk_hex: String,
    pub admin_secret: Option<String>,
    /// HTTPS endpoint to push authorization events to (optional SIEM integration).
    pub webhook_url: Option<String>,
    /// BLAKE3-HMAC signing key for outbound webhook payloads.
    pub webhook_secret: Option<String>,
}

impl AppState {
    pub async fn from_env() -> anyhow::Result<Self> {
        let signing_identity = match std::env::var("A1_SIGNING_KEY_HEX") {
            Ok(hex_str) => {
                let bytes = hex::decode(&hex_str)
                    .map_err(|_| anyhow::anyhow!("A1_SIGNING_KEY_HEX must be valid hex"))?;
                let arr: [u8; 32] = bytes
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("A1_SIGNING_KEY_HEX must be exactly 32 bytes (64 hex chars)"))?;
                if arr == [0u8; 32] {
                    anyhow::bail!(
                        "A1_SIGNING_KEY_HEX cannot be all zeros — \
                         generate a real key with `a1 keygen` and set it in your environment."
                    );
                }
                DyoloIdentity::from_signing_bytes(&arr)
            }
            Err(_) => {
                tracing::warn!(
                    "A1_SIGNING_KEY_HEX not set — generating ephemeral signing key. \
                     This key will be lost on restart. Set A1_SIGNING_KEY_HEX for production."
                );
                DyoloIdentity::generate()
            }
        };

        let mac_key = Zeroizing::new(match std::env::var("A1_MAC_KEY_HEX") {
            Ok(hex_str) => {
                let bytes = hex::decode(&hex_str)?;
                bytes
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("A1_MAC_KEY_HEX must be 32 bytes"))?
            }
            Err(_) => {
                tracing::warn!(
                    "A1_MAC_KEY_HEX not set — generating ephemeral MAC key. \
                     VerifiedTokens will not survive restart. Set A1_MAC_KEY_HEX for production."
                );
                let mut base = [0u8; 32];
                rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut base);
                let mut h = blake3::Hasher::new_derive_key("a1::64796f6c6f::mac::ephemeral::v2.8.0");
                h.update(&base);
                h.finalize().into()
            }
        });

        let admin_secret = std::env::var("A1_ADMIN_SECRET").ok();
        if admin_secret.is_none() {
            tracing::warn!(
                "A1_ADMIN_SECRET not set — certificate issuance endpoints are unprotected. \
                 Set A1_ADMIN_SECRET for production deployments."
            );
        }

        let mut revocation: Arc<dyn AsyncRevocationStore> = Arc::new(SyncRevocationAdapter(
            Arc::new(MemoryRevocationStore::new()),
        ));
        let mut nonces: Arc<dyn AsyncNonceStore> =
            Arc::new(SyncNonceAdapter(Arc::new(MemoryNonceStore::new())));
        let mut using_ephemeral_storage = true;

        if let Ok(redis_url) = std::env::var("A1_REDIS_URL") {
            tracing::info!("Connecting to Redis: {}", redis_url);
            let redis_rev =
                a1_redis::RedisRevocationStore::connect(&redis_url, "a1:rev", None).await?;
            let redis_nonces = a1_redis::RedisNonceStore::connect(
                &redis_url,
                "a1:nonce",
                std::time::Duration::from_secs(86400),
            )
            .await?;
            revocation = Arc::new(redis_rev);
            nonces = Arc::new(redis_nonces);
            using_ephemeral_storage = false;
        } else if let Ok(pg_url) = std::env::var("A1_PG_URL") {
            tracing::info!("Connecting to Postgres: {}", pg_url);
            let pool = sqlx::PgPool::connect(&pg_url).await?;
            revocation = Arc::new(a1_pg::PgRevocationStore::new(pool.clone()));
            nonces = Arc::new(a1_pg::PgNonceStore::new(pool));
            using_ephemeral_storage = false;
        }

        if using_ephemeral_storage {
            tracing::warn!(
                "No A1_REDIS_URL or A1_PG_URL configured. \
                 Using in-memory storage: revocation and nonce state WILL BE LOST on restart. \
                 This is acceptable for local development only. \
                 Production deployments MUST set one of these environment variables."
            );
        }

        let rps = std::env::var("A1_RATE_LIMIT_RPS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(500);
        let rps = NonZeroU32::new(rps.max(1)).unwrap();

        let rate_limiter = Arc::new(
            RateLimiter::<IpAddr, DefaultKeyedStateStore<IpAddr>, DefaultClock>::keyed(
                Quota::per_second(rps),
            ),
        );

        let gateway_pk_hex = hex::encode(signing_identity.verifying_key().as_bytes());

        let webhook_url    = std::env::var("A1_WEBHOOK_URL").ok();
        let webhook_secret = std::env::var("A1_WEBHOOK_SECRET").ok();

        if webhook_url.is_some() {
            tracing::info!("webhook delivery enabled");
        }

        Ok(Self {
            signing_identity,
            revocation,
            nonces,
            mac_key,
            rate_limiter,
            gateway_pk_hex,
            admin_secret,
            webhook_url,
            webhook_secret,
        })
    }
}