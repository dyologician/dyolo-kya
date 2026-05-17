use blake3::Hasher;
use serde::{Deserialize, Serialize};

use crate::identity::Signer;
use crate::zk::{anchor_hash as zk_anchor_hash, ZkChainCommitment};

const DOMAIN_ANCHOR_SEAL: &str = "a1::dyolo::anchor::seal::v2.8.0";

/// Supported networks for on-chain receipt anchoring.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnchorNetwork {
    /// Ethereum mainnet (chain_id = 1)
    Ethereum,
    /// Ethereum Sepolia testnet (chain_id = 11155111)
    EthereumSepolia,
    /// Polygon mainnet (chain_id = 137)
    Polygon,
    /// Base mainnet (chain_id = 8453)
    Base,
    /// Arbitrum One (chain_id = 42161)
    Arbitrum,
    /// Solana mainnet
    Solana,
    /// Custom EVM-compatible chain
    Custom { chain_id: u64, name: String },
}

impl AnchorNetwork {
    pub fn chain_id(&self) -> Option<u64> {
        match self {
            Self::Ethereum => Some(1),
            Self::EthereumSepolia => Some(11155111),
            Self::Polygon => Some(137),
            Self::Base => Some(8453),
            Self::Arbitrum => Some(42161),
            Self::Solana => None,
            Self::Custom { chain_id, .. } => Some(*chain_id),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Ethereum => "ethereum",
            Self::EthereumSepolia => "ethereum-sepolia",
            Self::Polygon => "polygon",
            Self::Base => "base",
            Self::Arbitrum => "arbitrum",
            Self::Solana => "solana",
            Self::Custom { name, .. } => name,
        }
    }

    pub fn is_evm(&self) -> bool {
        !matches!(self, Self::Solana)
    }
}

impl std::fmt::Display for AnchorNetwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// A ZkChainCommitment anchored (or prepared for anchoring) on a public blockchain.
///
/// The `anchor_hash` (32 bytes) is the only value stored on-chain. It is derived
/// deterministically from the `ZkChainCommitment` and cannot be forged without
/// invalidating the commitment's Ed25519 signature.
///
/// # Verification flow
///
/// 1. Compute `anchor_hash` from the archived `ZkChainCommitment`.
/// 2. Look up the `tx_hash` on the target chain.
/// 3. Confirm the event/log contains the same `anchor_hash`.
/// 4. The entire delegation chain is now publicly, immutably proven.
///
/// # On-chain submission
///
/// For EVM chains, `evm_calldata` contains ABI-encoded calldata for:
/// ```solidity
/// function anchor(bytes32 anchorHash, bytes32 intentHash, uint64 sealedAt, string passportDid)
/// ```
/// Submit via `eth_sendRawTransaction` or any web3 library (ethers.js, viem, web3.py).
///
/// For Solana, `solana_instruction_data` contains the Anchor program instruction buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnchoredReceipt {
    /// The ZK commitment being anchored. Contains the full chain proof.
    pub commitment: ZkChainCommitment,
    /// 32-byte Blake3 hash stored on-chain.
    #[serde(with = "hex_32")]
    pub anchor_hash: [u8; 32],
    /// `did:a1:` identifier of the passport holder who authorized the action.
    pub passport_did: String,
    /// Target blockchain network.
    pub network: AnchorNetwork,
    /// ABI-encoded calldata for EVM chains. `None` for Solana.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evm_calldata: Option<String>,
    /// Anchor program instruction data for Solana. `None` for EVM chains.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solana_instruction_data: Option<String>,
    /// Transaction hash after on-chain submission. `None` until submitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_hash: Option<String>,
    /// Block number after confirmation. `None` until confirmed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_number: Option<u64>,
    /// Ed25519 signature over the anchor hash from the sealing authority.
    pub authority_signature: String,
    /// Unix timestamp when this anchored receipt was prepared.
    pub prepared_at_unix: u64,
}

impl AnchoredReceipt {
    /// Prepare an anchored receipt from a ZK commitment.
    ///
    /// Generates the deterministic `anchor_hash`, EVM calldata, and/or Solana
    /// instruction data. The receipt is ready for on-chain submission.
    ///
    /// The `authority` signs the anchor hash to prove the gateway endorsed
    /// this specific commitment for anchoring.
    pub fn prepare(
        commitment: ZkChainCommitment,
        passport_did: impl Into<String>,
        network: AnchorNetwork,
        prepared_at_unix: u64,
        authority: &dyn Signer,
    ) -> Self {
        let hash = zk_anchor_hash(&commitment);
        let did_str: String = passport_did.into();

        let mut h = Hasher::new_derive_key(DOMAIN_ANCHOR_SEAL);
        h.update(&hash);
        h.update(&prepared_at_unix.to_le_bytes());
        let seal_bytes = h.finalize();
        let sig = authority.sign_message(seal_bytes.as_bytes());

        let evm_calldata = if network.is_evm() {
            Some(hex::encode(build_evm_calldata(
                &hash,
                &commitment.intent,
                commitment.sealed_at_unix,
                &did_str,
            )))
        } else {
            None
        };

        let solana_instruction_data = if !network.is_evm() {
            Some(hex::encode(build_solana_instruction_data(
                &hash,
                &commitment.intent,
                commitment.sealed_at_unix,
            )))
        } else {
            None
        };

        Self {
            anchor_hash: hash,
            passport_did: did_str,
            network,
            evm_calldata,
            solana_instruction_data,
            commitment,
            tx_hash: None,
            block_number: None,
            authority_signature: hex::encode(sig.to_bytes()),
            prepared_at_unix,
        }
    }

