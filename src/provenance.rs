use blake3::Hasher;

use crate::cert::CERT_VERSION;
use crate::crypto::merkle_node;
use crate::error::A1Error;
use crate::registry::fresh_nonce;

const DOMAIN_PROVENANCE_LEAF: &str = "a1::provenance::leaf::v1";
const DOMAIN_PROVENANCE_ROOT: &str = "a1::provenance::root::v1";
const DOMAIN_PROVENANCE_META: &str = "a1::provenance::meta::v1";

// ── ReasoningStepKind ─────────────────────────────────────────────────────────

/// The semantic category of a recorded reasoning step.
///
/// These map directly to the observation/thought/action loop used in ReAct-style
/// agents and are compatible with LangChain, LlamaIndex, AutoGen, and OpenAI
/// Agents SDK trace structures.
///
/// The numeric value is stable across library versions and appears in the
/// Merkle leaf hash. Adding a new variant requires a new leaf domain version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ReasoningStepKind {
    /// An internal model thought or chain-of-thought step.
    Thought = 1,
    /// A tool or function call executed by the agent.
    ToolCall = 2,
    /// The output returned from a tool or function call.
    Observation = 3,
    /// A branching decision point with enumerated alternatives considered.
    Decision = 4,
    /// A high-level plan step decomposed from the goal.
    PlanStep = 5,
    /// The final action taken as the outcome of reasoning.
    FinalAction = 6,
    /// An error or exception that influenced reasoning.
    Error = 7,
    /// Retrieval result incorporated into reasoning context.
    Retrieval = 8,
}

impl ReasoningStepKind {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Thought => "thought",
            Self::ToolCall => "tool_call",
            Self::Observation => "observation",
            Self::Decision => "decision",
            Self::PlanStep => "plan_step",
            Self::FinalAction => "final_action",
            Self::Error => "error",
            Self::Retrieval => "retrieval",
        }
    }
}

impl std::fmt::Display for ReasoningStepKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

// ── ReasoningStep ─────────────────────────────────────────────────────────────

/// A single recorded step in an agent's reasoning trace.
///
/// Actual content is never stored — only a Blake3 hash of it. This preserves
/// privacy while maintaining verifiability: an auditor who holds the original
/// content can independently verify its hash matches the recorded step.
///
/// # Content Hashing
///
/// Hash the content before recording:
///
/// ```rust,ignore
/// use a1::provenance::{ReasoningTrace, ReasoningStepKind};
///
/// let thought = "I should buy AAPL because ...";
/// let content_hash = blake3::hash(thought.as_bytes()).into();
///
/// trace.record_hashed(ReasoningStepKind::Thought, content_hash, None);
/// ```
///
/// Or use the convenience methods that hash for you:
///
/// ```rust,ignore
/// trace.record(ReasoningStepKind::Thought, thought.as_bytes());
/// trace.record_tool_call("get_stock_price", r#"{"symbol":"AAPL"}"#.as_bytes());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReasoningStep {
    /// Position in the trace (0-indexed, monotonically increasing).
    pub index: u32,
    /// Semantic category of this step.
    pub kind: ReasoningStepKind,
    /// Blake3 hash of the actual step content.
    #[cfg_attr(feature = "serde", serde(with = "hex_32"))]
    pub content_hash: [u8; 32],
    /// Unix timestamp when this step was recorded (seconds).
    pub timestamp_unix: u64,
    /// Blake3 hash of optional structured metadata (tool name, model id, etc.).
    /// All-zeros when no metadata is associated.
    #[cfg_attr(feature = "serde", serde(with = "hex_32"))]
    pub metadata_hash: [u8; 32],
}

impl ReasoningStep {
    /// Compute the Merkle leaf hash for this step.
    ///
    /// The leaf is a Blake3 keyed hash over all fields in a canonical order,
    /// bound to the step index. This ensures that reordering steps changes
    /// the Merkle root even if content hashes are identical.
    pub fn leaf_hash(&self) -> [u8; 32] {
        let mut h = Hasher::new_derive_key(DOMAIN_PROVENANCE_LEAF);
        h.update(&[CERT_VERSION]);
        h.update(&self.index.to_le_bytes());
        h.update(&[self.kind.as_u8()]);
        h.update(&self.content_hash);
        h.update(&self.timestamp_unix.to_be_bytes());
        h.update(&self.metadata_hash);
        h.finalize().into()
    }
}

// ── ProvenanceRoot ────────────────────────────────────────────────────────────

