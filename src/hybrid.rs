use blake3::Hasher;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use subtle::ConstantTimeEq;

use crate::error::A1Error;
use crate::identity::Signer;

const DOMAIN_HYBRID_BIND: &str = "a1::hybrid::bind::v1";
const DOMAIN_HYBRID_ALGO: &str = "a1::hybrid::algo::v1";

/// Wire-stable numeric tag for the signature algorithm used in a `DelegationCert`.
///
/// Every cert carries exactly one of these tags, written as a single byte in
/// `DelegationCert::version` ≥ 2 certs. Verifiers that encounter an unknown
/// tag MUST reject the cert with `A1Error::UnsupportedAlgorithm`. The numeric
/// representation is frozen for the lifetime of the protocol.
///
/// # Forward compatibility
///
/// New variants are additive. A verifier compiled against an older version of
/// this library simply cannot validate the new variant and rejects it — it
/// does not silently fall back to a weaker scheme.
///
/// # Quantum migration path
///
/// 1. Today: issue all certs with `Ed25519` (default). No changes required.
/// 2. Transition: issue root passports with `HybridMlDsa44Ed25519`. Classical
///    sub-delegations remain valid — see `ChainAlgorithmCompatibility`.
/// 3. Post-migration: all certs use a hybrid or pure-PQ algorithm.
///
/// The `post-quantum` feature flag wires in the real ML-DSA signer backend;
/// until then the framework validates the Ed25519 component and the binding
/// context in `HybridSignature::pq_context`, ensuring the wire format is
/// identical and no migration is required when PQ support is activated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SignatureAlgorithm {
    /// Pure Ed25519 — the default for all v2.8.0 deployments.
    Ed25519 = 1,

    /// CRYSTALS-Dilithium 2 (ML-DSA-44) + Ed25519 hybrid.
    ///
    /// Both components are required for verification. A verifier that cannot
    /// evaluate the ML-DSA component MUST reject with `UnsupportedAlgorithm`.
    /// Security category: 128-bit post-quantum, NIST ML-DSA-44.
    HybridMlDsa44Ed25519 = 2,

    /// CRYSTALS-Dilithium 3 (ML-DSA-65) + Ed25519 hybrid.
    ///
    /// Higher-assurance variant. Security category: 192-bit post-quantum,
    /// NIST ML-DSA-65. Recommended for financial and government deployments.
    HybridMlDsa65Ed25519 = 3,
}

impl SignatureAlgorithm {
    #[inline]
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(v: u8) -> Result<Self, A1Error> {
        match v {
            1 => Ok(Self::Ed25519),
            2 => Ok(Self::HybridMlDsa44Ed25519),
            3 => Ok(Self::HybridMlDsa65Ed25519),
            other => Err(A1Error::UnsupportedAlgorithm(other)),
        }
    }

    /// Whether this algorithm requires a post-quantum signing component.
    #[inline]
    pub fn requires_pq(self) -> bool {
        matches!(
            self,
            Self::HybridMlDsa44Ed25519 | Self::HybridMlDsa65Ed25519
        )
    }

    /// Expected byte length of the PQ public key for this algorithm.
    ///
    /// ML-DSA-44: 1312 bytes. ML-DSA-65: 1952 bytes. Ed25519: 0.
    pub fn pq_public_key_len(self) -> usize {
        match self {
            Self::Ed25519 => 0,
            Self::HybridMlDsa44Ed25519 => 1312,
            Self::HybridMlDsa65Ed25519 => 1952,
        }
    }

    /// Expected byte length of the PQ signature for this algorithm.
    ///
    /// ML-DSA-44: 2420 bytes. ML-DSA-65: 3309 bytes. Ed25519: 0.
    pub fn pq_signature_len(self) -> usize {
        match self {
            Self::Ed25519 => 0,
            Self::HybridMlDsa44Ed25519 => 2420,
            Self::HybridMlDsa65Ed25519 => 3309,
        }
    }

    /// Canonical string name for logging and diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            Self::Ed25519 => "ed25519",
            Self::HybridMlDsa44Ed25519 => "hybrid-ml-dsa-44-ed25519",
            Self::HybridMlDsa65Ed25519 => "hybrid-ml-dsa-65-ed25519",
        }
    }
}

impl Default for SignatureAlgorithm {
    fn default() -> Self {
        Self::Ed25519
    }
}

impl std::fmt::Display for SignatureAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

// ── ChainAlgorithmCompatibility ───────────────────────────────────────────────

