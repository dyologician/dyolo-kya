//! Bind an Ed25519 verifying key to a SPIFFE X.509 SVID.
//! 
//! This allows workloads in a service mesh (Istio, Linkerd, SPIRE) to prove
//! that an Ed25519 key used for dyolo-kya delegation chains is cryptographically
//! bound to their verified SPIFFE identity (URI SAN).

use blake3::Hasher;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use dyolo_kya::Signer;

const DOMAIN_SPIFFE_BINDING: &str = "dyolo::kya::identity::spiffe_binding::v1";

/// A claim that an Ed25519 `verifying_key` is controlled by the workload
/// identified by `spiffe_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiffeBinding {
    /// The SPIFFE ID URI (e.g. "spiffe://example.org/myservice")
    pub spiffe_id: String,
    /// The Ed25519 verifying key being bound to this SPIFFE ID
    pub verifying_key: String,
    /// Unix timestamp when this binding was created
    pub bound_at: u64,
}

/// A [`SpiffeBinding`] signed by the Ed25519 key it binds.
/// 
/// The signature proves that the holder of the private key explicitly created
/// this binding — it cannot be forged by an observer who only knows the
/// public key and the SPIFFE ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedSpiffeBinding {
    pub binding: SpiffeBinding,
    pub signature: String,
}

impl SpiffeBinding {
    /// Create a new binding between a SPIFFE ID and an Ed25519 verifying key.
    pub fn new(spiffe_id: impl Into<String>, vk: &VerifyingKey) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock before epoch")
            .as_secs();

        Self {
            spiffe_id: spiffe_id.into(),
            verifying_key: hex::encode(vk.as_bytes()),
            bound_at: now,
        }
    }

    /// Sign this binding with the private key corresponding to `verifying_key`.
    pub fn sign(self, signer: &dyn Signer) -> SignedSpiffeBinding {
        let msg = self.signable_bytes();
        let sig = signer.sign_message(&msg);
        SignedSpiffeBinding {
            binding: self,
            signature: hex::encode(sig.to_bytes()),
        }
    }

    fn signable_bytes(&self) -> Vec<u8> {
        let mut h = Hasher::new_derive_key(DOMAIN_SPIFFE_BINDING);
        for field in [self.spiffe_id.as_str(), self.verifying_key.as_str()] {
            h.update(&(field.len() as u64).to_le_bytes());
            h.update(field.as_bytes());
        }
        h.update(&self.bound_at.to_be_bytes());
        h.finalize().as_bytes().to_vec()
    }
}

impl SignedSpiffeBinding {
    /// Verify the Ed25519 signature of the binding.
    pub fn verify_signature(&self) -> Result<(), SpiffeBindingError> {
        let pk_bytes = hex::decode(&self.binding.verifying_key)
            .map_err(|_| SpiffeBindingError::InvalidKey("hex decode failed".into()))?;
        let pk_arr: [u8; 32] = pk_bytes
            .try_into()
            .map_err(|_| SpiffeBindingError::InvalidKey("must be 32 bytes".into()))?;
        let vk = VerifyingKey::from_bytes(&pk_arr)
            .map_err(|e| SpiffeBindingError::InvalidKey(e.to_string()))?;

        let sig_bytes = hex::decode(&self.signature)
            .map_err(|_| SpiffeBindingError::InvalidSignature)?;
        let sig_arr: [u8; 64] = sig_bytes
            .try_into()
            .map_err(|_| SpiffeBindingError::InvalidSignature)?;
        let sig = Signature::from_bytes(&sig_arr);

        let msg = self.binding.signable_bytes();
        vk.verify(&msg, &sig)
            .map_err(|_| SpiffeBindingError::InvalidSignature)
    }

    /// Verify the Ed25519 signature and assert that the bound SPIFFE ID matches
    /// the validated SVID URI SAN.
    /// 
    /// Call this after verifying the X.509 SVID chain against your SPIRE trust bundle
    /// and extracting its URI SAN.
    pub fn verify_svid_san(&self, svid_uri_san: &str) -> Result<(), SpiffeBindingError> {
        self.verify_signature()?;
        
        if self.binding.spiffe_id != svid_uri_san {
            return Err(SpiffeBindingError::SpiffeIdMismatch {
                expected: svid_uri_san.to_string(),
                got: self.binding.spiffe_id.clone(),
            });
        }

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SpiffeBindingError {
    #[error("invalid Ed25519 key: {0}")]
    InvalidKey(String),
    #[error("binding signature is invalid")]
    InvalidSignature,
    #[error("SPIFFE ID mismatch: expected {expected:?}, got {got:?}")]
    SpiffeIdMismatch { expected: String, got: String },
}