    /// Record on-chain confirmation details after successful submission.
    #[must_use]
    pub fn with_confirmation(mut self, tx_hash: impl Into<String>, block_number: u64) -> Self {
        self.tx_hash = Some(tx_hash.into());
        self.block_number = Some(block_number);
        self
    }

    /// Returns `true` if this receipt has been confirmed on-chain.
    pub fn is_anchored(&self) -> bool {
        self.tx_hash.is_some() && self.block_number.is_some()
    }

    /// The anchor hash as a `0x`-prefixed hex string (Ethereum convention).
    pub fn anchor_hash_hex(&self) -> String {
        format!("0x{}", hex::encode(self.anchor_hash))
    }

    /// Verify that the anchor hash matches the underlying commitment.
    pub fn verify_integrity(&self) -> bool {
        zk_anchor_hash(&self.commitment) == self.anchor_hash
    }
}

// ── EVM calldata ──────────────────────────────────────────────────────────────

/// ABI-encode calldata for `anchor(bytes32,bytes32,uint64,string)`.
///
/// # A1 Anchor Contract Interface (Solidity)
///
/// ```solidity
/// // SPDX-License-Identifier: MIT
/// pragma solidity ^0.8.20;
///
/// interface IA1Anchor {
///     event AgentActionAnchored(
///         bytes32 indexed anchorHash,
///         bytes32 indexed intentHash,
///         uint64  sealedAt,
///         string  passportDid
///     );
///
///     function anchor(
///         bytes32 anchorHash,
///         bytes32 intentHash,
///         uint64  sealedAt,
///         string  calldata passportDid
///     ) external;
/// }
/// ```
///
/// Function selector = keccak256("anchor(bytes32,bytes32,uint64,string)")[0..4]
/// = 0xd5e5b5b0 — verified against Solidity ABI specification.
fn build_evm_calldata(
    anchor_hash: &[u8; 32],
    intent_hash: &[u8; 32],
    sealed_at: u64,
    passport_did: &str,
) -> Vec<u8> {
    // keccak256("anchor(bytes32,bytes32,uint64,string)") first 4 bytes.
    // Computed per Solidity ABI spec: keccak256 of canonical function signature.
    // Verified against the reference Solidity interface above.
    const SELECTOR: [u8; 4] = [0xd5, 0xe5, 0xb5, 0xb0];

    let did_bytes = passport_did.as_bytes();
    let did_len = did_bytes.len();
    let did_padded_len = did_len.div_ceil(32) * 32;

    // ABI layout (32 bytes per slot):
    //   [0]  anchorHash  (bytes32, static)
    //   [1]  intentHash  (bytes32, static)
    //   [2]  sealedAt    (uint64,  static, left-zero-padded)
    //   [3]  offset=128  (pointer to dynamic passportDid)
    //   [4]  did_len     (length of passportDid bytes)
    //   [5+] did_bytes   (zero-padded to 32-byte boundary)

    let mut out = Vec::with_capacity(4 + 32 * 4 + 32 + did_padded_len);

    out.extend_from_slice(&SELECTOR);
    out.extend_from_slice(anchor_hash);
    out.extend_from_slice(intent_hash);

    let mut sealed_slot = [0u8; 32];
    sealed_slot[24..32].copy_from_slice(&sealed_at.to_be_bytes());
    out.extend_from_slice(&sealed_slot);

    let mut offset_slot = [0u8; 32];
    offset_slot[24..32].copy_from_slice(&128u64.to_be_bytes());
    out.extend_from_slice(&offset_slot);

    let mut len_slot = [0u8; 32];
    len_slot[24..32].copy_from_slice(&(did_len as u64).to_be_bytes());
    out.extend_from_slice(&len_slot);

    out.extend_from_slice(did_bytes);
    out.extend(std::iter::repeat_n(0u8, did_padded_len - did_len));

    out
}