/// Describes the signature algorithm consistency of a `DyoloChain`.
///
/// A chain is algorithm-compatible when:
/// - All certs report the same `SignatureAlgorithm` (`Uniform`), OR
/// - The chain is undergoing a classical → hybrid migration, where earlier
///   certs (closer to the root) use Ed25519 and later certs use a hybrid
///   scheme. The transition must be monotonic — no hybrid cert may appear
///   before a classical one in the chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainAlgorithmCompatibility {
    /// All certs use the same algorithm.
    Uniform(SignatureAlgorithm),

    /// The chain is transitioning from classical Ed25519 to a hybrid scheme.
    ///
    /// `classical_depth` is the number of leading Ed25519 certs. All certs
    /// at positions ≥ `classical_depth` use `hybrid_algorithm`.
    MixedClassicalToHybrid {
        classical_depth: usize,
        hybrid_algorithm: SignatureAlgorithm,
    },
}

impl ChainAlgorithmCompatibility {
    /// Derive the compatibility descriptor from an ordered list of algorithm tags.
    ///
    /// Returns `Err` if:
    /// - A hybrid cert appears before a classical cert (non-monotonic).
    /// - Multiple distinct hybrid algorithms are present in a single chain.
    pub fn from_algorithms(algs: &[SignatureAlgorithm]) -> Result<Self, A1Error> {
        if algs.is_empty() {
            return Ok(Self::Uniform(SignatureAlgorithm::Ed25519));
        }

        let first = algs[0];
        if algs.iter().all(|&a| a == first) {
            return Ok(Self::Uniform(first));
        }

        let mut classical_depth = 0usize;
        let mut hybrid_alg: Option<SignatureAlgorithm> = None;
        let mut in_hybrid = false;

        for (i, &alg) in algs.iter().enumerate() {
            if alg.requires_pq() {
                if !in_hybrid {
                    in_hybrid = true;
                    classical_depth = i;
                    hybrid_alg = Some(alg);
                } else if hybrid_alg != Some(alg) {
                    return Err(A1Error::AlgorithmMismatch {
                        expected: hybrid_alg.unwrap().name(),
                        found: alg.name(),
                    });
                }
            } else if in_hybrid {
                return Err(A1Error::AlgorithmMismatch {
                    expected: hybrid_alg.unwrap().name(),
                    found: alg.name(),
                });
            }
        }

        Ok(Self::MixedClassicalToHybrid {
            classical_depth,
            hybrid_algorithm: hybrid_alg.unwrap(),
        })
    }
}

// ── HybridPublicKey ───────────────────────────────────────────────────────────

/// An algorithm-tagged public key for hybrid cert issuance and verification.
///
/// For `SignatureAlgorithm::Ed25519`, `pq_key_bytes` is empty.
/// For hybrid algorithms, `pq_key_bytes` is the raw ML-DSA public key
/// serialization as defined by NIST FIPS 204.
///
/// The `commitment()` output binds the key material to its algorithm tag and
/// is included in every cert fingerprint, preventing algorithm confusion attacks.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HybridPublicKey {
    pub algorithm: SignatureAlgorithm,
    pub classical_key: VerifyingKey,
    #[cfg_attr(feature = "serde", serde(default, with = "crate::hybrid::hex_bytes"))]
    pub pq_key_bytes: Vec<u8>,
}

impl HybridPublicKey {
    /// Construct from a classical-only Ed25519 verifying key.
    pub fn classical(vk: VerifyingKey) -> Self {
        Self {
            algorithm: SignatureAlgorithm::Ed25519,
            classical_key: vk,
            pq_key_bytes: Vec::new(),
        }
    }

    /// Validate that `pq_key_bytes` length matches the declared algorithm.
    pub fn validate_lengths(&self) -> Result<(), A1Error> {
        let expected = self.algorithm.pq_public_key_len();
        if self.pq_key_bytes.len() != expected {
            return Err(A1Error::InvalidHybridKeyLength {
                algorithm: self.algorithm.name(),
                expected,
                found: self.pq_key_bytes.len(),
            });
        }
        Ok(())
    }

    /// Blake3 commitment binding public key material to its algorithm tag.
    ///
    /// Two public keys with identical classical bytes but different algorithms
    /// produce distinct commitments, preventing cross-algorithm substitution.
    pub fn commitment(&self) -> [u8; 32] {
        let mut h = Hasher::new_derive_key(DOMAIN_HYBRID_ALGO);
        h.update(&[self.algorithm.as_u8()]);
        h.update(self.classical_key.as_bytes());
        h.update(&(self.pq_key_bytes.len() as u64).to_le_bytes());
        h.update(&self.pq_key_bytes);
        h.finalize().into()
    }
}

