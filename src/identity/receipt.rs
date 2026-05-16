use crate::chain::VerificationReceipt;
use crate::identity::narrowing::NarrowingMatrix;
use crate::provenance::{ProvenanceRoot, ProvenanceStepProof, ReasoningTrace};

/// The outcome of a passport-guarded authorization.
///
/// `ProvableReceipt` extends [`VerificationReceipt`] with the passport
/// namespace and narrowing commitment so that any downstream system can
/// independently verify which human authorized the action, under which
/// capability scope, without retaining any secrets.
///
/// The `narrowing_commitment` is a Blake3 hash over the enforced
/// [`NarrowingMatrix`], bound to the chain fingerprint. An auditor can
/// recompute it from the archived passport file and the receipt alone.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProvableReceipt {
    /// The underlying chain-level verification outcome.
    pub inner: VerificationReceipt,

    /// The namespace of the passport that authorized this action.
    pub passport_namespace: String,

    /// Blake3 commitment over the enforced capability mask at authorization time.
    ///
    /// Recompute via `NarrowingMatrix::from_hex(mask_hex).commitment()` to
    /// independently verify the enforced scope.
    pub narrowing_commitment: [u8; 32],

    /// Hex encoding of the `NarrowingMatrix` that was active at authorization time.
    pub capability_mask_hex: String,

    /// Optional cryptographic commitment to the agent's full reasoning trace.
    ///
    /// When present, this binds the receipt to the exact sequence of thoughts,
    /// tool calls, and decisions that led to the authorized action. Individual
    /// steps can be selectively disclosed to auditors via `ProvenanceStepProof`
    /// without revealing the full trace.
    ///
    /// Attach a trace with [`ProvableReceipt::with_provenance`] after
    /// authorization. Verify an individual step with
    /// [`ProvableReceipt::verify_provenance_step`].
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub provenance: Option<ProvenanceRoot>,
}

impl ProvableReceipt {
    pub(crate) fn new(
        inner: VerificationReceipt,
        passport_namespace: String,
        mask: &NarrowingMatrix,
    ) -> Self {
        Self {
            inner,
            passport_namespace,
            narrowing_commitment: mask.commitment(),
            capability_mask_hex: mask.to_hex(),
            provenance: None,
        }
    }

    /// Hex of the chain fingerprint — stable identifier for this authorization event.
    pub fn fingerprint_hex(&self) -> String {
        self.inner.fingerprint_hex()
    }

    /// Verify that the archived `capability_mask_hex` matches the stored commitment.
    ///
    /// Use this during audit to confirm the receipt has not been tampered with.
    pub fn verify_commitment(&self) -> bool {
        match NarrowingMatrix::from_hex(&self.capability_mask_hex) {
            Ok(m) => m.commitment() == self.narrowing_commitment,
            Err(_) => false,
        }
    }

    /// Attach a finalized `ProvenanceRoot` to this receipt.
    ///
    /// The `root` must have been produced by calling
    /// [`ReasoningTrace::finalize`] with `&self.inner.chain_fingerprint`
    /// as the `chain_fingerprint` argument. This ensures the reasoning trace
    /// is cryptographically bound to this specific authorization event.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let receipt = passport.guard_local(&chain, &agent_pk, &intent)?;
    /// let root = trace.finalize(now_unix, &receipt.inner.chain_fingerprint)?;
    /// let receipt = receipt.with_provenance(root);
    /// assert!(receipt.provenance.is_some());
    /// ```
    #[must_use]
    pub fn with_provenance(mut self, root: ProvenanceRoot) -> Self {
        self.provenance = Some(root);
        self
    }

    /// Finalize a `ReasoningTrace` and attach it in one step.
    ///
    /// Equivalent to calling `trace.finalize(finalized_at_unix, &self.inner.chain_fingerprint)`
    /// followed by `self.with_provenance(root)`, but more ergonomic when the
    /// receipt is already available.
    ///
    /// Returns `Err` if the trace is empty.
    pub fn bind_reasoning_trace(
        self,
        trace: &ReasoningTrace,
        finalized_at_unix: u64,
    ) -> Result<Self, crate::error::A1Error> {
        let root = trace.finalize(finalized_at_unix, &self.inner.chain_fingerprint)?;
        Ok(self.with_provenance(root))
    }

    /// Verify that `proof` is a valid inclusion proof for one of this receipt's
    /// recorded reasoning steps.
    ///
    /// Returns `false` if this receipt has no attached provenance, or if the
    /// proof does not verify against the stored `ProvenanceRoot`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let proof = trace.step_proof(2).unwrap();
    /// assert!(receipt.verify_provenance_step(&proof));
    /// ```
    pub fn verify_provenance_step(&self, proof: &ProvenanceStepProof) -> bool {
        match &self.provenance {
            Some(root) => proof.verify(root),
            None => false,
        }
    }

    /// Returns `true` if this receipt carries a cryptographic reasoning trace.
    #[inline]
    pub fn has_provenance(&self) -> bool {
        self.provenance.is_some()
    }
}

