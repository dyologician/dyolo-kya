//! Bind an Ed25519 verifying key to a JWT `sub` claim.
//!
//! This lets enterprises prove that a dyolo-kya principal key is controlled by
//! the same entity that holds a JWT issued by their IdP (Okta, Auth0, Azure AD,
//! Cognito, etc.).  Without this binding, Ed25519 keys are standalone islands
//! that security teams cannot map back to their identity directory — and will
//! reject in architecture review.
//!
//! # Protocol
//!
//! 1. The principal holds a JWT from their IdP with `sub = "user@corp.example"`.
//! 2. They also hold a `DyoloIdentity` with an Ed25519 keypair.
//! 3. They create a [`JwtBinding`] attesting that this Ed25519 key corresponds
//!    to that `sub` claim, and sign it with the Ed25519 key.
//! 4. The binding is published alongside the delegation chain.
//! 5. Verifiers call [`SignedJwtBinding::verify_sub_matches`] to confirm the
//!    binding before accepting the chain from that principal.
//!
//! The JWT itself is not stored in the binding — only the `sub` claim and the
//! verifying key. This keeps the binding privacy-preserving: the subject
//! identifier is only as sensitive as the binding caller makes it.

use blake3::Hasher;
use ed25519_dalek::{Signature, VerifyingKey, Verifier};
use serde::{Deserialize, Serialize};

use dyolo_kya::Signer;

const DOMAIN_JWT_BINDING: &str = "dyolo::kya::identity::jwt_binding::v1";

/// A claim that an Ed25519 `verifying_key` is controlled by the entity
/// identified by `sub` in a JWT from `issuer`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtBinding {
    /// The JWT issuer URL (e.g. `"https://accounts.google.com"`).
    pub issuer:        String,
    /// The JWT `sub` claim value (e.g. `"alice@corp.example"` or a UUID).
    pub sub:           String,
    /// The Ed25519 verifying key being bound to this subject.
    pub verifying_key: String,
    /// Unix timestamp when this binding was created.
    pub bound_at:      u64,
}

/// A [`JwtBinding`] signed by the Ed25519 key it binds.
///
/// The signature proves that the holder of the private key explicitly created
/// this binding — it cannot be forged by a third party who only knows the
/// public key and the JWT.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedJwtBinding {
    pub binding:   JwtBinding,
    pub signature: String,
}

pub struct JwtBindingBuilder {
    issuer: String,
    sub:    String,
}

impl Default for JwtBindingBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl JwtBindingBuilder {
    pub fn new() -> Self {
        Self {
            issuer: String::new(),
            sub:    String::new(),
        }
    }

    pub fn issuer(mut self, issuer: impl Into<String>) -> Self {
        self.issuer = issuer.into();
        self
    }

    pub fn sub(mut self, sub: impl Into<String>) -> Self {
        self.sub = sub.into();
        self
    }

    pub fn build(self, vk: &VerifyingKey) -> JwtBinding {
        let mut binding = JwtBinding::new(self.sub, vk);
        binding.issuer = self.issuer;
        binding
    }
}

impl JwtBinding {
    /// Create a new builder for `JwtBinding`.
    pub fn builder() -> JwtBindingBuilder {
        JwtBindingBuilder::new()
    }

    /// Create a new binding for `sub` and the Ed25519 `verifying_key`.
    pub fn new(sub: impl Into<String>, vk: &VerifyingKey) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock before epoch")
            .as_secs();
        Self {
            issuer:        String::new(),
            sub:           sub.into(),
            verifying_key: hex::encode(vk.as_bytes()),
            bound_at:      now,
        }
    }

    /// Sign this binding with the private key corresponding to `verifying_key`.
    ///
    /// The `signer` must hold the private key for `self.verifying_key`.
    pub fn sign(self, signer: &dyn Signer) -> SignedJwtBinding {
        let msg = self.signable_bytes();
        let sig = signer.sign_message(&msg);
        SignedJwtBinding {
            binding:   self,
            signature: hex::encode(sig.to_bytes()),
        }
    }

    fn signable_bytes(&self) -> Vec<u8> {
        let mut h = Hasher::new_derive_key(DOMAIN_JWT_BINDING);
        for field in [self.issuer.as_str(), self.sub.as_str(), self.verifying_key.as_str()] {
            h.update(&(field.len() as u64).to_le_bytes());
            h.update(field.as_bytes());
        }
        h.update(&self.bound_at.to_be_bytes());
        h.finalize().as_bytes().to_vec()
    }
}