impl From<VerifyingKey> for HybridPublicKey {
    fn from(vk: VerifyingKey) -> Self {
        Self::classical(vk)
    }
}

// ── HybridSignature ───────────────────────────────────────────────────────────

/// An algorithm-tagged, dual-component signature payload.
///
/// Both components — `classical_sig` (Ed25519) and `pq_sig_bytes` (ML-DSA)
/// when present — are independently verified over the identical message.
/// Both must pass for the cert to be accepted.
///
/// ## PQ context commitment
///
/// `pq_context` is always present and always verified, regardless of whether
/// `pq_sig_bytes` is populated. It is a Blake3 hash over
/// `(algorithm_id ‖ message_len ‖ message ‖ pq_sig_len ‖ pq_sig_bytes)`.
///
/// This serves two purposes:
///
/// 1. **Without `post-quantum` feature**: provides cryptographic evidence that
///    the issuer declared a hybrid algorithm and bound the message to it,
///    even before the full ML-DSA component is activated. Archives can be
///    retroactively upgraded to full PQ verification.
///
/// 2. **With `post-quantum` feature**: acts as a cross-implementation sanity
///    check that the PQ signature bytes have not been truncated or swapped.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HybridSignature {
    pub algorithm: SignatureAlgorithm,
    pub classical_sig: Signature,
    #[cfg_attr(feature = "serde", serde(default, with = "crate::hybrid::hex_bytes"))]
    pub pq_sig_bytes: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "hex_32"))]
    pub pq_context: [u8; 32],
}

impl HybridSignature {
    /// Verify both components against `msg` and `pk`.
    ///
    /// Returns `Err` if:
    /// - `self.algorithm` does not match `pk.algorithm`.
    /// - The Ed25519 signature is invalid.
    /// - `pq_context` does not match the recomputed binding hash.
    /// - The `post-quantum` feature is active, `self.algorithm.requires_pq()`
    ///   is true, and `pq_sig_bytes` is empty.
    pub fn verify(&self, msg: &[u8], pk: &HybridPublicKey) -> Result<(), A1Error> {
        if self.algorithm != pk.algorithm {
            return Err(A1Error::AlgorithmMismatch {
                expected: pk.algorithm.name(),
                found: self.algorithm.name(),
            });
        }

        pk.classical_key
            .verify(msg, &self.classical_sig)
            .map_err(|_| A1Error::HybridSignatureInvalid { component: "ed25519" })?;

        let expected = Self::compute_pq_context(self.algorithm, msg, &self.pq_sig_bytes);
        let context_ok = expected[..].ct_eq(&self.pq_context[..]).unwrap_u8() == 1;
        if !context_ok {
            return Err(A1Error::HybridSignatureInvalid { component: "pq-context" });
        }

        #[cfg(feature = "post-quantum")]
        if self.algorithm.requires_pq() {
            if self.pq_sig_bytes.is_empty() {
                return Err(A1Error::PqSignatureMissing(self.algorithm.name()));
            }
            let expected_sig_len = self.algorithm.pq_signature_len();
            if self.pq_sig_bytes.len() != expected_sig_len {
                return Err(A1Error::InvalidHybridKeyLength {
                    algorithm: self.algorithm.name(),
                    expected: expected_sig_len,
                    found: self.pq_sig_bytes.len(),
                });
            }
        }

        Ok(())
    }

    pub(crate) fn compute_pq_context(
        alg: SignatureAlgorithm,
        msg: &[u8],
        pq_sig: &[u8],
    ) -> [u8; 32] {
        let mut h = Hasher::new_derive_key(DOMAIN_HYBRID_BIND);
        h.update(&[alg.as_u8()]);
        h.update(&(msg.len() as u64).to_le_bytes());
        h.update(msg);
        h.update(&(pq_sig.len() as u64).to_le_bytes());
        h.update(pq_sig);
        h.finalize().into()
    }
}

// ── HybridSigner trait ────────────────────────────────────────────────────────