impl std::fmt::Display for ProvableReceipt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ProvableReceipt {{ namespace={}, depth={}, fingerprint={}, provenance={}, system=a1_dyolo_v2.8.0 }}",
            self.passport_namespace,
            self.inner.chain_depth,
            self.fingerprint_hex(),
            if self.has_provenance() { "attached" } else { "none" },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::narrowing::NarrowingMatrix;
    use crate::provenance::{ReasoningStepKind, ReasoningTrace};

    fn dummy_receipt() -> VerificationReceipt {
        VerificationReceipt {
            chain_depth: 1,
            verified_scope_root: [0u8; 32],
            intent: [1u8; 32],
            verified_at_unix: 1_700_000_000,
            chain_fingerprint: [2u8; 32],
            namespace: Some("test".into()),
        }
    }

    fn make_receipt() -> ProvableReceipt {
        let mask = NarrowingMatrix::from_capabilities(&["trade.equity"]);
        ProvableReceipt::new(dummy_receipt(), "test-ns".into(), &mask)
    }

    #[test]
    fn commitment_roundtrip() {
        let receipt = make_receipt();
        assert!(receipt.verify_commitment());
    }

    #[test]
    fn tampered_mask_fails_commitment() {
        let mut receipt = make_receipt();
        receipt.capability_mask_hex = NarrowingMatrix::FULL.to_hex();
        assert!(!receipt.verify_commitment());
    }

    #[test]
    fn no_provenance_by_default() {
        let receipt = make_receipt();
        assert!(!receipt.has_provenance());
        assert!(receipt.provenance.is_none());
    }

    #[test]
    fn with_provenance_attaches_root() {
        let receipt = make_receipt();
        let fp = receipt.inner.chain_fingerprint;
        let mut trace = ReasoningTrace::new(1_700_000_000);
        trace.record(ReasoningStepKind::Thought, b"analyzing the request", 1_700_000_001);
        trace.record(ReasoningStepKind::FinalAction, b"execute trade", 1_700_000_002);

        let root = trace.finalize(1_700_000_003, &fp).unwrap();
        let receipt = receipt.with_provenance(root);

        assert!(receipt.has_provenance());
        let pr = receipt.provenance.as_ref().unwrap();
        assert_eq!(pr.step_count, 2);
        assert!(pr.verify_chain_binding(&fp));
    }

    #[test]
    fn bind_reasoning_trace_convenience() {
        let receipt = make_receipt();
        let mut trace = ReasoningTrace::new(1_700_000_000);
        trace.record(ReasoningStepKind::Thought, b"step one", 1_700_000_001);

        let receipt = receipt.bind_reasoning_trace(&trace, 1_700_000_002).unwrap();
        assert!(receipt.has_provenance());
    }

    #[test]
    fn bind_empty_trace_returns_err() {
        let receipt = make_receipt();
        let trace = ReasoningTrace::new(1_700_000_000);
        assert!(receipt.bind_reasoning_trace(&trace, 1_700_000_001).is_err());
    }

    #[test]
    fn verify_provenance_step_without_provenance_is_false() {
        let receipt = make_receipt();
        let mut trace = ReasoningTrace::new(1_700_000_000);
        trace.record(ReasoningStepKind::Thought, b"only thought", 1_700_000_001);
        let proof = trace.step_proof(0).unwrap();
        assert!(!receipt.verify_provenance_step(&proof));
    }

    #[test]
    fn verify_provenance_step_roundtrip() {
        let mut trace = ReasoningTrace::new(1_700_000_000);
        trace.record(ReasoningStepKind::Thought, b"checking price", 1_700_000_001);
        trace.record_tool_call("get_price", b"{\"symbol\":\"AAPL\"}", 1_700_000_002);
        trace.record(ReasoningStepKind::Observation, b"182.50", 1_700_000_003);
        trace.record(ReasoningStepKind::FinalAction, b"buy AAPL x10", 1_700_000_004);

        let receipt = make_receipt();
        let fp = receipt.inner.chain_fingerprint;
        let root = trace.finalize(1_700_000_005, &fp).unwrap();
        let receipt = receipt.with_provenance(root);

        for i in 0..4 {
            let proof = trace.step_proof(i).unwrap();
            assert!(receipt.verify_provenance_step(&proof), "step {i} failed");
        }
    }

    #[test]
    fn tampered_step_fails_verify() {
        let mut trace = ReasoningTrace::new(1_700_000_000);
        trace.record(ReasoningStepKind::Thought, b"step a", 1_700_000_001);
        trace.record(ReasoningStepKind::Thought, b"step b", 1_700_000_002);

        let receipt = make_receipt();
        let fp = receipt.inner.chain_fingerprint;
        let root = trace.finalize(1_700_000_003, &fp).unwrap();
        let receipt = receipt.with_provenance(root);

        let mut proof = trace.step_proof(0).unwrap();
        proof.step.content_hash[0] ^= 0xFF;
        assert!(!receipt.verify_provenance_step(&proof));
    }

    #[test]
    fn display_shows_provenance_state() {
        let receipt = make_receipt();
        assert!(format!("{receipt}").contains("provenance=none"));

        let mut trace = ReasoningTrace::new(1_700_000_000);
        trace.record(ReasoningStepKind::Thought, b"x", 1_700_000_001);
        let fp = receipt.inner.chain_fingerprint;
        let root = trace.finalize(1_700_000_002, &fp).unwrap();
        let receipt = receipt.with_provenance(root);
        assert!(format!("{receipt}").contains("provenance=attached"));
    }
}
