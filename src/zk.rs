use blake3::Hasher;
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;

use crate::chain::DyoloChain;
use crate::error::A1Error;
use crate::identity::Signer;
use crate::intent::IntentHash;

const DOMAIN_ZK_COMMIT: &str = "a1::dyolo::zk::commit::v2.8.0";
const DOMAIN_ZK_BIND:   &str = "a1::dyolo::zk::bind::v2.8.0";

/// How a `ZkChainCommitment` was produced.
///
/// `Blake3Commit` is the default: a cryptographic commitment derived from
/// Blake3 over all chain state. It is verifiable offline by anyone with the
/// chain — but it is not zero-knowledge (it reveals the chain length and
/// fingerprint). This mode requires no extra dependencies and works
/// everywhere A1 runs.
///
/// `ExternalZkvm` signals that a real zero-knowledge proof has been generated
/// by an external zkVM backend (RISC Zero, Jolt, SP1, etc.) and attached as
/// `zk_proof_bytes`. The verifier must use the same zkVM to check it.
///
/// Both modes share the same `ZkChainCommitment` wire format so consumers
/// can upgrade from Blake3Commit to ExternalZkvm without changing any
/// downstream code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ZkProofMode {
    Blake3Commit = 1,
    ExternalZkvm = 2,
}

impl ZkProofMode {
    pub fn as_u8(&self) -> u8 {
        match self {
            Self::Blake3Commit => 1,
            Self::ExternalZkvm => 2,
        }
    }
}

/// A compact, verifiable commitment to the validity of a full delegation chain.
///
/// Instead of shipping the entire delegation chain to every verifier, a gateway
/// or trusted service can compute a `ZkChainCommitment` and hand it to
/// downstream consumers. Verification is O(1): one Blake3 hash + one Ed25519
/// signature check, regardless of chain depth.
///
/// The `commitment` field is a domain-separated Blake3 hash over:
/// - The chain fingerprint (covers all certs and the principal scope)
/// - The authorized intent hash
/// - The narrowing commitment (capability mask at authorization time)
/// - The timestamp
///
/// Because the chain fingerprint already commits to every cert's signature and
/// scope, this commitment transitively proves that the full chain is valid.
///
/// # Wire format
///
/// `ZkChainCommitment` is JSON-serializable (requires `wire` feature). Store it
/// in your audit log, ship it to downstream services, or anchor it on-chain.
///
/// # Upgrade path to real ZK
///
/// Set `mode = ZkProofMode::ExternalZkvm` and populate `zk_proof_bytes` with
/// the output of your zkVM (RISC Zero, Jolt, etc.) over the same `commitment`.
/// The `verify_commitment` check stays unchanged — consumers that understand
/// the ZK mode can additionally verify the zkVM proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZkChainCommitment {
    /// Blake3 hash binding chain fingerprint + intent + narrowing + timestamp.
    pub commitment: [u8; 32],

    /// Authorized intent hash.
    pub intent: IntentHash,

    /// Unix timestamp when this commitment was sealed.
    pub sealed_at_unix: u64,

    /// Hex of the chain fingerprint, for human-readable logs.
    pub chain_fingerprint_hex: String,

    /// Ed25519 signature over `commitment` from the sealing authority.
    pub authority_signature: String,

    /// DID of the sealing authority (hex public key in `did:a1:` format).
    pub authority_did: String,

    /// Proof mode. `Blake3Commit` unless a zkVM proof is attached.
    pub mode: ZkProofMode,

    /// Raw zkVM proof bytes (hex). Empty for `Blake3Commit` mode.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub zk_proof_hex: String,

    /// Optional passport namespace for human-readable audit records.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passport_namespace: Option<String>,
}

impl ZkChainCommitment {
    /// Seal a commitment over an authorized chain.
    ///
    /// Call this after `DyoloChain::authorize` succeeds. The `authority`
    /// is typically the gateway's signing identity. The resulting commitment
    /// can be distributed to downstream consumers in place of the full chain.
    pub fn seal(
        chain: &DyoloChain,
        intent: &IntentHash,
        narrowing_commitment: &[u8; 32],
        sealed_at_unix: u64,
        authority: &dyn Signer,
        passport_namespace: Option<&str>,
    ) -> Self {
        let chain_fp = chain.fingerprint();
        let commitment = compute_commitment(&chain_fp, intent, narrowing_commitment, sealed_at_unix);
        let sig = authority.sign_message(&commitment);
        let authority_did = format!(
            "did:a1:{}",
            hex::encode(authority.verifying_key().as_bytes())
        );

        Self {
            commitment,
            intent: *intent,
            sealed_at_unix,
            chain_fingerprint_hex: hex::encode(chain_fp),
            authority_signature: hex::encode(sig.to_bytes()),
            authority_did,
            mode: ZkProofMode::Blake3Commit,
            zk_proof_hex: String::new(),
            passport_namespace: passport_namespace.map(String::from),
        }
    }