/// A compact cryptographic commitment to a complete reasoning trace.
///
/// `ProvenanceRoot` is the value stored in a `ProvableReceipt`. It binds the
/// receipt to the specific sequence of reasoning steps that led to the
/// authorized action, without storing the steps themselves.
///
/// # Selective Disclosure
///
/// An auditor can request disclosure of individual steps. The agent provides
/// the original step content plus a `ProvenanceStepProof`, which the auditor
/// verifies against the `merkle_root` stored in the archived receipt.
///
/// # Reconstruction
///
/// Given the archived `ProvableReceipt` (containing `ProvenanceRoot`) and the
/// original step contents, any party can independently verify the complete
/// reasoning trace. No secrets are required.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProvenanceRoot {
    /// Total number of recorded reasoning steps.
    pub step_count: u32,
    /// Merkle root over all reasoning step leaf hashes.
    #[cfg_attr(feature = "serde", serde(with = "hex_32"))]
    pub merkle_root: [u8; 32],
    /// Random nonce identifying this trace instance.
    ///
    /// Prevents two traces with identical step sequences from producing the
    /// same `ProvenanceRoot`, protecting against trace substitution.
    #[cfg_attr(feature = "serde", serde(with = "hex_16"))]
    pub trace_id: [u8; 16],
    /// Unix timestamp when the trace was started.
    pub started_at_unix: u64,
    /// Unix timestamp when the trace was finalized.
    pub finalized_at_unix: u64,
    /// Blake3 commitment over the `ProvenanceRoot` fields themselves, bound
    /// to the issuing chain fingerprint. Prevents a root from being detached
    /// from its receipt and reattached to a different authorization event.
    #[cfg_attr(feature = "serde", serde(with = "hex_32"))]
    pub chain_binding: [u8; 32],
}

impl ProvenanceRoot {
    /// Recompute the `chain_binding` from the archived root and a chain fingerprint.
    ///
    /// Returns `true` if the root has not been tampered with and was issued
    /// against the provided chain fingerprint.
    pub fn verify_chain_binding(&self, chain_fingerprint: &[u8; 32]) -> bool {
        let expected = compute_chain_binding(self, chain_fingerprint);
        subtle::ConstantTimeEq::ct_eq(&expected[..], &self.chain_binding[..]).unwrap_u8() == 1
    }

    /// Hex-encoded Merkle root for logging and display.
    pub fn merkle_root_hex(&self) -> String {
        hex::encode(self.merkle_root)
    }

    /// Hex-encoded trace ID.
    pub fn trace_id_hex(&self) -> String {
        hex::encode(self.trace_id)
    }
}

fn compute_chain_binding(root: &ProvenanceRoot, chain_fp: &[u8; 32]) -> [u8; 32] {
    let mut h = Hasher::new_derive_key(DOMAIN_PROVENANCE_ROOT);
    h.update(&root.step_count.to_le_bytes());
    h.update(&root.merkle_root);
    h.update(&root.trace_id);
    h.update(&root.started_at_unix.to_be_bytes());
    h.update(&root.finalized_at_unix.to_be_bytes());
    h.update(chain_fp);
    h.finalize().into()
}

// ── ProvenanceStepProof ───────────────────────────────────────────────────────

/// A Merkle inclusion proof for a single reasoning step.
///
/// Proves that `step` was the `step.index`-th element of the trace whose
/// root is `claimed_root`, without revealing any other step.
///
/// # Usage
///
/// ```rust,ignore
/// let proof = trace.step_proof(2).unwrap();
///
/// let receipt: ProvableReceipt = /* archived */;
/// let root = receipt.provenance.as_ref().unwrap();
///
/// assert!(proof.verify(root));
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProvenanceStepProof {
    /// The step being proved.
    pub step: ReasoningStep,
    /// Sibling hashes from leaf to root. Length is `ceil(log2(next_power_of_two(step_count)))`.
    pub siblings: Vec<[u8; 32]>,
    /// Total step count of the original trace (determines tree shape).
    pub step_count: u32,
}

impl ProvenanceStepProof {
    /// Verify that `self.step` is included in a trace with the given `ProvenanceRoot`.
    pub fn verify(&self, root: &ProvenanceRoot) -> bool {
        if self.step.index >= self.step_count {
            return false;
        }
        if self.step_count != root.step_count {
            return false;
        }

        let leaf_count = next_power_of_two(self.step_count as usize);
        let expected_depth = leaf_count.trailing_zeros() as usize;

        if self.siblings.len() != expected_depth {
            return false;
        }

        let mut current = self.step.leaf_hash();
        let mut idx = self.step.index as usize;

        for sibling in &self.siblings {
            if idx.is_multiple_of(2) {
                current = merkle_node(&current, sibling);
            } else {
                current = merkle_node(sibling, &current);
            }
            idx >>= 1;
        }

        subtle::ConstantTimeEq::ct_eq(&current[..], &root.merkle_root[..]).unwrap_u8() == 1
    }
}

