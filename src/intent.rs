use std::collections::BTreeMap;

use crate::crypto::{hasher_intent_leaf, hasher_subscope, merkle_node};
use crate::error::A1Error;

/// A 32-byte commitment to a single authorized action.
pub type IntentHash = [u8; 32];

/// Maximum allowed length in bytes for an intent action string.
pub const MAX_ACTION_LEN: usize = 256;
/// Maximum allowed length in bytes for an intent parameter key.
pub const MAX_PARAM_KEY_LEN: usize = 128;
/// Maximum allowed length in bytes for an intent parameter value.
pub const MAX_PARAM_VALUE_LEN: usize = 4096;
/// Maximum allowed number of parameters per intent.
pub const MAX_INTENT_PARAMS: usize = 64;

// ── Structured Intent ─────────────────────────────────────────────────────────

/// A human-readable action with named, canonically-ordered parameters.
///
/// Parameters are sorted by key before hashing, so two `Intent` values
/// with identical fields but different insertion order produce the same hash.
/// This makes intent construction order-independent and audit logs readable.
///
/// # Examples
///
/// ```rust
/// use a1::Intent;
///
/// let intent = Intent::new("trade.equity").unwrap()
///     .param("symbol", "AAPL")
///     .param("side", "buy")
///     .param("limit_usd", "182.50")
///     .param("qty", "100");
///
/// let h = intent.hash();
/// assert_ne!(h, [0u8; 32]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Intent {
    /// Action identifier (e.g. `"trade.equity"`, `"query.portfolio"`).
    pub action: String,
    /// Named parameters, sorted by key for canonical serialization.
    pub params: BTreeMap<String, String>,
}

impl Intent {
    /// Create a new intent. Returns `A1Error::WireFormatError` if the action string
    /// is empty or exceeds `MAX_ACTION_LEN`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use a1::Intent;
    ///
    /// let intent = Intent::new("trade.equity").unwrap()
    ///     .param("symbol", "AAPL")
    ///     .param("side", "buy");
    ///
    /// assert!(Intent::new("").is_err());
    /// ```
    pub fn new(action: impl Into<String>) -> Result<Self, A1Error> {
        let action_str = action.into();
        if action_str.is_empty() {
            return Err(A1Error::WireFormatError(
                "Intent action cannot be empty".into(),
            ));
        }
        if action_str.len() > MAX_ACTION_LEN {
            return Err(A1Error::WireFormatError(format!(
                "Intent action exceeds maximum length of {MAX_ACTION_LEN}"
            )));
        }
        Ok(Self {
            action: action_str,
            params: BTreeMap::new(),
        })
    }

    /// Alias for `new`. Prefer `new` in new code; retained for API symmetry with `try_param`.
    #[inline]
    pub fn try_new(action: impl Into<String>) -> Result<Self, A1Error> {
        Self::new(action)
    }

    /// Attach a named parameter. Replaces any existing value for the same key.
    /// Panics on debug if limits are exceeded.
    /// Use `try_param` for validation-critical paths.
    pub fn param(self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.try_param(key, value)
            .expect("invalid intent parameter")
    }

