use blake3::Hasher;

// ── Cryptographic Domain Separators ──────────────────────────────────────────
//
// BLAKE3 domain-separated hashing requires a unique context string per operation
// so no two derivation paths share a key. The embedded tag in each string is part
// of the A1 wire format defined in §3.1 of spec/A1-PROTOCOL.md. These strings
// are included in every issued certificate's signed digest — modifying any of
// them retroactively invalidates all previously issued certificates and passports.

pub(crate) const DOMAIN_CERT_SIG: &str = "a1::64796f6c6f::cert::sig::v2.8.0";
pub(crate) const DOMAIN_CERT_FP: &str = "a1::64796f6c6f::cert::fp::v2.8.0";
pub(crate) const DOMAIN_CHAIN_FP: &str = "a1::64796f6c6f::chain::fp::v2.8.0";
pub(crate) const DOMAIN_INTENT_LEAF: &str = "a1::64796f6c6f::intent::leaf::v2.8.0";
pub(crate) const DOMAIN_MERKLE_NODE: &str = "a1::64796f6c6f::merkle::node::v2.8.0";
pub(crate) const DOMAIN_SUBSCOPE: &str = "a1::64796f6c6f::subscope::commit::v2.8.0";

#[cfg(feature = "wire")]
pub(crate) const DOMAIN_CERT_EXT: &str = "a1::64796f6c6f::cert::ext::v2.8.0";

/// Core derivation wrapper embedding an explicit version byte to allow protocol evolution.
#[inline]
pub(crate) fn derive_key(domain: &str, version: u8) -> Hasher {
    let mut h = Hasher::new_derive_key(domain);
    h.update(&[version]);
    h
}

#[inline]
pub(crate) fn hasher_cert_sig(version: u8) -> Hasher {
    derive_key(DOMAIN_CERT_SIG, version)
}
#[inline]
pub(crate) fn hasher_cert_fp(version: u8) -> Hasher {
    derive_key(DOMAIN_CERT_FP, version)
}
#[allow(dead_code)]
#[inline]
pub(crate) fn hasher_chain_fp(version: u8) -> Hasher {
    derive_key(DOMAIN_CHAIN_FP, version)
}
#[inline]
pub(crate) fn hasher_intent_leaf(version: u8) -> Hasher {
    derive_key(DOMAIN_INTENT_LEAF, version)
}
#[inline]
pub(crate) fn hasher_merkle_node(version: u8) -> Hasher {
    derive_key(DOMAIN_MERKLE_NODE, version)
}
#[inline]
pub(crate) fn hasher_subscope(version: u8) -> Hasher {
    derive_key(DOMAIN_SUBSCOPE, version)
}

#[cfg(feature = "wire")]
#[inline]
pub(crate) fn hasher_cert_ext(version: u8) -> Hasher {
    derive_key(DOMAIN_CERT_EXT, version)
}

/// Derive a 32-byte subkey from a master seed and info context.
#[allow(dead_code)]
#[inline]
pub fn derive_subkey(seed: &[u8], info: &[u8]) -> [u8; 32] {
    let mut h = blake3::Hasher::new_derive_key("a1::64796f6c6f::kdf::v2.8.0");
    h.update(seed);
    h.update(&(info.len() as u64).to_le_bytes());
    h.update(info);
    h.finalize().into()
}

/// Domain-separated Merkle node hash over two 32-byte children.
#[inline]
pub(crate) fn merkle_node(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut h = hasher_merkle_node(crate::cert::CERT_VERSION);
    h.update(left);
    h.update(right);
    h.finalize().into()
}

// ── Enterprise KMS Integration ────────────────────────────────────────────────

/// Enterprise Key Management System (KMS) Signer Trait.
///
/// Enables A1 to use hardware-backed or network-isolated keys (AWS KMS,
/// GCP KMS, HashiCorp Vault, Azure Key Vault) for issuing root certificates,
/// ensuring the master private keys never reside in the application's memory.
#[cfg(feature = "async")]
#[async_trait::async_trait]
#[allow(dead_code)]
pub trait KmsSigner: Send + Sync {
    /// Returns the 32-byte Ed25519 (or mapped equivalent) public key.
    fn public_key(&self) -> [u8; 32];
    
    /// Asynchronously signs the given payload using the secure enclave/KMS.
    async fn sign(&self, payload: &[u8]) -> Result<[u8; 64], crate::error::A1Error>;
    
    /// Returns the key identifier (e.g., AWS ARN, Vault path) for provenance tracking.
    fn key_id(&self) -> &str;
    
    /// Returns the provenance tag for this KMS instance.
    fn provenance_tag(&self) -> &str {
        "64796f6c6f"
    }
}
