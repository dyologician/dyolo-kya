//! Portable wire formats for cross-service authorization transport.
//!
//! Enable with `features = ["wire"]`.
//!
//! # Why this module exists
//!
//! In a microservice architecture the service that *builds* a delegation chain
//! and the service that *executes* an action under it are separate processes.
//! `DyoloChain` and `AuthorizedAction` cannot cross that boundary directly:
//! `DyoloChain` contains deserialized `ed25519-dalek` values, and
//! `AuthorizedAction` is deliberately non-serializable (the sealed `_sealed` field
//! enforces that authorization stays in-process).
//!
//! This module provides two cross-boundary types:
//!
//! - [`SignedChain`] — the full chain as a JSON/CBOR document. The *authorizing*
//!   service serializes it; the *executing* service deserializes it and calls
//!   [`DyoloChain::authorize`] again to re-verify.
//! - [`VerifiedToken`] — a receipt authenticated with a shared HMAC key.
//!   The *authorizing* service verifies the chain and signs the receipt; the
//!   *executing* service checks the HMAC without re-running the chain. Suitable
//!   for high-throughput paths where re-verification is too slow.
//!
//! # Quick start
//!
//! ```rust,ignore
//! use dyolo_kya::wire::{SignedChain, VerifiedToken};
//!
//! // ── Authorizing service ───────────────────────────────────────────────────
//! let signed = SignedChain::from_chain(&chain);
//! let chain_json = serde_json::to_string(&signed)?;
//!
//! // Full re-verification on the executing service:
//! let chain = SignedChain::from_json(&chain_json)?.into_chain()?;
//! let action = chain.authorize(&agent_pk, &intent, &proof, &clock, &rev, &nonce)?;
//!
//! // ── For trust-delegated execution (shared MAC key out-of-band) ────────────
//! let mac_key: [u8; 32] = /* from your secrets manager */;
//! let token = VerifiedToken::sign(&action.receipt, &mac_key);
//! let token_json = serde_json::to_string(&token)?;
//!
//! // Executing service just validates the MAC:
//! let token: VerifiedToken = serde_json::from_str(&token_json)?;
//! let receipt = token.verify(&mac_key)?;
//! println!("Authorized depth={}", receipt.chain_depth);
//! ```

use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};

use crate::cert::DelegationCert;
use crate::chain::{DyoloChain, VerificationReceipt};
use crate::error::KyaError;
use crate::intent::IntentHash;

// ── SignedChain ───────────────────────────────────────────────────────────────

/// A portable, serializable representation of a [`DyoloChain`].
///
/// All `ed25519-dalek` types are encoded as hex strings so the wire format is
/// language-agnostic. Any service that can deserialize JSON and call the
/// dyolo-kya library (or a compatible implementation) can verify the chain.
///
/// The format is intentionally minimal: `principal_pk`, `principal_scope`, and
/// `certs`. The chain fingerprint is recomputed on deserialization.
///
/// # Interoperability
///
/// Non-Rust services can verify a `SignedChain` by:
/// 1. Deserializing the JSON into native types.
/// 2. Re-running the Ed25519 batch verification over the cert chain.
/// 3. Re-checking scope narrowing and temporal constraints.
///
/// A formal JSON Schema is published at
/// <https://docs.rs/dyolo-kya/latest/dyolo_kya/wire/index.html>.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedChain {
    /// Wire format version. Currently `1`.
    pub version: u8,
    /// Hex-encoded 32-byte Ed25519 verifying key of the root authority.
    pub principal_pk: String,
    /// Hex-encoded 32-byte Merkle root of the principal's intent set.
    pub principal_scope: String,
    /// Ordered delegation certificates from principal to terminal agent.
    pub certs: Vec<DelegationCert>,
}

impl SignedChain {
    /// Serialize a [`DyoloChain`] into a portable wire document.
    pub fn from_chain(chain: &DyoloChain) -> Self {
        Self {
            version: 1,
            principal_pk:    hex::encode(chain.principal_pk.as_bytes()),
            principal_scope: hex::encode(chain.principal_scope),
            certs:           chain.certs().to_vec(),
        }
    }

    /// Deserialize a JSON wire document.
    pub fn from_json(json: &str) -> Result<Self, KyaError> {
        serde_json::from_str(json)
            .map_err(|e| KyaError::WireFormatError(e.to_string()))
    }

    #[cfg(feature = "cbor")]
    #[cfg_attr(docsrs, doc(cfg(feature = "cbor")))]
    pub fn from_cbor(cbor: &[u8]) -> Result<Self, KyaError> {
        ciborium::from_reader(cbor)
            .map_err(|e| KyaError::WireFormatError(e.to_string()))
    }