#[cfg(feature = "jwt")]
#[derive(Debug, Clone)]
pub struct JwtVerificationOptions {
    pub expected_issuer: String,
    pub expected_audience: String,
}

#[cfg(feature = "jwt")]
impl JwtVerificationOptions {
    pub fn new(issuer: impl Into<String>, audience: impl Into<String>) -> Self {
        Self {
            expected_issuer: issuer.into(),
            expected_audience: audience.into(),
        }
    }
}

impl SignedJwtBinding {
    /// Verify the actual JWT signature, expiry, and audience using `jsonwebtoken`,
    /// then assert that the binding's `sub` matches the validated JWT `sub`.
    #[cfg(feature = "jwt")]
    pub fn verify_jwt_and_sub(
        &self,
        token: &str,
        key: &jsonwebtoken::DecodingKey,
        opts: &JwtVerificationOptions,
        algorithm: jsonwebtoken::Algorithm,
    ) -> Result<(), JwtBindingError> {
        self.verify_signature()?;

        let mut validation = jsonwebtoken::Validation::new(algorithm);
        validation.set_issuer(&[opts.expected_issuer.clone()]);
        validation.set_audience(&[opts.expected_audience.clone()]);

        #[derive(serde::Deserialize)]
        struct Claims {
            sub: String,
        }

        let token_data = jsonwebtoken::decode::<Claims>(token, key, &validation)
            .map_err(|e| JwtBindingError::JwtValidationFailed(e.to_string()))?;

        if self.binding.sub != token_data.claims.sub {
            return Err(JwtBindingError::SubMismatch {
                expected: token_data.claims.sub,
                got:      self.binding.sub.clone(),
            });
        }

        Ok(())
    }

    /// Verify that the signature is valid for the embedded binding.
    pub fn verify_signature(&self) -> Result<(), JwtBindingError> {
        let pk_bytes = hex::decode(&self.binding.verifying_key)
            .map_err(|_| JwtBindingError::InvalidKey("hex decode failed".into()))?;
        let pk_arr: [u8; 32] = pk_bytes.try_into()
            .map_err(|_| JwtBindingError::InvalidKey("must be 32 bytes".into()))?;
        let vk = VerifyingKey::from_bytes(&pk_arr)
            .map_err(|e| JwtBindingError::InvalidKey(e.to_string()))?;

        let sig_bytes = hex::decode(&self.signature)
            .map_err(|_| JwtBindingError::InvalidSignature)?;
        let sig_arr: [u8; 64] = sig_bytes.try_into()
            .map_err(|_| JwtBindingError::InvalidSignature)?;
        let sig = Signature::from_bytes(&sig_arr);

        let msg = self.binding.signable_bytes();
        vk.verify(&msg, &sig).map_err(|_| JwtBindingError::InvalidSignature)
    }

    /// Assert that the binding's `sub` matches `expected_sub`.
    ///
    /// Call this after verifying the JWT from your IdP to confirm the binding
    /// ties to the authenticated user.
    pub fn verify_sub_matches(&self, expected_sub: &str) -> Result<(), JwtBindingError> {
        self.verify_signature()?;
        if self.binding.sub != expected_sub {
            return Err(JwtBindingError::SubMismatch {
                expected: expected_sub.into(),
                got:      self.binding.sub.clone(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum JwtBindingError {
    #[error("invalid Ed25519 key: {0}")]
    InvalidKey(String),
    #[error("binding signature is invalid")]
    InvalidSignature,
    #[error("sub mismatch: expected {expected:?}, got {got:?}")]
    SubMismatch { expected: String, got: String },
    #[error("JWT validation failed: {0}")]
    JwtValidationFailed(String),
}