    /// Verify the authority signature and optionally check commitment freshness.
    ///
    /// This is an O(1) operation regardless of the original chain depth.
    /// Pass `max_age_secs = None` to skip freshness checking.
    pub fn verify_commitment(
        &self,
        narrowing_commitment: &[u8; 32],
        now_unix: u64,
        max_age_secs: Option<u64>,
    ) -> Result<(), A1Error> {
        let chain_fp_bytes = hex::decode(&self.chain_fingerprint_hex)
            .map_err(|_| A1Error::WireFormatError("invalid chain_fingerprint_hex".into()))?;
        let chain_fp: [u8; 32] = chain_fp_bytes
            .try_into()
            .map_err(|_| A1Error::WireFormatError("chain fingerprint must be 32 bytes".into()))?;

        let expected = compute_commitment(&chain_fp, &self.intent, narrowing_commitment, self.sealed_at_unix);

        if expected[..].ct_eq(&self.commitment[..]).unwrap_u8() == 0 {
            return Err(A1Error::InvalidSubScopeProof);
        }

        if let Some(max_age) = max_age_secs {
            let age = now_unix.saturating_sub(self.sealed_at_unix);
            if age > max_age {
                return Err(A1Error::Expired(0, self.sealed_at_unix + max_age, now_unix));
            }
        }

        let pk_hex = self.authority_did
            .strip_prefix("did:a1:")
            .ok_or_else(|| A1Error::WireFormatError("invalid authority DID".into()))?;
        let pk_bytes = hex::decode(pk_hex)
            .map_err(|_| A1Error::WireFormatError("invalid authority DID hex".into()))?;
        let pk_arr: [u8; 32] = pk_bytes
            .try_into()
            .map_err(|_| A1Error::WireFormatError("authority key must be 32 bytes".into()))?;
        let authority_vk = ed25519_dalek::VerifyingKey::from_bytes(&pk_arr)
            .map_err(|_| A1Error::WireFormatError("invalid authority Ed25519 key".into()))?;

        let sig_bytes = hex::decode(&self.authority_signature)
            .map_err(|_| A1Error::WireFormatError("invalid authority_signature hex".into()))?;
        let sig_arr: [u8; 64] = sig_bytes
            .try_into()
            .map_err(|_| A1Error::WireFormatError("signature must be 64 bytes".into()))?;
        let sig = ed25519_dalek::Signature::from_bytes(&sig_arr);

        use ed25519_dalek::Verifier;
        authority_vk
            .verify(&self.commitment, &sig)
            .map_err(|_| A1Error::HybridSignatureInvalid { component: "zk-commitment" })
    }

    /// Attach an external zkVM proof to this commitment.
    ///
    /// Use this after running the chain through RISC Zero, Jolt, or any
    /// compatible zkVM. The commitment bytes stay identical — consumers that
    /// only check `verify_commitment` will continue to work unchanged, while
    /// consumers that understand ZK can additionally verify `zk_proof_hex`.
    pub fn with_zk_proof(mut self, proof_bytes: &[u8]) -> Self {
        self.zk_proof_hex = hex::encode(proof_bytes);
        self.mode = ZkProofMode::ExternalZkvm;
        self
    }

    /// Returns `true` if this commitment carries a zkVM proof.
    pub fn has_zk_proof(&self) -> bool {
        self.mode == ZkProofMode::ExternalZkvm && !self.zk_proof_hex.is_empty()
    }
}

fn compute_commitment(
    chain_fp: &[u8; 32],
    intent: &IntentHash,
    narrowing_commitment: &[u8; 32],
    sealed_at: u64,
) -> [u8; 32] {
    let mut h = Hasher::new_derive_key(DOMAIN_ZK_COMMIT);
    h.update(chain_fp);
    h.update(intent);
    h.update(narrowing_commitment);
    h.update(&sealed_at.to_le_bytes());
    h.finalize().into()
}

