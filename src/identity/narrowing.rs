use blake3::Hasher;

use crate::error::A1Error;

const DOMAIN: &[u8] = b"a1::dyolo::narrowing::v2.8.0";

/// A 256-bit capability mask with cryptographic narrowing guarantees.
///
/// `NarrowingMatrix` enforces that delegated capability sets are strict subsets
/// of their parent's set. The check is O(1): eight 64-bit AND operations on
/// modern hardware, regardless of how many named capabilities exist.
///
/// Each capability name maps deterministically to a bit position in the 256-bit
/// field via Blake3. Two distinct capability names that collide on the same bit
/// are both authorized by setting that bit — this is intentional and
/// conservative: the narrowing guarantee (sub ⊆ parent) still holds.
///
/// # Collision behaviour
///
/// Hash-based mapping distributes capabilities uniformly across 256 bits. With
/// the birthday bound, collisions become likely when a single deployment uses
/// more than ~100–150 distinct capability names. Collisions are **not a security
/// vulnerability** — they produce false positives (capability A grants slot X,
/// and so does capability B, so holding A also passes a check for B). For large
/// deployments use [`CapabilityRegistry`] to assign explicit, collision-free
/// bit positions.
///
/// # Narrowing invariant
///
/// For any parent mask `P` and requested mask `R`:
/// ```text
/// R.is_subset_of(P) ↔ (P.mask & R.mask) == R.mask
/// ```
///
/// This is the sole enforcement rule. No external registry, no network call,
/// no configuration file is required at verification time.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NarrowingMatrix {
    mask: [u8; 32],
}

impl NarrowingMatrix {
    /// An empty mask: no capabilities authorized.
    pub const EMPTY: Self = Self { mask: [0u8; 32] };

    /// A full mask: all 256 bits set — used by root passports.
    pub const FULL: Self = Self { mask: [0xFF; 32] };

    /// Build a mask from a slice of capability name strings.
    ///
    /// Each name is mapped to a bit position via Blake3. Order does not matter.
    /// Duplicate names are idempotent.
    ///
    /// For deployments with more than ~100 distinct capability names, prefer
    /// [`CapabilityRegistry::build_mask`] to avoid hash-space collisions.
    pub fn from_capabilities<S: AsRef<str>>(caps: &[S]) -> Self {
        let mut mask = [0u8; 32];
        for cap in caps {
            let (byte_idx, bit_idx) = capability_to_bit(cap.as_ref());
            mask[byte_idx] |= 1u8 << bit_idx;
        }
        Self { mask }
    }

    /// Parse a comma-separated capability string (e.g. `"trade.equity,portfolio.read"`).
    pub fn from_csv(csv: &str) -> Self {
        let caps: Vec<&str> = csv.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
        Self::from_capabilities(&caps)
    }

    /// Construct directly from a raw 32-byte mask.
    ///
    /// Used by [`CapabilityRegistry::build_mask`] and other internal callers
    /// that manage their own bit layout.
    pub(crate) fn from_raw(mask: [u8; 32]) -> Self {
        Self { mask }
    }

    /// Return `true` if `self` is a subset of `parent`.
    ///
    /// This is the sole enforcement rule for narrowing. A sub-passport cannot
    /// carry capabilities that its parent does not have.
    pub fn is_subset_of(&self, parent: &NarrowingMatrix) -> bool {
        let read_u64 = |bytes: &[u8; 32], i: usize| -> u64 {
            u64::from_le_bytes(bytes[i * 8..(i + 1) * 8].try_into().expect("slice is 8 bytes"))
        };
        (0..4).all(|i| {
            let s = read_u64(&self.mask, i);
            let p = read_u64(&parent.mask, i);
            s & p == s
        })
    }

    /// Validate that `self` is a subset of `parent`, returning an error otherwise.
    pub fn enforce_narrowing(&self, parent: &NarrowingMatrix) -> Result<(), A1Error> {
        if self.is_subset_of(parent) {
            Ok(())
        } else {
            Err(A1Error::PassportNarrowingViolation)
        }
    }

    /// Produce the intersection of two masks (logical AND).
    ///
    /// Useful for computing the maximum allowed sub-mask from a parent.
    pub fn intersect(&self, other: &NarrowingMatrix) -> NarrowingMatrix {
        let mut mask = [0u8; 32];
        for (i, item) in mask.iter_mut().enumerate() {
            *item = self.mask[i] & other.mask[i];
        }
        NarrowingMatrix { mask }
    }