    #[cfg(feature = "cbor")]
    #[cfg_attr(docsrs, doc(cfg(feature = "cbor")))]
    pub fn to_cbor(&self) -> Result<Vec<u8>, KyaError> {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf)
            .map_err(|e| KyaError::WireFormatError(e.to_string()))?;
        Ok(buf)
    }

    /// Convert this wire document back into a live [`DyoloChain`].
    ///
    /// The receiver must specify the clock drift tolerance for the reconstructed chain.
    /// This prevents malicious intermediaries from widening the temporal window.
    pub fn into_chain_with_drift(self, drift_tolerance_secs: u64) -> Result<DyoloChain, KyaError> {
        if self.version != 1 {
            return Err(KyaError::UnsupportedVersion { expected: 1, got: self.version });
        }

        let pk_bytes: [u8; 32] = hex::decode(&self.principal_pk)
            .map_err(|e| KyaError::WireFormatError(format!("principal_pk: {e}")))?
            .try_into()
            .map_err(|_| KyaError::WireFormatError("principal_pk must be 32 bytes".into()))?;

        let scope_bytes: [u8; 32] = hex::decode(&self.principal_scope)
            .map_err(|e| KyaError::WireFormatError(format!("principal_scope: {e}")))?
            .try_into()
            .map_err(|_| KyaError::WireFormatError("principal_scope must be 32 bytes".into()))?;

        let pk = VerifyingKey::from_bytes(&pk_bytes)
            .map_err(|e| KyaError::WireFormatError(format!("invalid principal_pk: {e}")))?;

        let mut chain = DyoloChain::new(pk, scope_bytes)
            .with_drift_tolerance(drift_tolerance_secs);

        for cert in self.certs {
            chain.push(cert);
        }

        Ok(chain)
    }

    #[deprecated(since = "2.0.0", note = "Use `into_chain_with_drift` instead.")]
    pub fn into_chain(self) -> Result<DyoloChain, KyaError> {
        self.into_chain_with_drift(15)
    }

    /// Serialize to a compact JSON string.
    pub fn to_json(&self) -> Result<String, KyaError> {
        serde_json::to_string(self)
            .map_err(|e| KyaError::WireFormatError(e.to_string()))
    }

    /// Serialize to a pretty-printed JSON string (useful for audit logs).
    pub fn to_json_pretty(&self) -> Result<String, KyaError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| KyaError::WireFormatError(e.to_string()))
    }
}

// ── VerifiedToken ─────────────────────────────────────────────────────────────

/// A [`VerificationReceipt`] authenticated with a shared HMAC key.
///
/// Allows an executing service to accept an authorization decision from a
/// trusted verifying service without re-running the full Ed25519 chain
/// verification. The HMAC is computed with Blake3 in keyed mode over the
/// canonical binary encoding of the receipt fields.
///
/// # Security requirements
///
/// - The `mac_key` must be a 32-byte secret shared exclusively between the
///   verifying service and the executing service.
/// - Rotate the key regularly (recommended: every 24 hours).
/// - Transport tokens over a secure channel (TLS 1.3 minimum).
/// - Set a short expiry on tokens — the receipt's `verified_at_unix` field
///   lets executors enforce their own maximum age.
///
/// # Example
///
/// ```rust,ignore
/// use dyolo_kya::wire::VerifiedToken;
///
/// // Verifying service:
/// let mac_key: [u8; 32] = /* from secrets manager */;
/// let token = VerifiedToken::sign(&action.receipt, &mac_key);
/// let json  = serde_json::to_string(&token)?;
/// // → send json over TLS to executing service
///
/// // Executing service:
/// let token: VerifiedToken = serde_json::from_str(&json)?;
/// let receipt = token.verify(&mac_key)?;  // fails if tampered or wrong key
/// // Use receipt.intent, receipt.chain_fingerprint for audit log
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedToken {
    /// The receipt to transport.
    pub receipt: VerificationReceipt,
    /// Hex-encoded 32-byte Blake3 keyed MAC over the canonical receipt bytes.
    pub mac: String,
}

impl VerifiedToken {
    /// Produce a `VerifiedToken` by signing `receipt` with the given 32-byte key.
    pub fn sign(receipt: &VerificationReceipt, mac_key: &[u8; 32]) -> Self {
        let mac = Self::compute_mac(receipt, mac_key);
        Self {
            receipt: receipt.clone(),
            mac: hex::encode(mac),
        }
    }

    /// Verify the MAC and return a reference to the receipt on success.
    ///
    /// Returns [`KyaError::InvalidSubScopeProof`] if the MAC is invalid
    /// (tampered token, wrong key, or truncated hex). The specific error
    /// variant is intentionally generic to avoid oracle attacks.
    pub fn verify(&self, mac_key: &[u8; 32]) -> Result<&VerificationReceipt, KyaError> {
        let mac_bytes: [u8; 32] = hex::decode(&self.mac)
            .map_err(|_| KyaError::WireFormatError("invalid MAC hex".into()))?
            .try_into()
            .map_err(|_| KyaError::WireFormatError("MAC must be 32 bytes".into()))?;

        let expected = Self::compute_mac(&self.receipt, mac_key);

        // Constant-time comparison prevents timing side-channels.
        use subtle::ConstantTimeEq;
        if mac_bytes.ct_eq(&expected).unwrap_u8() == 0 {
            return Err(KyaError::MacVerificationFailed);
        }

        Ok(&self.receipt)
    }

    fn compute_mac(receipt: &VerificationReceipt, key: &[u8; 32]) -> [u8; 32] {
        let mut h = blake3::Hasher::new_keyed(key);
        h.update(&receipt.canonical_bytes());
        h.finalize().into()
    }
}

// ── JSON Schema constant ──────────────────────────────────────────────────────

/// JSON Schema for [`SignedChain`] (v1).
///
/// Generated via `schemars`.
/// Embed this in your API documentation or OpenAPI spec to give non-Rust
/// clients a machine-readable contract for the wire format.
#[cfg(feature = "schema")]
#[cfg_attr(docsrs, doc(cfg(feature = "schema")))]
pub const SIGNED_CHAIN_SCHEMA_V1: &str = include_str!("../wire/schema.json");

#[cfg(not(feature = "schema"))]
pub const SIGNED_CHAIN_SCHEMA_V1: &str = "Enable the `schema` feature to include the JSON schema.";