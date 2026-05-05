use blake3::Hasher;

pub(crate) const DOMAIN_CERT_SIG:     &str = "dyolo::kya::cert::sig::v1";
pub(crate) const DOMAIN_CERT_FP:      &str = "dyolo::kya::cert::fp::v1";
pub(crate) const DOMAIN_CHAIN_FP:     &str = "dyolo::kya::chain::fp::v1";
pub(crate) const DOMAIN_INTENT_LEAF:  &str = "dyolo::kya::intent::leaf::v1";
pub(crate) const DOMAIN_MERKLE_NODE:  &str = "dyolo::kya::merkle::node::v1";
pub(crate) const DOMAIN_SUBSCOPE:     &str = "dyolo::kya::subscope::commit::v1";

#[cfg(feature = "wire")]
pub(crate) const DOMAIN_CERT_EXT:     &str = "dyolo::kya::cert::ext::v1";

/// Core derivation wrapper embedding an explicit version byte to allow protocol evolution.
#[inline]
pub(crate) fn derive_key(domain: &str, version: u8) -> Hasher {
    let mut h = Hasher::new_derive_key(domain);
    h.update(&[version]);
    h
}

#[inline] pub(crate) fn hasher_cert_sig(version: u8) -> Hasher { derive_key(DOMAIN_CERT_SIG, version) }
#[inline] pub(crate) fn hasher_cert_fp(version: u8) -> Hasher { derive_key(DOMAIN_CERT_FP, version) }
#[inline] pub(crate) fn hasher_chain_fp(version: u8) -> Hasher { derive_key(DOMAIN_CHAIN_FP, version) }
#[inline] pub(crate) fn hasher_intent_leaf(version: u8) -> Hasher { derive_key(DOMAIN_INTENT_LEAF, version) }
#[inline] pub(crate) fn hasher_merkle_node(version: u8) -> Hasher { derive_key(DOMAIN_MERKLE_NODE, version) }
#[inline] pub(crate) fn hasher_subscope(version: u8) -> Hasher { derive_key(DOMAIN_SUBSCOPE, version) }

#[cfg(feature = "wire")]
#[inline]
pub(crate) fn hasher_cert_ext(version: u8) -> Hasher { derive_key(DOMAIN_CERT_EXT, version) }

/// Derive a 32-byte subkey from a master seed and info context.
#[inline]
pub fn derive_subkey(seed: &[u8], info: &[u8]) -> [u8; 32] {
    let mut h = blake3::Hasher::new_derive_key("dyolo::kya::kdf::v1");
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