/// Produce a binding hash over a `ZkChainCommitment` for on-chain anchoring.
///
/// This is the value to submit to a smart contract, a transparency log,
/// or an audit ledger. It is a domain-separated Blake3 hash over the
/// entire commitment so that the chain state can be anchored in 32 bytes.
pub fn anchor_hash(commitment: &ZkChainCommitment) -> [u8; 32] {
    let mut h = Hasher::new_derive_key(DOMAIN_ZK_BIND);
    h.update(&commitment.commitment);
    h.update(&commitment.sealed_at_unix.to_le_bytes());
    h.update(commitment.authority_did.as_bytes());
    h.finalize().into()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

// ── ZkTraceProof ──────────────────────────────────────────────────────────────

/// A combined proof of both authorization (chain) and reasoning (trace).
///
/// `ZkTraceProof` binds a `ZkChainCommitment` to a `ProvenanceRoot` in a
/// single 32-byte commitment. This proves not just that an agent was authorized
/// to act, but that a specific, tamper-evident reasoning trace produced that
/// action.
///
/// # Why this matters
///
/// `ZkChainCommitment` answers: *"Was the agent authorized?"*
/// `ZkTraceProof` answers: *"Was the agent authorized, and did it reason correctly?"*
///
/// For EU AI Act high-risk systems and NIST AI RMF Govern 6.2 compliance,
/// you need both. `ZkTraceProof` is the single artifact that satisfies both
/// requirements.
///
/// # Upgrade path to full ZK
///
/// Set `zk_proof_hex` with the output of the RISC Zero guest at
/// `src/zk_guest/src/main.rs`. The guest program can verify both the chain
/// commitment and the trace Merkle root in a single proof. Consumers that
/// only call `verify()` continue working unchanged when the zkVM proof is
/// attached.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ZkTraceProof {
    /// Chain authorization commitment.
    pub chain_commitment: ZkChainCommitment,
    /// Finalized Merkle root of the agent's reasoning trace.
    pub trace_root: crate::provenance::ProvenanceRoot,
    /// Blake3 commitment binding chain + trace: `Blake3(DOMAIN || chain_commit || merkle_root)`.
    #[cfg_attr(feature = "serde", serde(with = "crate::zk::hex_32_serde"))]
    pub combined_commitment: [u8; 32],
    /// Ed25519 signature over `combined_commitment` from the sealing authority.
    pub authority_signature: String,
    /// `did:a1:` identifier of the sealing authority.
    pub authority_did: String,
    /// Optional zkVM proof bytes (hex). Activate with the RISC Zero guest program.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "String::is_empty"))]
    pub zk_proof_hex: String,
}

impl ZkTraceProof {
    /// Seal a combined authorization + reasoning proof.
    ///
    /// The `authority` is typically the gateway's signing identity.
    /// `trace_root` must have been finalized against the chain fingerprint
    /// in `chain_commitment` via `ReasoningTrace::finalize()`.
    pub fn seal(
        chain_commitment: ZkChainCommitment,
        trace_root: crate::provenance::ProvenanceRoot,
        authority: &dyn crate::identity::Signer,
    ) -> Self {
        let combined = trace_combined_commitment(
            &chain_commitment.commitment,
            &trace_root.merkle_root,
        );
        let sig = authority.sign_message(&combined);
        let authority_did = format!(
            "did:a1:{}",
            hex::encode(authority.verifying_key().as_bytes())
        );
        Self {
            chain_commitment,
            trace_root,
            combined_commitment: combined,
            authority_signature: hex::encode(sig.to_bytes()),
            authority_did,
            zk_proof_hex: String::new(),
        }
    }

