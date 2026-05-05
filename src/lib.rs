#![doc(html_logo_url = "https://raw.githubusercontent.com/dyologician/dyolo-kya/main/docs/assets/logo.png")]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/dyologician/dyolo-kya/main/docs/assets/favicon.ico")]
#![cfg_attr(docsrs, feature(doc_cfg))]
//! # dyolo-kya — Know Your Agent v2.0.0
//!
//! Cryptographic chain-of-custody for recursive AI agent delegation.
//!
//! ## What it solves
//!
//! When one AI agent delegates a task to another, the authorization chain breaks
//! down — a liability called the "Recursive Delegation Gap." This library closes
//! that gap with a native Know Your Agent (KYA) protocol: every action executed
//! by any agent in a delegation tree carries an irrefutable, cryptographically
//! verified chain proving exactly which human authorized it, with enforced scope
//! boundaries that hold offline.
//!
//! ## v2.0.0 additions over v1
//!
//! - **Namespace isolation** — `DyoloChain::with_namespace("tenant-id")` cryptographically
//!   binds a chain to a single tenant. A cert issued for tenant-a cannot authorize
//!   under tenant-b. Hard multi-tenant separation with zero configuration overhead.
//!
//! - **Storage health checks** — `NonceStore::health_check()` and
//!   `RevocationStore::health_check()` propagate to gateway `/healthz` so load
//!   balancers pull degraded instances automatically.
//!
//! - **Rate limiting trait** — `RateLimitStore` + `MemoryRateLimitStore` cap
//!   intent executions per principal per time window. Plug in a Redis or Postgres
//!   backend for distributed enforcement.
//!
//! - **KyaContext builder** — wire all dependencies in three lines:
//!   ```rust,ignore
//!   let ctx = KyaContext::builder().namespace("my-tenant").build();
//!   let action = ctx.authorize(&chain, &agent_pk, &intent, &proof)?;
//!   ```
//!
//! ## Feature flags
//!
//! | Flag          | Description                                                               |
//! |---------------|---------------------------------------------------------------------------|
//! | `serde`       | Serialization for all core types. Required for most integrations.         |
//! | `async`       | `AsyncNonceStore`, `AsyncRevocationStore`, `AsyncKyaContext`.             |
//! | `wire`        | `SignedChain`, `VerifiedToken`, `CertExtensions` (requires `serde`).      |
//! | `tracing`     | Structured `tracing` spans during authorization.                          |
//! | `ffi`         | C ABI for Python, Go, Java, and Node.js (requires `wire`).                |
//! | `policy-yaml` | Parse delegation policies from YAML files.                                |
//! | `schema`      | JSON Schema export for `SignedChain`.                                     |
//! | `full`        | All of the above except `ffi`.                                            |

#![deny(unsafe_code)]

mod crypto;

pub mod audit;
pub mod cert;
pub mod chain;
pub mod context;
pub mod error;
pub mod identity;
pub mod intent;
pub mod policy;
pub mod registry;

#[cfg(feature = "wire")]
#[cfg_attr(docsrs, doc(cfg(feature = "wire")))]
pub mod cert_extensions;

#[cfg(feature = "wire")]
#[cfg_attr(docsrs, doc(cfg(feature = "wire")))]
pub mod wire;

#[cfg(feature = "ffi")]
#[cfg_attr(docsrs, doc(cfg(feature = "ffi")))]
#[allow(unsafe_code)]
pub mod ffi;

pub use cert::{CertBuilder, CertBundle, DelegationCert, CERT_VERSION};
pub use chain::{BatchAuthorizeResult, Clock, DyoloChain, SystemClock, VerificationReceipt};
#[must_use = "Authorization receipts must be explicitly handled. Dropping an AuthorizedAction implies an action was executed without verifying its authorization receipt."]
pub use chain::AuthorizedAction;
pub use context::KyaContext;
pub use error::{KyaError, KyaStorageError, StorageErrorKind};
pub use identity::{DyoloIdentity, Signer, SharedIdentity};
pub use intent::{Intent, IntentHash, IntentTree, MerkleProof, SiblingNode, SubScopeProof, intent_hash};
pub use policy::{CapabilitySet, DelegationPolicy, PolicySet};
pub use audit::{AuditEvent, AuditOutcome, AuditSink, CompositeAuditSink, LogAuditSink, NoopAuditSink};
pub use registry::{
    MemoryNonceStore, MemoryRevocationStore, MemoryRateLimitStore,
    NonceStore, RevocationStore, RateLimitStore, fresh_nonce,
};

#[cfg(feature = "wire")]
#[cfg_attr(docsrs, doc(cfg(feature = "wire")))]
pub use cert_extensions::{CertExtensions, ExtValue};

#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
pub use context::AsyncKyaContext;

#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
pub use registry::r#async::{
    AsyncNonceStore, AsyncRevocationStore, AsyncRateLimitStore,
    SyncNonceAdapter, SyncRevocationAdapter,
};

#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
pub use audit::r#async::{AsyncAuditSink, SyncAuditAdapter};

#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
pub use identity::AsyncSigner;