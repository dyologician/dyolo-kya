//! Enterprise identity bridging for dyolo-kya.
//!
//! Binds existing enterprise identity systems (JWT/OIDC, OAuth2, SPIFFE) to
//! Ed25519 delegation keys so that `DyoloIdentity` keys are not isolated islands
//! — they are anchored to your existing authorization infrastructure.
//!
//! # Features
//!
//! - **`jwt`** (default) — [`JwtBinding`]: bind an Ed25519 key to a JWT `sub` claim.
//!   Verify that the JWT is currently valid before trusting the key.
//! - **`spiffe`** — [`SpiffeBinding`]: bind an Ed25519 key to a SPIFFE X.509 SVID.
//!   Enables service meshes (Istio, Linkerd) to securely delegate workloads.
//! - **`policy`** — [`PolicyDocument`]: issue delegation certificates from YAML/JSON
//!   without writing Rust code. Suitable for ops teams and GitOps workflows.
//!
//! # Example — JWT binding
//!
//! ```rust,no_run
//! use dyolo_kya_identity::{JwtBinding, JwtVerificationOptions};
//! use dyolo_kya::DyoloIdentity;
//!
//! let identity = DyoloIdentity::generate();
//! let binding  = JwtBinding::new(
//!     "alice@corp.example",                // JWT sub claim
//!     identity.verifying_key(),
//! );
//! let signed = binding.sign(&identity);    // binding is signed by the identity key
//!
//! // On verification (requires `jwt` feature):
//! use jsonwebtoken::{DecodingKey, Algorithm};
//! let opts = JwtVerificationOptions::new("https://idp.corp.example", "my-api-audience");
//! let decoding_key = DecodingKey::from_rsa_pem(b"-----BEGIN PUBLIC KEY...").unwrap();
//! // signed.verify_jwt_and_sub("eyJhbG...", &decoding_key, &opts, Algorithm::RS256).unwrap();
//! ```

pub mod jwt_binding;
pub mod policy;
#[cfg(feature = "spiffe")]
pub mod spiffe_binding;

pub use jwt_binding::{JwtBinding, SignedJwtBinding};
#[cfg(feature = "jwt")]
pub use jwt_binding::JwtVerificationOptions;
pub use policy::{PolicyDocument, PolicyCert, PolicyIntent};

#[cfg(feature = "spiffe")]
pub use spiffe_binding::{SpiffeBinding, SignedSpiffeBinding};