// ── ReasoningTrace ────────────────────────────────────────────────────────────

/// Builder for an agent's reasoning trace.
///
/// Record steps as the agent reasons, then call `finalize` to produce a
/// `ProvenanceRoot` for embedding in the `ProvableReceipt`.
///
/// # Thread safety
///
/// `ReasoningTrace` is not `Send + Sync`. It is intended to be held within a
/// single agent execution context. If multiple concurrent threads contribute
/// to a single trace, synchronize access externally.
///
/// # Example — LangChain-style agent
///
/// ```rust,ignore
/// use a1::provenance::{ReasoningTrace, ReasoningStepKind};
/// use a1::chain::SystemClock;
///
/// let clock = SystemClock;
/// let mut trace = ReasoningTrace::new(&clock);
///
/// trace.record(ReasoningStepKind::Thought,
///     b"I need to check the current AAPL price before deciding.");
///
/// trace.record_tool_call("get_stock_price",
///     b"{\"symbol\": \"AAPL\"}");
///
/// trace.record(ReasoningStepKind::Observation,
///     b"AAPL is trading at $182.50, within the limit.");
///
/// trace.record(ReasoningStepKind::FinalAction,
///     b"Executing buy order: AAPL x100 @ $182.50");
///
/// // Authorize the intent ...
/// let receipt = passport.guard_local(&chain, &agent_pk, &intent)?;
///
/// // Bind provenance to the receipt
/// let root = trace.finalize(&clock, &receipt.inner.chain_fingerprint)?;
/// let receipt_with_provenance = receipt.with_provenance(root);
/// ```
pub struct ReasoningTrace {
    steps: Vec<ReasoningStep>,
    trace_id: [u8; 16],
    started_at_unix: u64,
}

impl ReasoningTrace {
    /// Start a new trace. Records the start timestamp from `clock`.
    pub fn new(started_at_unix: u64) -> Self {
        Self {
            steps: Vec::new(),
            trace_id: fresh_nonce(),
            started_at_unix,
        }
    }

    /// Record a step by hashing the provided content bytes.
    ///
    /// Content is hashed with Blake3 before storage. The raw content is not
    /// retained. For selective disclosure, preserve the raw content externally.
    pub fn record(
        &mut self,
        kind: ReasoningStepKind,
        content: &[u8],
        timestamp_unix: u64,
    ) -> &ReasoningStep {
        let content_hash = blake3::hash(content).into();
        self.record_hashed(kind, content_hash, [0u8; 32], timestamp_unix)
    }

    /// Record a tool call with its input payload.
    pub fn record_tool_call(
        &mut self,
        tool_name: &str,
        input: &[u8],
        timestamp_unix: u64,
    ) -> &ReasoningStep {
        let content_hash = blake3::hash(input).into();
        let metadata_hash = hash_metadata(&[("tool", tool_name)]);
        self.record_hashed(ReasoningStepKind::ToolCall, content_hash, metadata_hash, timestamp_unix)
    }

    /// Record an observation returned by a tool.
    pub fn record_observation(
        &mut self,
        tool_name: &str,
        output: &[u8],
        timestamp_unix: u64,
    ) -> &ReasoningStep {
        let content_hash = blake3::hash(output).into();
        let metadata_hash = hash_metadata(&[("tool", tool_name)]);
        self.record_hashed(ReasoningStepKind::Observation, content_hash, metadata_hash, timestamp_unix)
    }

    /// Record a pre-hashed step with explicit metadata hash.
    ///
    /// Use this when you want to control hashing yourself, or when the
    /// content was hashed by a different system.
    pub fn record_hashed(
        &mut self,
        kind: ReasoningStepKind,
        content_hash: [u8; 32],
        metadata_hash: [u8; 32],
        timestamp_unix: u64,
    ) -> &ReasoningStep {
        let index = self.steps.len() as u32;
        self.steps.push(ReasoningStep {
            index,
            kind,
            content_hash,
            timestamp_unix,
            metadata_hash,
        });
        self.steps.last().expect("just pushed")
    }

