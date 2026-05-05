# dyolo-kya-redis

Production Redis storage backends for [dyolo-kya](https://crates.io/crates/dyolo-kya).

Provides `RedisRevocationStore` and `RedisNonceStore` that implement the
`AsyncRevocationStore` and `AsyncNonceStore` traits using a `deadpool-redis`
connection pool.

## Features

- **Connection pooling** via `deadpool-redis` — no connection-per-request overhead
- **`SET NX EX`** for nonce consumption — atomic across distributed workers
- **Configurable namespace** — run multiple deployments in the same Redis instance
- **TTL guidance** — nonce expiry tied to maximum certificate lifetime
- **Typed errors** — `KyaStorageError::Transient` for network failures (retry-safe),
  `KyaStorageError::Permanent` for data errors (alert immediately)

## Usage

```toml
[dependencies]
dyolo-kya       = { version = "2.0.0", features = ["async"] }
dyolo-kya-redis = "2.0.0"
```

```rust
use dyolo_kya_redis::{RedisRevocationStore, RedisNonceStore};

// nonce_ttl_secs = max_cert_expiry + drift_tolerance + safety_margin
// e.g.             3600             + 15              + 300            = 3915
let rev    = RedisRevocationStore::connect("redis://127.0.0.1/", "kya:rev", None).await?;
let nonces = RedisNonceStore::connect("redis://127.0.0.1/", "kya:nonce", 3915).await?;

let action = chain.authorize_async(
    &agent_pk, &intent, &proof, &SystemClock, &rev, &nonces,
).await?;
```

## Key naming

- Revocation: `{namespace}:{hex-fingerprint}` → `"1"` (no expiry by default)
- Nonces:     `{namespace}:{hex-nonce}` → `"1"` (expiry = `nonce_ttl_secs`)

## License

MIT OR Apache-2.0