    /// Attach a named parameter. Replaces any existing value for the same key.
    /// Returns `A1Error::WireFormatError` if limits are exceeded.
    ///
    /// Keys and values are normalized to lowercase and trimmed to ensure
    /// deterministic hashing regardless of how the caller constructs them.
    pub fn try_param(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, A1Error> {
        if self.params.len() >= MAX_INTENT_PARAMS {
            return Err(A1Error::WireFormatError(format!(
                "Intent exceeds maximum parameter count of {}",
                MAX_INTENT_PARAMS
            )));
        }
        let normalized_key = key.into().trim().to_lowercase();
        let normalized_value = value.into().trim().to_lowercase();
        if normalized_key.len() > MAX_PARAM_KEY_LEN {
            return Err(A1Error::WireFormatError(format!(
                "Intent parameter key exceeds maximum length of {}",
                MAX_PARAM_KEY_LEN
            )));
        }
        if normalized_value.len() > MAX_PARAM_VALUE_LEN {
            return Err(A1Error::WireFormatError(format!(
                "Intent parameter value exceeds maximum length of {}",
                MAX_PARAM_VALUE_LEN
            )));
        }
        self.params.insert(normalized_key, normalized_value);
        Ok(self)
    }

    /// Compute the domain-separated hash of this intent.
    ///
    /// Uses a prefix-free canonical encoding: each field is length-prefixed
    /// before its content, preventing length-extension and collision attacks.
    pub fn hash(&self) -> IntentHash {
        let mut h = hasher_intent_leaf(crate::cert::CERT_VERSION);
        h.update(b"a1::dyolo::intent::v2.8.0");
        h.update(&(self.action.len() as u64).to_le_bytes());
        h.update(self.action.as_bytes());
        h.update(&(self.params.len() as u64).to_le_bytes());
        for (k, v) in self.params.iter() {
            h.update(&(k.len() as u64).to_le_bytes());
            h.update(k.as_bytes());
            h.update(&(v.len() as u64).to_le_bytes());
            h.update(v.as_bytes());
        }
        h.finalize().into()
    }
}

impl std::fmt::Display for Intent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.action)?;
        if !self.params.is_empty() {
            write!(f, "[")?;
            let mut iter = self.params.iter().peekable();
            while let Some((k, v)) = iter.next() {
                write!(f, "{k}={v}")?;
                if iter.peek().is_some() {
                    write!(f, ",")?;
                }
            }
            write!(f, "]")?;
        }
        Ok(())
    }
}

// ── Low-level hashing ─────────────────────────────────────────────────────────

/// Hash a raw action identifier and opaque parameter bytes into a
/// domain-separated leaf hash.
///
/// The `params` argument is treated as a single opaque byte slice and encoded
/// with a length prefix before hashing. This encoding differs from
/// [`Intent::hash`], which encodes a `BTreeMap` of named key-value pairs with
/// individual per-field length prefixes. The two functions are **not**
/// interchangeable; use this only when the parameter payload is already
/// serialized by the caller and no structured field access is needed.
#[deprecated(
    since = "2.0.0",
    note = "Use `Intent::new` and `Intent::hash` to avoid serialization mismatches. This function will be removed in v3.0."
)]
pub fn intent_hash(action: &str, params: &[u8]) -> IntentHash {
    let mut h = hasher_intent_leaf(crate::cert::CERT_VERSION);
    h.update(&(action.len() as u64).to_le_bytes());
    h.update(action.as_bytes());
    h.update(&(params.len() as u64).to_le_bytes());
    h.update(params);
    h.finalize().into()
}

// ── Merkle Proof ──────────────────────────────────────────────────────────────

/// A Merkle path proving that one leaf belongs to a tree.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MerkleProof {
    pub siblings: Vec<SiblingNode>,
}

/// One node along a Merkle path.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SiblingNode {
    pub hash: IntentHash,
    /// When `true`, this sibling is the left child; the current node is on the right.
    pub is_left: bool,
}

impl MerkleProof {
    /// Recompute the root from `leaf` along this proof path and return whether
    /// it equals `expected_root`.
    pub fn verify(&self, leaf: &IntentHash, expected_root: &IntentHash) -> bool {
        let mut current = *leaf;
        for node in &self.siblings {
            current = if node.is_left {
                merkle_node(&node.hash, &current)
            } else {
                merkle_node(&current, &node.hash)
            };
        }
        use subtle::ConstantTimeEq;
        current.ct_eq(expected_root).into()
    }
}

// ── Intent Tree ───────────────────────────────────────────────────────────────

/// A Merkle tree over a set of authorized intent hashes.
///
/// The root commits to the full intent set without revealing its members.
/// Leaves are sorted and deduplicated, so the root is deterministic regardless
/// of insertion order.
pub struct IntentTree {
    leaves: Vec<IntentHash>,
    layers: Vec<Vec<IntentHash>>,
}