    /// Number of steps recorded so far.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Finalize the trace and produce a `ProvenanceRoot`.
    ///
    /// `chain_fingerprint` MUST be the fingerprint of the `VerificationReceipt`
    /// that will carry this root. This binds the provenance to a specific
    /// authorization event — the root cannot be reused across receipts.
    ///
    /// Returns `Err` if the trace is empty.
    pub fn finalize(
        &self,
        finalized_at_unix: u64,
        chain_fingerprint: &[u8; 32],
    ) -> Result<ProvenanceRoot, A1Error> {
        if self.steps.is_empty() {
            return Err(A1Error::EmptyTree);
        }

        let merkle_root = build_merkle_root(&self.steps);

        let mut root = ProvenanceRoot {
            step_count: self.steps.len() as u32,
            merkle_root,
            trace_id: self.trace_id,
            started_at_unix: self.started_at_unix,
            finalized_at_unix,
            chain_binding: [0u8; 32],
        };

        root.chain_binding = compute_chain_binding(&root, chain_fingerprint);
        Ok(root)
    }

    /// Generate a Merkle inclusion proof for the step at `index`.
    ///
    /// Returns `None` if `index` is out of range.
    pub fn step_proof(&self, index: usize) -> Option<ProvenanceStepProof> {
        if index >= self.steps.len() {
            return None;
        }

        let leaf_count = next_power_of_two(self.steps.len());
        let mut leaves: Vec<[u8; 32]> = self.steps.iter().map(|s| s.leaf_hash()).collect();
        let last = *leaves.last().expect("non-empty");
        leaves.resize(leaf_count, last);

        let depth = leaf_count.trailing_zeros() as usize;
        let mut siblings = Vec::with_capacity(depth);
        let mut layer = leaves;
        let mut idx = index;

        for _ in 0..depth {
            let sibling_idx = if idx.is_multiple_of(2) { idx + 1 } else { idx - 1 };
            siblings.push(layer[sibling_idx]);
            let next_len = layer.len() / 2;
            let mut next = Vec::with_capacity(next_len);
            for i in 0..next_len {
                next.push(merkle_node(&layer[2 * i], &layer[2 * i + 1]));
            }
            layer = next;
            idx >>= 1;
        }

        Some(ProvenanceStepProof {
            step: self.steps[index].clone(),
            siblings,
            step_count: self.steps.len() as u32,
        })
    }

    /// Iterate over all recorded steps.
    pub fn steps(&self) -> &[ReasoningStep] {
        &self.steps
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn build_merkle_root(steps: &[ReasoningStep]) -> [u8; 32] {
    assert!(!steps.is_empty());

    let leaf_count = next_power_of_two(steps.len());
    let mut layer: Vec<[u8; 32]> = steps.iter().map(|s| s.leaf_hash()).collect();
    let last = *layer.last().expect("non-empty");
    layer.resize(leaf_count, last);

    while layer.len() > 1 {
        let next_len = layer.len() / 2;
        let mut next = Vec::with_capacity(next_len);
        for i in 0..next_len {
            next.push(merkle_node(&layer[2 * i], &layer[2 * i + 1]));
        }
        layer = next;
    }

    layer[0]
}

fn next_power_of_two(n: usize) -> usize {
    if n <= 1 {
        return 1;
    }
    let mut p = 1usize;
    while p < n {
        p <<= 1;
    }
    p
}

fn hash_metadata(pairs: &[(&str, &str)]) -> [u8; 32] {
    let mut h = Hasher::new_derive_key(DOMAIN_PROVENANCE_META);
    h.update(&(pairs.len() as u32).to_le_bytes());
    for (k, v) in pairs {
        h.update(&(k.len() as u32).to_le_bytes());
        h.update(k.as_bytes());
        h.update(&(v.len() as u32).to_le_bytes());
        h.update(v.as_bytes());
    }
    h.finalize().into()
}

// ── Serde helpers ─────────────────────────────────────────────────────────────

#[cfg(feature = "serde")]
mod hex_32 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &[u8; 32], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(v))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
        let raw = hex::decode(String::deserialize(d)?).map_err(serde::de::Error::custom)?;
        raw.try_into().map_err(|_| serde::de::Error::custom("expected 32-byte hex"))
    }
}