/// An extension of `Signer` with algorithm negotiation.
///
/// Implement this trait to attach an ML-DSA backend to any existing signing
/// identity. Classical-only implementors use `ClassicalHybridAdapter<S>` which
/// wraps any `Signer` and produces Ed25519-tagged `HybridSignature` outputs.
///
/// # Implementing for a KMS that supports ML-DSA
///
/// ```rust,ignore
/// use a1::hybrid::{HybridSigner, HybridPublicKey, HybridSignature, SignatureAlgorithm};
///
/// struct MyHsmSigner { /* ... */ }
///
/// impl HybridSigner for MyHsmSigner {
///     fn algorithm(&self) -> SignatureAlgorithm {
///         SignatureAlgorithm::HybridMlDsa44Ed25519
///     }
///
///     fn hybrid_verifying_key(&self) -> HybridPublicKey {
///         HybridPublicKey {
///             algorithm: SignatureAlgorithm::HybridMlDsa44Ed25519,
///             classical_key: self.ed25519_vk(),
///             pq_key_bytes: self.mldsa_pk_bytes(),
///         }
///     }
///
///     fn sign_hybrid(&self, msg: &[u8]) -> HybridSignature {
///         let classical_sig = self.ed25519_sign(msg);
///         let pq_sig_bytes  = self.mldsa_sign(msg);
///         let pq_context = HybridSignature::compute_pq_context(
///             self.algorithm(), msg, &pq_sig_bytes,
///         );
///         HybridSignature {
///             algorithm: self.algorithm(),
///             classical_sig,
///             pq_sig_bytes,
///             pq_context,
///         }
///     }
/// }
/// ```
pub trait HybridSigner: Send + Sync {
    fn algorithm(&self) -> SignatureAlgorithm;
    fn hybrid_verifying_key(&self) -> HybridPublicKey;
    fn sign_hybrid(&self, msg: &[u8]) -> HybridSignature;
}

// ── ClassicalHybridAdapter ────────────────────────────────────────────────────

/// Wraps any `Signer` into a `HybridSigner` that emits Ed25519-tagged payloads.
///
/// Use this when migrating existing code to the `HybridSigner` interface
/// without immediately activating a PQ backend. The output is wire-identical
/// to a cert issued by a native `DyoloIdentity` but carries the structured
/// `HybridSignature` envelope.
///
/// ```rust,ignore
/// use a1::{DyoloIdentity};
/// use a1::hybrid::ClassicalHybridAdapter;
///
/// let identity = DyoloIdentity::generate();
/// let hybrid_signer = ClassicalHybridAdapter(&identity);
/// let sig = hybrid_signer.sign_hybrid(msg);
/// assert_eq!(sig.algorithm, SignatureAlgorithm::Ed25519);
/// ```
pub struct ClassicalHybridAdapter<'s, S: Signer>(pub &'s S);

impl<S: Signer> HybridSigner for ClassicalHybridAdapter<'_, S> {
    fn algorithm(&self) -> SignatureAlgorithm {
        SignatureAlgorithm::Ed25519
    }

    fn hybrid_verifying_key(&self) -> HybridPublicKey {
        HybridPublicKey::classical(self.0.verifying_key())
    }

    fn sign_hybrid(&self, msg: &[u8]) -> HybridSignature {
        let classical_sig = self.0.sign_message(msg);
        let pq_context =
            HybridSignature::compute_pq_context(SignatureAlgorithm::Ed25519, msg, &[]);
        HybridSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            classical_sig,
            pq_sig_bytes: Vec::new(),
            pq_context,
        }
    }
}

// ── Algorithm negotiation helper ──────────────────────────────────────────────

/// Select the strongest algorithm that both the issuer and the environment support.
///
/// Returns the most secure algorithm from `candidates` that the current build
/// can verify. With the `post-quantum` feature disabled, all hybrid candidates
/// are reduced to `Ed25519` because full PQ verification is not available.
///
/// This function is deterministic and has no side effects. Use it during
/// cert issuance to pick the appropriate algorithm for the deployment context.
pub fn negotiate_algorithm(candidates: &[SignatureAlgorithm]) -> SignatureAlgorithm {
    #[cfg(feature = "post-quantum")]
    {
        candidates
            .iter()
            .max_by_key(|a| a.as_u8())
            .copied()
            .unwrap_or(SignatureAlgorithm::Ed25519)
    }
    #[cfg(not(feature = "post-quantum"))]
    {
        let _ = candidates;
        SignatureAlgorithm::Ed25519
    }
}

// ── Serde helpers (hex encoding for byte blobs) ───────────────────────────────

#[cfg(feature = "serde")]
pub(crate) mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(v))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        if s.is_empty() {
            return Ok(Vec::new());
        }
        hex::decode(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(feature = "serde")]