impl IntentTree {
    /// Build a tree from a set of intent hashes.
    /// Returns [`A1Error::EmptyTree`] if `intents` is empty.
    pub fn build(mut intents: Vec<IntentHash>) -> Result<Self, A1Error> {
        if intents.is_empty() {
            return Err(A1Error::EmptyTree);
        }
        intents.sort_unstable();
        intents.dedup();

        let depth = (usize::BITS - intents.len().leading_zeros()) as usize;
        let mut layers: Vec<Vec<IntentHash>> = Vec::with_capacity(depth);
        layers.push(intents.clone());

        let mut current = intents;
        while current.len() > 1 {
            let next_len = current.len().div_ceil(2);
            let mut next = Vec::with_capacity(next_len);
            for chunk in current.chunks(2) {
                if chunk.len() == 2 {
                    next.push(merkle_node(&chunk[0], &chunk[1]));
                } else {
                    next.push(chunk[0]);
                }
            }
            layers.push(next.clone());
            current = next;
        }

        let leaves = layers.first().expect("layers is never empty").clone();
        Ok(Self { leaves, layers })
    }

    /// The Merkle root — the canonical commitment to this intent set.
    pub fn root(&self) -> IntentHash {
        self.layers.last().unwrap()[0]
    }

    /// Generate an inclusion proof for `intent`.
    /// Returns [`A1Error::IntentNotFound`] if the intent is not in this tree.
    pub fn prove(&self, intent: &IntentHash) -> Result<MerkleProof, A1Error> {
        let mut pos = self
            .leaves
            .binary_search(intent)
            .map_err(|_| A1Error::IntentNotFound)?;

        let mut siblings = Vec::new();
        for layer in self.layers.iter().take(self.layers.len() - 1) {
            let sibling_pos = if pos.is_multiple_of(2) { pos + 1 } else { pos - 1 };
            if sibling_pos < layer.len() {
                siblings.push(SiblingNode {
                    hash: layer[sibling_pos],
                    is_left: !pos.is_multiple_of(2),
                });
            }
            pos /= 2;
        }

        Ok(MerkleProof { siblings })
    }

    pub fn contains(&self, intent: &IntentHash) -> bool {
        self.leaves.binary_search(intent).is_ok()
    }

    pub fn leaf_count(&self) -> usize {
        self.leaves.len()
    }
}

// ── Sub-scope Proof ───────────────────────────────────────────────────────────

/// Cryptographic evidence that a delegated scope is a strict subset of the
/// delegator's authorized intent set.
///
/// Each entry in `subset_intents` has a corresponding Merkle proof against
/// the parent scope root. After verification, the subset is formed into its
/// own Merkle tree whose root becomes the child scope root.
///
/// An empty `SubScopeProof` is a full-scope pass-through: the child receives
/// the same scope root as the parent. This is explicit in the API — callers
/// must consciously choose between [`SubScopeProof::full_passthrough`] and
/// [`SubScopeProof::build`].
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SubScopeProof {
    pub subset_intents: Vec<IntentHash>,
    pub proofs: Vec<MerkleProof>,
}

impl SubScopeProof {
    /// Full-scope pass-through: the delegated scope equals the parent scope.
    pub fn full_passthrough() -> Self {
        Self::default()
    }