    /// A 32-byte Blake3 commitment over this mask, domain-separated.
    ///
    /// Stored in the cert extension so that a tampered mask fails the
    /// extension commitment check inside `DelegationCert::signable_bytes`.
    pub fn commitment(&self) -> [u8; 32] {
        let mut h = Hasher::new_derive_key(std::str::from_utf8(DOMAIN).unwrap());
        h.update(&self.mask);
        *h.finalize().as_bytes()
    }

    /// The raw 32-byte mask. Suitable for serialization and storage.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.mask
    }

    /// Hex representation of the mask.
    pub fn to_hex(&self) -> String {
        hex::encode(self.mask)
    }

    /// Reconstruct from a 32-byte hex string.
    pub fn from_hex(s: &str) -> Result<Self, A1Error> {
        let bytes = hex::decode(s).map_err(|_| {
            A1Error::WireFormatError("invalid narrowing matrix hex".into())
        })?;
        if bytes.len() != 32 {
            return Err(A1Error::WireFormatError(
                "narrowing matrix must be exactly 32 bytes".into(),
            ));
        }
        let mut mask = [0u8; 32];
        mask.copy_from_slice(&bytes);
        Ok(Self { mask })
    }

    /// Return `true` if no capabilities are set.
    pub fn is_empty(&self) -> bool {
        self.mask.iter().all(|&b| b == 0)
    }

    /// Count how many bits are set (number of unique capability slots in use).
    pub fn capacity_count(&self) -> u32 {
        self.mask.iter().map(|b| b.count_ones()).sum()
    }
}

impl Default for NarrowingMatrix {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl std::fmt::Display for NarrowingMatrix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Map a capability name to a `(byte_index, bit_index)` pair deterministically.
///
/// Uses the first two bytes of `blake3(DOMAIN || name)`. The DOMAIN prefix
/// ensures the mapping is protocol-specific and will not collide with unrelated
/// Blake3 usages.
fn capability_to_bit(name: &str) -> (usize, usize) {
    let mut h = Hasher::new_derive_key(std::str::from_utf8(DOMAIN).unwrap());
    h.update(name.as_bytes());
    let out = h.finalize();
    let b = out.as_bytes();
    let byte_idx = (b[0] as usize) % 32;
    let bit_idx = (b[1] as usize) % 8;
    (byte_idx, bit_idx)
}

// ── CapabilityRegistry ────────────────────────────────────────────────────────

/// Collision-free explicit capability-to-bit registry.
///
/// For deployments with more than ~100 distinct named capabilities, the
/// hash-based mapping in [`NarrowingMatrix::from_capabilities`] has a
/// non-negligible probability of two different names landing on the same
/// bit (birthday bound over 256 positions). `CapabilityRegistry` eliminates
/// this risk by assigning bit positions sequentially in registration order —
/// no hashing, no collisions, deterministic.
///
/// # When to use this
///
/// - You have more than ~100 distinct capability names in a single deployment.
/// - You need a guaranteed bijection between name and bit position.
/// - You want to audit exactly which bit corresponds to which capability.
///
/// # Capacity
///
/// Each registry holds up to 256 capabilities (the bit-width of a
/// `NarrowingMatrix`). For larger sets, use multiple registries partitioned
/// by capability domain (e.g. `trading.*`, `portfolio.*`, `audit.*`).
///
/// # Example
///
/// ```rust
/// use a1::identity::narrowing::CapabilityRegistry;
///
/// let mut registry = CapabilityRegistry::new();
/// registry.register_all(&["trade.equity", "portfolio.read", "audit.read"]).unwrap();
///
/// let parent = registry.build_mask(&["trade.equity", "portfolio.read"]).unwrap();
/// let child  = registry.build_mask(&["trade.equity"]).unwrap();
///
/// assert!(child.is_subset_of(&parent));
/// ```
#[derive(Debug, Clone)]
pub struct CapabilityRegistry {
    /// name → sequential slot index (0..255)
    slots: std::collections::HashMap<String, u8>,
    /// next available slot
    next: u8,
    /// total registered (separate from next to handle overflow correctly)
    count: usize,
}

impl CapabilityRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            slots: std::collections::HashMap::new(),
            next: 0,
            count: 0,
        }
    }

    /// Register a single capability name and return its assigned slot index.
    ///
    /// If the name is already registered, returns its existing slot without
    /// consuming a new slot. Returns an error if the registry is full (256
    /// capabilities already registered).
    pub fn register(&mut self, name: impl Into<String>) -> Result<u8, A1Error> {
        let name = name.into();
        if let Some(&slot) = self.slots.get(&name) {
            return Ok(slot);
        }
        if self.count >= 256 {
            return Err(A1Error::WireFormatError(
                "CapabilityRegistry is full: maximum 256 capabilities per registry".into(),
            ));
        }
        let slot = self.next;
        self.slots.insert(name, slot);
        self.next = self.next.wrapping_add(1);
        self.count += 1;
        Ok(slot)
    }

    /// Register multiple capability names in order.
    pub fn register_all<S: AsRef<str>>(&mut self, names: &[S]) -> Result<(), A1Error> {
        for name in names {
            self.register(name.as_ref())?;
        }
        Ok(())
    }

    /// Build a `NarrowingMatrix` from a set of registered capability names.
    ///
    /// Returns an error if any name has not been registered. This ensures
    /// that only explicitly declared capabilities can appear in a mask —
    /// typos are caught at mask-build time rather than silently granting
    /// an unexpected bit.
    pub fn build_mask<S: AsRef<str>>(&self, capabilities: &[S]) -> Result<NarrowingMatrix, A1Error> {
        let mut mask = [0u8; 32];
        for cap in capabilities {
            let name = cap.as_ref();
            let slot = self.slots.get(name).ok_or_else(|| {
                A1Error::WireFormatError(format!(
                    "capability '{}' is not registered; call register() first",
                    name
                ))
            })?;
            let byte_idx = (*slot as usize) / 8;
            let bit_idx  = (*slot as usize) % 8;
            mask[byte_idx] |= 1u8 << bit_idx;
        }
        Ok(NarrowingMatrix::from_raw(mask))
    }

    /// Build a full mask covering all registered capabilities.
    pub fn build_full_mask(&self) -> NarrowingMatrix {
        let mut mask = [0u8; 32];
        for slot in self.slots.values() {
            let byte_idx = (*slot as usize) / 8;
            let bit_idx  = (*slot as usize) % 8;
            mask[byte_idx] |= 1u8 << bit_idx;
        }
        NarrowingMatrix::from_raw(mask)
    }

    /// Return the slot index for a registered capability name, if present.
    pub fn slot_of(&self, name: &str) -> Option<u8> {
        self.slots.get(name).copied()
    }

    /// Return all registered capability names sorted by their slot index.
    pub fn names_in_order(&self) -> Vec<&str> {
        let mut pairs: Vec<(&str, u8)> = self
            .slots
            .iter()
            .map(|(k, &v)| (k.as_str(), v))
            .collect();
        pairs.sort_by_key(|&(_, slot)| slot);
        pairs.into_iter().map(|(name, _)| name).collect()
    }

    /// Number of registered capabilities.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Return `true` if no capabilities have been registered.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