#[cfg(feature = "serde")]
mod hex_16 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &[u8; 16], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(v))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 16], D::Error> {
        let raw = hex::decode(String::deserialize(d)?).map_err(serde::de::Error::custom)?;
        raw.try_into().map_err(|_| serde::de::Error::custom("expected 16-byte hex"))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_chain_fp() -> [u8; 32] {
        let mut fp = [0u8; 32];
        fp[0] = 0xAB;
        fp[31] = 0xCD;
        fp
    }

    fn build_trace(n: usize) -> ReasoningTrace {
        let mut trace = ReasoningTrace::new(1_700_000_000);
        for i in 0..n {
            trace.record(
                ReasoningStepKind::Thought,
                format!("step {i}").as_bytes(),
                1_700_000_000 + i as u64,
            );
        }
        trace
    }

    #[test]
    fn single_step_trace_finalizes() {
        let trace = build_trace(1);
        let fp = fake_chain_fp();
        let root = trace.finalize(1_700_001_000, &fp).unwrap();
        assert_eq!(root.step_count, 1);
        assert!(root.verify_chain_binding(&fp));
    }

    #[test]
    fn chain_binding_fails_wrong_fp() {
        let trace = build_trace(3);
        let fp = fake_chain_fp();
        let root = trace.finalize(1_700_001_000, &fp).unwrap();
        let mut wrong_fp = fp;
        wrong_fp[0] ^= 0xFF;
        assert!(!root.verify_chain_binding(&wrong_fp));
    }

    #[test]
    fn empty_trace_returns_error() {
        let trace = ReasoningTrace::new(1_700_000_000);
        let fp = fake_chain_fp();
        assert!(trace.finalize(1_700_001_000, &fp).is_err());
    }

    #[test]
    fn merkle_proof_verifies_each_step() {
        for n in [1usize, 2, 3, 4, 5, 7, 8, 9, 15, 16] {
            let trace = build_trace(n);
            let fp = fake_chain_fp();
            let root = trace.finalize(1_700_001_000, &fp).unwrap();

            for i in 0..n {
                let proof = trace.step_proof(i).expect("step exists");
                assert!(
                    proof.verify(&root),
                    "proof failed for step {i} in trace of {n}"
                );
            }
        }
    }

    #[test]
    fn step_proof_out_of_range_is_none() {
        let trace = build_trace(3);
        assert!(trace.step_proof(3).is_none());
        assert!(trace.step_proof(100).is_none());
    }

    #[test]
    fn tampered_step_content_fails_proof() {
        let trace = build_trace(4);
        let fp = fake_chain_fp();
        let root = trace.finalize(1_700_001_000, &fp).unwrap();
        let mut proof = trace.step_proof(2).unwrap();
        proof.step.content_hash[0] ^= 0x01;
        assert!(!proof.verify(&root));
    }

    #[test]
    fn reordered_step_index_fails_proof() {
        let trace = build_trace(4);
        let fp = fake_chain_fp();
        let root = trace.finalize(1_700_001_000, &fp).unwrap();
        let mut proof = trace.step_proof(1).unwrap();
        proof.step.index = 3;
        assert!(!proof.verify(&root));
    }

    #[test]
    fn different_traces_produce_different_roots() {
        let fp = fake_chain_fp();
        let t1 = build_trace(3);
        let t2 = build_trace(3);
        let r1 = t1.finalize(1_700_001_000, &fp).unwrap();
        let r2 = t2.finalize(1_700_001_000, &fp).unwrap();
        assert_ne!(r1.trace_id, r2.trace_id);
        assert_ne!(r1.chain_binding, r2.chain_binding);
    }

    #[test]
    fn tool_call_and_observation_record_metadata() {
        let mut trace = ReasoningTrace::new(1_700_000_000);
        let step = trace.record_tool_call("search", b"AAPL price", 1_700_000_001);
        assert_eq!(step.kind, ReasoningStepKind::ToolCall);
        assert_ne!(step.metadata_hash, [0u8; 32]);

        let step = trace.record_observation("search", b"182.50", 1_700_000_002);
        assert_eq!(step.kind, ReasoningStepKind::Observation);
        assert_ne!(step.metadata_hash, [0u8; 32]);
    }

    #[test]
    fn leaf_hash_is_index_sensitive() {
        let mut s1 = ReasoningStep {
            index: 0,
            kind: ReasoningStepKind::Thought,
            content_hash: [1u8; 32],
            timestamp_unix: 1_700_000_000,
            metadata_hash: [0u8; 32],
        };
        let hash_at_0 = s1.leaf_hash();
        s1.index = 1;
        let hash_at_1 = s1.leaf_hash();
        assert_ne!(hash_at_0, hash_at_1);
    }
}