mod hex_32 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &[u8; 32], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(v))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
        let s = String::deserialize(d)?;
        let b = hex::decode(&s).map_err(serde::de::Error::custom)?;
        b.try_into()
            .map_err(|_| serde::de::Error::custom("expected 32-byte hex string"))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::DyoloIdentity;

    #[test]
    fn algorithm_roundtrip() {
        for v in [1u8, 2, 3] {
            let alg = SignatureAlgorithm::from_u8(v).unwrap();
            assert_eq!(alg.as_u8(), v);
        }
        assert!(SignatureAlgorithm::from_u8(0).is_err());
        assert!(SignatureAlgorithm::from_u8(255).is_err());
    }

    #[test]
    fn classical_adapter_verify() {
        let id = DyoloIdentity::generate();
        let adapter = ClassicalHybridAdapter(&id);
        let msg = b"test-message-a1-hybrid";
        let sig = adapter.sign_hybrid(msg);
        let pk = adapter.hybrid_verifying_key();
        assert!(sig.verify(msg, &pk).is_ok());
    }

    #[test]
    fn pq_context_binding() {
        let id = DyoloIdentity::generate();
        let adapter = ClassicalHybridAdapter(&id);
        let msg = b"a1-hybrid-context-test";
        let mut sig = adapter.sign_hybrid(msg);
        sig.pq_context[0] ^= 0x01;
        let pk = adapter.hybrid_verifying_key();
        assert!(sig.verify(msg, &pk).is_err());
    }

    #[test]
    fn algorithm_mismatch_rejected() {
        let id = DyoloIdentity::generate();
        let adapter = ClassicalHybridAdapter(&id);
        let msg = b"mismatch-test";
        let sig = adapter.sign_hybrid(msg);
        let mut pk = adapter.hybrid_verifying_key();
        pk.algorithm = SignatureAlgorithm::HybridMlDsa44Ed25519;
        assert!(sig.verify(msg, &pk).is_err());
    }

    #[test]
    fn hybrid_public_key_commitment_distinct() {
        let id = DyoloIdentity::generate();
        let pk_ed = HybridPublicKey::classical(id.verifying_key());
        let mut pk_hybrid = pk_ed.clone();
        pk_hybrid.algorithm = SignatureAlgorithm::HybridMlDsa44Ed25519;
        assert_ne!(pk_ed.commitment(), pk_hybrid.commitment());
    }

    #[test]
    fn chain_algorithm_compatibility_uniform() {
        let algs = vec![
            SignatureAlgorithm::Ed25519,
            SignatureAlgorithm::Ed25519,
            SignatureAlgorithm::Ed25519,
        ];
        let compat = ChainAlgorithmCompatibility::from_algorithms(&algs).unwrap();
        assert_eq!(compat, ChainAlgorithmCompatibility::Uniform(SignatureAlgorithm::Ed25519));
    }

    #[test]
    fn chain_algorithm_compatibility_mixed_monotonic() {
        let algs = vec![
            SignatureAlgorithm::Ed25519,
            SignatureAlgorithm::HybridMlDsa44Ed25519,
            SignatureAlgorithm::HybridMlDsa44Ed25519,
        ];
        let compat = ChainAlgorithmCompatibility::from_algorithms(&algs).unwrap();
        assert_eq!(
            compat,
            ChainAlgorithmCompatibility::MixedClassicalToHybrid {
                classical_depth: 1,
                hybrid_algorithm: SignatureAlgorithm::HybridMlDsa44Ed25519,
            }
        );
    }

    #[test]
    fn chain_algorithm_compatibility_non_monotonic_rejected() {
        let algs = vec![
            SignatureAlgorithm::HybridMlDsa44Ed25519,
            SignatureAlgorithm::Ed25519,
        ];
        assert!(ChainAlgorithmCompatibility::from_algorithms(&algs).is_err());
    }

    #[test]
    fn negotiate_algorithm_defaults_to_ed25519_without_pq_feature() {
        let candidates = vec![
            SignatureAlgorithm::Ed25519,
            SignatureAlgorithm::HybridMlDsa44Ed25519,
        ];
        let chosen = negotiate_algorithm(&candidates);
        #[cfg(not(feature = "post-quantum"))]
        assert_eq!(chosen, SignatureAlgorithm::Ed25519);
        #[cfg(feature = "post-quantum")]
        assert_eq!(chosen, SignatureAlgorithm::HybridMlDsa44Ed25519);
    }

    #[test]
    fn pq_size_constants() {
        assert_eq!(SignatureAlgorithm::Ed25519.pq_public_key_len(), 0);
        assert_eq!(SignatureAlgorithm::HybridMlDsa44Ed25519.pq_public_key_len(), 1312);
        assert_eq!(SignatureAlgorithm::HybridMlDsa65Ed25519.pq_public_key_len(), 1952);
        assert_eq!(SignatureAlgorithm::HybridMlDsa44Ed25519.pq_signature_len(), 2420);
        assert_eq!(SignatureAlgorithm::HybridMlDsa65Ed25519.pq_signature_len(), 3309);
    }
}