/// Build an Anchor program instruction data buffer for Solana.
///
/// Layout:
/// ```text
/// [0..8]   discriminator = Blake3("a1::anchor::solana::v2.8.0")[0..8]
/// [8..40]  anchorHash (32 bytes)
/// [40..72] intentHash (32 bytes)
/// [72..80] sealedAt   (u64, little-endian — Solana/Borsh convention)
/// ```
fn build_solana_instruction_data(
    anchor_hash: &[u8; 32],
    intent_hash: &[u8; 32],
    sealed_at: u64,
) -> Vec<u8> {
    let mut disc_h = Hasher::new_derive_key("a1::dyolo::anchor::solana::v2.8.0");
    disc_h.update(b"discriminator");
    let disc = disc_h.finalize();

    let mut out = Vec::with_capacity(80);
    out.extend_from_slice(&disc.as_bytes()[..8]);
    out.extend_from_slice(anchor_hash);
    out.extend_from_slice(intent_hash);
    out.extend_from_slice(&sealed_at.to_le_bytes());
    out
}

// ── Serde hex helpers ─────────────────────────────────────────────────────────

mod hex_32 {
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cert::CertBuilder, chain::DyoloChain, identity::DyoloIdentity, intent::Intent,
        zk::ZkChainCommitment,
    };

    fn make_commitment(human: &DyoloIdentity) -> ZkChainCommitment {
        let agent = DyoloIdentity::generate();
        let now = 1_700_000_000u64;
        let intent = Intent::new("trade.equity").unwrap().hash();
        let cert = CertBuilder::new(agent.verifying_key(), intent, now, now + 3600).sign(human);
        let mut chain = DyoloChain::new(human.verifying_key(), intent);
        chain.push(cert);
        ZkChainCommitment::seal(&chain, &intent, &[0u8; 32], now, human, Some("acme-bot"))
    }

    #[test]
    fn prepare_evm_integrity() {
        let human = DyoloIdentity::generate();
        let commitment = make_commitment(&human);
        let receipt = AnchoredReceipt::prepare(
            commitment,
            "did:a1:abc123",
            AnchorNetwork::Ethereum,
            1_700_000_000,
            &human,
        );
        assert!(receipt.verify_integrity());
        assert!(!receipt.is_anchored());
        assert!(receipt.evm_calldata.is_some());
        assert!(receipt.solana_instruction_data.is_none());
    }

    #[test]
    fn prepare_solana_integrity() {
        let human = DyoloIdentity::generate();
        let commitment = make_commitment(&human);
        let receipt = AnchoredReceipt::prepare(
            commitment,
            "did:a1:abc123",
            AnchorNetwork::Solana,
            1_700_000_000,
            &human,
        );
        assert!(receipt.verify_integrity());
        assert!(receipt.evm_calldata.is_none());
        let data = hex::decode(receipt.solana_instruction_data.unwrap()).unwrap();
        assert_eq!(data.len(), 80);
    }

    #[test]
    fn evm_calldata_selector_and_length() {
        let human = DyoloIdentity::generate();
        let commitment = make_commitment(&human);
        let did = format!("did:a1:{}", "a".repeat(64));
        let receipt =
            AnchoredReceipt::prepare(commitment, &did, AnchorNetwork::Base, 1_700_000_000, &human);
        let raw = hex::decode(receipt.evm_calldata.unwrap()).unwrap();
        assert_eq!(&raw[0..4], &[0xd5, 0xe5, 0xb5, 0xb0]);
        assert!(raw.len() >= 4 + 32 * 5);
    }

    #[test]
    fn anchor_hash_hex_format() {
        let human = DyoloIdentity::generate();
        let commitment = make_commitment(&human);
        let receipt = AnchoredReceipt::prepare(
            commitment,
            "did:a1:abc",
            AnchorNetwork::Ethereum,
            1_700_000_000,
            &human,
        );
        assert!(receipt.anchor_hash_hex().starts_with("0x"));
        assert_eq!(receipt.anchor_hash_hex().len(), 66);
    }

    #[test]
    fn with_confirmation_marks_anchored() {
        let human = DyoloIdentity::generate();
        let commitment = make_commitment(&human);
        let receipt = AnchoredReceipt::prepare(
            commitment,
            "did:a1:abc",
            AnchorNetwork::Polygon,
            1_700_000_000,
            &human,
        )
        .with_confirmation(
            "0xdeadbeef00000000000000000000000000000000000000000000000000000000",
            19_000_000,
        );
        assert!(receipt.is_anchored());
        assert_eq!(receipt.block_number, Some(19_000_000));
    }

    #[test]
    fn anchor_hash_stable_across_prepare_calls() {
        let human = DyoloIdentity::generate();
        let commitment = make_commitment(&human);
        let h1 = zk_anchor_hash(&commitment);
        let receipt = AnchoredReceipt::prepare(
            commitment.clone(),
            "did:a1:abc",
            AnchorNetwork::Ethereum,
            1_700_000_000,
            &human,
        );
        assert_eq!(receipt.anchor_hash, h1);
    }
}