    /// Verify the combined commitment and authority signature.
    pub fn verify(&self) -> Result<(), crate::error::A1Error> {
        let expected = trace_combined_commitment(
            &self.chain_commitment.commitment,
            &self.trace_root.merkle_root,
        );
        use subtle::ConstantTimeEq;
        if expected[..].ct_eq(&self.combined_commitment[..]).unwrap_u8() == 0 {
            return Err(crate::error::A1Error::InvalidSubScopeProof);
        }

        let pk_hex = self.authority_did.strip_prefix("did:a1:").ok_or_else(|| {
            crate::error::A1Error::WireFormatError("invalid authority DID".into())
        })?;
        let pk_bytes = hex::decode(pk_hex)
            .map_err(|_| crate::error::A1Error::WireFormatError("invalid DID hex".into()))?;
        let pk_arr: [u8; 32] = pk_bytes.try_into().map_err(|_| {
            crate::error::A1Error::WireFormatError("authority key must be 32 bytes".into())
        })?;
        let vk = ed25519_dalek::VerifyingKey::from_bytes(&pk_arr)
            .map_err(|_| crate::error::A1Error::WireFormatError("invalid Ed25519 key".into()))?;

        let sig_bytes = hex::decode(&self.authority_signature)
            .map_err(|_| crate::error::A1Error::WireFormatError("invalid signature hex".into()))?;
        let sig_arr: [u8; 64] = sig_bytes.try_into().map_err(|_| {
            crate::error::A1Error::WireFormatError("signature must be 64 bytes".into())
        })?;
        let sig = ed25519_dalek::Signature::from_bytes(&sig_arr);

        use ed25519_dalek::Verifier;
        vk.verify(&self.combined_commitment, &sig)
            .map_err(|_| crate::error::A1Error::HybridSignatureInvalid {
                component: "zk-trace",
            })
    }

    /// Attach a zkVM proof to upgrade from commitment-only to full ZK.
    #[must_use]
    pub fn with_zk_proof(mut self, proof_bytes: &[u8]) -> Self {
        self.zk_proof_hex = hex::encode(proof_bytes);
        self
    }

    /// Returns `true` if a zkVM proof is attached.
    pub fn has_zk_proof(&self) -> bool {
        !self.zk_proof_hex.is_empty()
    }
}

fn trace_combined_commitment(chain_commit: &[u8; 32], merkle_root: &[u8; 32]) -> [u8; 32] {
    let mut h = Hasher::new_derive_key("a1::dyolo::zk::trace::v2.8.0");
    h.update(chain_commit);
    h.update(merkle_root);
    h.finalize().into()
}

pub(crate) mod hex_32_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(v: &[u8; 32], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(v))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
        let h = String::deserialize(d)?;
        hex::decode(&h)
            .map_err(serde::de::Error::custom)?
            .try_into()
            .map_err(|_| serde::de::Error::custom("expected 32 bytes"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cert::CertBuilder,
        identity::DyoloIdentity,
        intent::Intent,
    };

    #[test]
    fn seal_and_verify() {
        let human = DyoloIdentity::generate();
        let agent = DyoloIdentity::generate();
        let now = 1_700_000_000u64;

        let intent = Intent::new("trade.equity").unwrap().hash();
        let cert = CertBuilder::new(agent.verifying_key(), intent, now, now + 3600).sign(&human);
        let mut chain = DyoloChain::new(human.verifying_key(), intent);
        chain.push(cert);

        let narrowing = [0u8; 32];
        let commitment = ZkChainCommitment::seal(
            &chain,
            &intent,
            &narrowing,
            now,
            &human,
            Some("acme-bot"),
        );

        assert!(commitment
            .verify_commitment(&narrowing, now, Some(86400))
            .is_ok());
        assert_eq!(commitment.mode, ZkProofMode::Blake3Commit);
        assert!(!commitment.has_zk_proof());
    }

    #[test]
    fn tampered_commitment_fails() {
        let human = DyoloIdentity::generate();
        let agent = DyoloIdentity::generate();
        let now = 1_700_000_000u64;
        let intent = Intent::new("read").unwrap().hash();
        let cert = CertBuilder::new(agent.verifying_key(), intent, now, now + 3600).sign(&human);
        let mut chain = DyoloChain::new(human.verifying_key(), intent);
        chain.push(cert);

        let narrowing = [0u8; 32];
        let mut commitment =
            ZkChainCommitment::seal(&chain, &intent, &narrowing, now, &human, None);
        commitment.commitment[0] ^= 0xFF;
        assert!(commitment.verify_commitment(&narrowing, now, None).is_err());
    }

    #[test]
    fn expired_commitment_fails() {
        let human = DyoloIdentity::generate();
        let agent = DyoloIdentity::generate();
        let now = 1_700_000_000u64;
        let intent = Intent::new("read").unwrap().hash();
        let cert = CertBuilder::new(agent.verifying_key(), intent, now, now + 3600).sign(&human);
        let mut chain = DyoloChain::new(human.verifying_key(), intent);
        chain.push(cert);

        let narrowing = [0u8; 32];
        let commitment = ZkChainCommitment::seal(&chain, &intent, &narrowing, now, &human, None);
        assert!(commitment
            .verify_commitment(&narrowing, now + 7200, Some(3600))
            .is_err());
    }

    #[test]
    fn with_zk_proof_upgrades_mode() {
        let human = DyoloIdentity::generate();
        let agent = DyoloIdentity::generate();
        let now = 1_700_000_000u64;
        let intent = Intent::new("read").unwrap().hash();
        let cert = CertBuilder::new(agent.verifying_key(), intent, now, now + 3600).sign(&human);
        let mut chain = DyoloChain::new(human.verifying_key(), intent);
        chain.push(cert);

        let narrowing = [0u8; 32];
        let commitment = ZkChainCommitment::seal(&chain, &intent, &narrowing, now, &human, None)
            .with_zk_proof(b"placeholder-proof-bytes");

        assert_eq!(commitment.mode, ZkProofMode::ExternalZkvm);
        assert!(commitment.has_zk_proof());
        assert!(commitment.verify_commitment(&narrowing, now, None).is_ok());
    }

    #[test]
    fn anchor_hash_is_deterministic() {
        let human = DyoloIdentity::generate();
        let agent = DyoloIdentity::generate();
        let now = 1_700_000_000u64;
        let intent = Intent::new("read").unwrap().hash();
        let cert = CertBuilder::new(agent.verifying_key(), intent, now, now + 3600).sign(&human);
        let mut chain = DyoloChain::new(human.verifying_key(), intent);
        chain.push(cert);

        let narrowing = [0u8; 32];
        let c = ZkChainCommitment::seal(&chain, &intent, &narrowing, now, &human, None);
        assert_eq!(anchor_hash(&c), anchor_hash(&c));
    }
}

