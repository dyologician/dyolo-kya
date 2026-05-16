use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use a1::{
    anchor::{AnchoredReceipt, AnchorNetwork},
    zk::ZkChainCommitment,
    Clock, SystemClock,
};

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct AnchorRequest {
    /// The ZK chain commitment to anchor on-chain.
    pub commitment: ZkChainCommitment,
    /// `did:a1:` of the passport holder who authorized the action.
    pub passport_did: String,
    /// Target network: "ethereum", "polygon", "base", "arbitrum", "solana",
    /// "ethereum-sepolia", or `{"custom": {"chain_id": N, "name": "..."}}`.
    #[serde(default = "default_network")]
    pub network: AnchorNetwork,
}

fn default_network() -> AnchorNetwork {
    AnchorNetwork::Ethereum
}

#[derive(Debug, Serialize)]
pub struct AnchorResponse {
    pub anchored_receipt: AnchoredReceipt,
    pub anchor_hash_hex: String,
    pub network: String,
    /// Hex calldata for EVM chains. Submit via `eth_sendRawTransaction`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evm_calldata: Option<String>,
    /// Base58 instruction data for Solana anchor program.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solana_instruction_data: Option<String>,
    /// How to submit: human-readable next steps.
    pub submission_guide: SubmissionGuide,
}

#[derive(Debug, Serialize)]
pub struct SubmissionGuide {
    pub contract_function: &'static str,
    pub contract_interface: &'static str,
    pub ethersjs_snippet: Option<String>,
    pub viemjs_snippet: Option<String>,
}

/// POST /v1/anchor
///
/// Prepare an on-chain anchor for a ZK chain commitment. Returns ABI-encoded
/// calldata ready for submission to the A1 Anchor Contract on any EVM chain,
/// or instruction data for the Solana anchor program.
///
/// The gateway signs the anchor hash with its Ed25519 identity, providing an
/// additional endorsement that the commitment was verified by this gateway.
///
/// This endpoint does NOT submit the transaction — it returns the calldata.
/// Submit via ethers.js, viem, web3.py, or the `a1 anchor` CLI command.
pub async fn handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AnchorRequest>,
) -> impl IntoResponse {
    let now = SystemClock.unix_now();
    let network_name = req.network.name().to_owned();
    let is_evm = req.network.is_evm();

    let receipt = AnchoredReceipt::prepare(
        req.commitment,
        req.passport_did,
        req.network,
        now,
        &state.signing_identity,
    );

    let anchor_hex = receipt.anchor_hash_hex();
    let evm_calldata = receipt.evm_calldata.clone();
    let solana_data = receipt.solana_instruction_data.clone();

    let (ethersjs, viemjs) = if is_evm {
        let hex = evm_calldata.as_deref().unwrap_or("");
        (
            Some(format!(
                "await provider.sendTransaction({{ to: A1_ANCHOR_CONTRACT, data: '0x{}' }});",
                hex
            )),
            Some(format!(
                "await walletClient.sendTransaction({{ to: A1_ANCHOR_CONTRACT, data: '0x{}' }});",
                hex
            )),
        )
    } else {
        (None, None)
    };

    let resp = AnchorResponse {
        anchor_hash_hex: anchor_hex,
        network: network_name,
        evm_calldata,
        solana_instruction_data: solana_data,
        submission_guide: SubmissionGuide {
            contract_function: "anchor(bytes32,bytes32,uint64,string)",
            contract_interface: "https://github.com/dyologician/a1/blob/main/contracts/IA1Anchor.sol",
            ethersjs_snippet: ethersjs,
            viemjs_snippet: viemjs,
        },
        anchored_receipt: receipt,
    };

    (StatusCode::OK, Json(resp)).into_response()
}