    /// Build a sub-scope proof by proving each intent against `parent_tree`.
    /// Returns [`A1Error::IntentNotFound`] if any intent is absent from the tree.
    pub fn build(parent_tree: &IntentTree, intents: &[IntentHash]) -> Result<Self, A1Error> {
        let proofs = intents
            .iter()
            .map(|intent| parent_tree.prove(intent))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            subset_intents: intents.to_vec(),
            proofs,
        })
    }

    /// Verify every subset intent against `parent_root`, then return the
    /// Merkle root of the subset as the new delegated scope root.
    ///
    /// Returns [`A1Error::InvalidSubScopeProof`] if any proof fails.
    pub fn verify_and_derive_root(&self, parent_root: &IntentHash) -> Result<IntentHash, A1Error> {
        if self.subset_intents.is_empty() {
            return Ok(*parent_root);
        }
        if self.subset_intents.len() != self.proofs.len() {
            return Err(A1Error::InvalidSubScopeProof);
        }
        for (intent, proof) in self.subset_intents.iter().zip(self.proofs.iter()) {
            if !proof.verify(intent, parent_root) {
                return Err(A1Error::InvalidSubScopeProof);
            }
        }
        let sub_tree = IntentTree::build(self.subset_intents.clone())?;
        Ok(sub_tree.root())
    }

    /// A deterministic commitment to the full proof structure.
    ///
    /// Included in every certificate signature so no one can substitute
    /// a different proof on an existing certificate.
    pub fn commitment(&self) -> [u8; 32] {
        let mut h = hasher_subscope(crate::cert::CERT_VERSION);
        h.update(b"a1::dyolo::subscope::v2.8.0");
        h.update(&(self.subset_intents.len() as u64).to_le_bytes());
        for intent in &self.subset_intents {
            h.update(intent);
        }
        h.update(&(self.proofs.len() as u64).to_le_bytes());
        for proof in &self.proofs {
            h.update(&(proof.siblings.len() as u64).to_le_bytes());
            for node in &proof.siblings {
                h.update(&node.hash);
                h.update(&[node.is_left as u8]);
            }
        }
        h.finalize().into()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(deprecated)]
    fn sample_intents() -> Vec<IntentHash> {
        (0..8u8)
            .map(|i| intent_hash(&format!("action_{i}"), &[i]))
            .collect()
    }

    #[test]
    fn tree_root_is_deterministic() {
        let a = IntentTree::build(sample_intents()).unwrap();
        let mut reversed = sample_intents();
        reversed.reverse();
        let b = IntentTree::build(reversed).unwrap();
        assert_eq!(a.root(), b.root());
    }

    #[test]
    fn proofs_verify_for_all_leaves() {
        let intents = sample_intents();
        let tree = IntentTree::build(intents.clone()).unwrap();
        let root = tree.root();
        for intent in &intents {
            let proof = tree.prove(intent).unwrap();
            assert!(proof.verify(intent, &root));
        }
    }

    #[test]
    #[allow(deprecated)]
    fn unknown_intent_proof_fails() {
        let tree = IntentTree::build(sample_intents()).unwrap();
        let unknown = intent_hash("unknown", b"");
        assert_eq!(tree.prove(&unknown), Err(A1Error::IntentNotFound));
    }

    #[test]
    fn sub_scope_derives_correct_root() {
        let intents = sample_intents();
        let tree = IntentTree::build(intents.clone()).unwrap();
        let subset = &intents[..3];

        let proof = SubScopeProof::build(&tree, subset).unwrap();
        let derived = proof.verify_and_derive_root(&tree.root()).unwrap();

        let expected = IntentTree::build(subset.to_vec()).unwrap().root();
        assert_eq!(derived, expected);
    }

    #[test]
    fn full_passthrough_returns_parent_root() {
        let tree = IntentTree::build(sample_intents()).unwrap();
        let root = tree.root();
        let derived = SubScopeProof::full_passthrough()
            .verify_and_derive_root(&root)
            .unwrap();
        assert_eq!(derived, root);
    }

    #[test]
    fn intent_struct_hash_is_order_independent() {
        let a = Intent::new("trade")
            .unwrap()
            .param("symbol", "AAPL")
            .param("qty", "100");
        let b = Intent::new("trade")
            .unwrap()
            .param("qty", "100")
            .param("symbol", "AAPL");
        assert_eq!(a.hash(), b.hash());
    }

    #[test]
    fn intent_display() {
        let s = Intent::new("trade.equity")
            .unwrap()
            .param("symbol", "AAPL")
            .to_string();
        assert!(s.contains("trade.equity"));
        assert!(s.contains("symbol=aapl"));
    }
}