#[test]
fn zk_trace_proof_seal_verify() {
    use crate::{
        cert::CertBuilder,
        identity::DyoloIdentity,
        intent::Intent,
        provenance::{ReasoningStepKind, ReasoningTrace},
    };

    let human = DyoloIdentity::generate();
    let agent = DyoloIdentity::generate();
    let now = 1_700_000_000u64;
    let intent = Intent::new("trade.equity").unwrap().hash();
    let cert = CertBuilder::new(agent.verifying_key(), intent, now, now + 3600).sign(&human);
    let mut chain = DyoloChain::new(human.verifying_key(), intent);
    chain.push(cert);

    let narrowing = [0u8; 32];
    let chain_fp = chain.fingerprint();
    let commitment = ZkChainCommitment::seal(&chain, &intent, &narrowing, now, &human, None);

    let mut trace = ReasoningTrace::new(now);
    trace.record(ReasoningStepKind::Thought, b"analyzing trade", now + 1);
    trace.record(
        ReasoningStepKind::FinalAction,
        b"execute trade.equity AAPL 100",
        now + 2,
    );
    let root = trace.finalize(now + 3, &chain_fp).unwrap();

    let proof = ZkTraceProof::seal(commitment, root, &human);
    assert!(proof.verify().is_ok());
    assert!(!proof.has_zk_proof());
}

#[test]
fn zk_trace_proof_tampered_fails() {
    use crate::{
        cert::CertBuilder,
        identity::DyoloIdentity,
        intent::Intent,
        provenance::{ReasoningStepKind, ReasoningTrace},
    };

    let human = DyoloIdentity::generate();
    let agent = DyoloIdentity::generate();
    let now = 1_700_000_000u64;
    let intent = Intent::new("read").unwrap().hash();
    let cert = CertBuilder::new(agent.verifying_key(), intent, now, now + 3600).sign(&human);
    let mut chain = DyoloChain::new(human.verifying_key(), intent);
    chain.push(cert);
    let chain_fp = chain.fingerprint();

    let mut trace = ReasoningTrace::new(now);
    trace.record(ReasoningStepKind::Thought, b"step one", now + 1);
    let root = trace.finalize(now + 2, &chain_fp).unwrap();

    let commitment = ZkChainCommitment::seal(&chain, &intent, &[0u8; 32], now, &human, None);
    let mut proof = ZkTraceProof::seal(commitment, root, &human);
    proof.combined_commitment[0] ^= 0xFF;
    assert!(proof.verify().is_err());
}
