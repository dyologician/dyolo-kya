# A1-redis

Production Redis storage backends for [a1](https://crates.io/crates/a1).

Provides `RedisRevocationStore` and `RedisNonceStore` that implement the
`AsyncRevocationStore` and `AsyncNonceStore` traits using a `deadpool-redis`
connection pool.

## Features

- **Connection pooling** via `deadpool-redis` — no connection-per-request overhead
- **`SET NX EX`** for nonce consumption — atomic across distributed workers
- **Configurable namespace** — run multiple deployments in the same Redis instance
- **TTL guidance** — nonce expiry tied to maximum certificate lifetime
- **Typed errors** — `A1StorageError::Transient` for network failures (retry-safe),
  `A1StorageError::Permanent` for data errors (alert immediately)

## Usage

```toml
[dependencies]
a1       = { version = "2.8", features = ["async"] }
a1-redis = "2.8"
```

```rust
use a1_redis::{RedisRevocationStore, RedisNonceStore};

// nonce_ttl_secs = max_cert_expiry + drift_tolerance + safety_margin
// e.g.             3600             + 15              + 300            = 3915
let rev    = RedisRevocationStore::connect("redis://127.0.0.1/", "a1:rev", None).await?;
let nonces = RedisNonceStore::connect("redis://127.0.0.1/", "a1:nonce", 3915).await?;

let action = chain.authorize_async(
    &agent_pk, &intent, &proof, &SystemClock, &rev, &nonces,
).await?;
```

## Key naming

- Revocation: `{namespace}:{hex-fingerprint}` → `"1"` (no expiry by default)
- Nonces:     `{namespace}:{hex-nonce}` → `"1"` (expiry = `nonce_ttl_secs`)

## License

MIT OR Apache-2.0