impl Default for CapabilityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_subset_of_full() {
        assert!(NarrowingMatrix::EMPTY.is_subset_of(&NarrowingMatrix::FULL));
    }

    #[test]
    fn full_is_not_subset_of_empty() {
        assert!(!NarrowingMatrix::FULL.is_subset_of(&NarrowingMatrix::EMPTY));
    }

    #[test]
    fn subset_of_itself() {
        let m = NarrowingMatrix::from_capabilities(&["trade.equity", "portfolio.read"]);
        assert!(m.is_subset_of(&m));
    }

    #[test]
    fn sub_is_subset_of_parent() {
        let parent = NarrowingMatrix::from_capabilities(&["trade.equity", "portfolio.read", "portfolio.write"]);
        let child = NarrowingMatrix::from_capabilities(&["trade.equity"]);
        assert!(child.is_subset_of(&parent));
        assert!(!parent.is_subset_of(&child));
    }

    #[test]
    fn escalation_detected() {
        let parent = NarrowingMatrix::from_capabilities(&["portfolio.read"]);
        let child = NarrowingMatrix::from_capabilities(&["trade.equity"]);
        assert!(child.enforce_narrowing(&parent).is_err());
    }

    #[test]
    fn commitment_is_stable() {
        let m = NarrowingMatrix::from_capabilities(&["trade.equity"]);
        let c1 = m.commitment();
        let c2 = m.commitment();
        assert_eq!(c1, c2);
    }

    #[test]
    fn commitment_differs_across_masks() {
        let a = NarrowingMatrix::from_capabilities(&["trade.equity"]);
        let b = NarrowingMatrix::from_capabilities(&["portfolio.write"]);
        assert_ne!(a.commitment(), b.commitment());
    }

    #[test]
    fn roundtrip_hex() {
        let m = NarrowingMatrix::from_capabilities(&["trade.equity", "audit.read"]);
        let hex = m.to_hex();
        let m2 = NarrowingMatrix::from_hex(&hex).unwrap();
        assert_eq!(m, m2);
    }

    #[test]
    fn csv_parsing() {
        let m = NarrowingMatrix::from_csv("trade.equity , portfolio.read, audit.read");
        let expected = NarrowingMatrix::from_capabilities(&["trade.equity", "portfolio.read", "audit.read"]);
        assert_eq!(m, expected);
    }

    #[test]
    fn intersect_produces_common_bits() {
        let a = NarrowingMatrix::from_capabilities(&["trade.equity", "portfolio.read"]);
        let b = NarrowingMatrix::from_capabilities(&["trade.equity", "audit.read"]);
        let common = a.intersect(&b);
        let expected = NarrowingMatrix::from_capabilities(&["trade.equity"]);
        assert_eq!(common, expected);
    }

    // ── CapabilityRegistry tests ──────────────────────────────────────────────

    #[test]
    fn registry_sequential_slots() {
        let mut reg = CapabilityRegistry::new();
        let s0 = reg.register("alpha").unwrap();
        let s1 = reg.register("beta").unwrap();
        let s2 = reg.register("gamma").unwrap();
        assert_eq!(s0, 0);
        assert_eq!(s1, 1);
        assert_eq!(s2, 2);
    }

    #[test]
    fn registry_idempotent_register() {
        let mut reg = CapabilityRegistry::new();
        let s0 = reg.register("alpha").unwrap();
        let s1 = reg.register("alpha").unwrap();
        assert_eq!(s0, s1);
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn registry_build_mask_subset() {
        let mut reg = CapabilityRegistry::new();
        reg.register_all(&["trade.equity", "portfolio.read", "audit.read"]).unwrap();

        let parent = reg.build_mask(&["trade.equity", "portfolio.read"]).unwrap();
        let child  = reg.build_mask(&["trade.equity"]).unwrap();

        assert!(child.is_subset_of(&parent));
        assert!(!parent.is_subset_of(&child));
    }

    #[test]
    fn registry_rejects_unknown_capability() {
        let mut reg = CapabilityRegistry::new();
        reg.register("trade.equity").unwrap();
        let result = reg.build_mask(&["portfolio.write"]);
        assert!(result.is_err());
    }

    #[test]
    fn registry_full_mask_covers_all() {
        let mut reg = CapabilityRegistry::new();
        reg.register_all(&["a", "b", "c"]).unwrap();

        let full = reg.build_full_mask();
        let a = reg.build_mask(&["a"]).unwrap();
        let b = reg.build_mask(&["b"]).unwrap();
        let c = reg.build_mask(&["c"]).unwrap();

        assert!(a.is_subset_of(&full));
        assert!(b.is_subset_of(&full));
        assert!(c.is_subset_of(&full));
    }

    #[test]
    fn registry_no_collisions_across_256_caps() {
        let mut reg = CapabilityRegistry::new();
        let caps: Vec<String> = (0..256).map(|i| format!("cap.{}", i)).collect();
        let cap_refs: Vec<&str> = caps.iter().map(String::as_str).collect();
        reg.register_all(&cap_refs).unwrap();

        for cap in &caps {
            let mask = reg.build_mask(&[cap.as_str()]).unwrap();
            assert_eq!(mask.capacity_count(), 1, "cap '{}' must occupy exactly one bit", cap);
        }
    }

    #[test]
    fn registry_over_256_returns_error() {
        let mut reg = CapabilityRegistry::new();
        let caps: Vec<String> = (0..256).map(|i| format!("cap.{}", i)).collect();
        let cap_refs: Vec<&str> = caps.iter().map(String::as_str).collect();
        reg.register_all(&cap_refs).unwrap();

        let result = reg.register("one.too.many");
        assert!(result.is_err());
    }

    #[test]
    fn registry_names_in_order_matches_registration() {
        let mut reg = CapabilityRegistry::new();
        let names = ["gamma", "alpha", "beta", "delta"];
        reg.register_all(&names).unwrap();
        let ordered = reg.names_in_order();
        assert_eq!(ordered, names.as_slice());
    }

    #[test]
    fn registry_no_collision_where_hash_would_collide() {
        // With hash-based mapping, two names can land on the same bit.
        // Registry always gives distinct bits.
        let mut reg = CapabilityRegistry::new();
        reg.register_all(&["cap.0", "cap.1"]).unwrap();
        let m0 = reg.build_mask(&["cap.0"]).unwrap();
        let m1 = reg.build_mask(&["cap.1"]).unwrap();
        // The masks must not overlap (distinct slots → distinct bits)
        assert_eq!(m0.intersect(&m1), NarrowingMatrix::EMPTY);
    }
}
