use crate::constants::chains::ChainId;
use crate::error::{Error, ModelResult};
use crate::models::types::common::TransferDetails;
use crate::models::types::user_types::{EVMData, SuiData};
use error_stack::report;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Common single chain order generic data
pub struct SingleChainGenericData {
    /// User address initiating the intent
    pub user: String,

    /// Chain identifier (e.g., Ethereum, Solana)
    pub chain_id: ChainId,
    /// The token address being spent in the operation
    pub token_in: String,
    /// Token to be received after the operation (e.g., "USDT", "DAI")
    pub token_out: String,
    /// The minimum amount of the output token to be received after the operation
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub amount_out_min: u128,
    /// Destination address for the operation (e.g., recipient address)
    pub destination_address: String,
    /// Requested array of extra transfers with fixed amounts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_transfers: Option<Vec<TransferDetails>>,
    /// Deadline for the operation, in Unix timestamp format, in SECONDS
    pub deadline: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Chain-specific single chain order data
pub enum SingleChainChainSpecificData {
    /// EVM-based chain data (e.g., Ethereum, Binance Smart Chain)
    EVM(EVMData),
    /// Sui-based chain data
    Sui(SuiData),
    /// Solana-based chain data
    Solana(SingleChainSolanaData),
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Solana-specific single chain order data
pub struct SingleChainSolanaData {
    /// Order account public key
    pub order_pubkey: String,
    /// Secret number for validating `secret_hash` that is stored on chain
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub secret_number: u64,
}

impl SingleChainChainSpecificData {
    pub fn try_get_evm(&self) -> ModelResult<&EVMData> {
        match self {
            SingleChainChainSpecificData::EVM(evm_data) => Ok(evm_data),
            _ => Err(report!(Error::LogicError(
                "Invalid chain-specific data".to_string()
            ))),
        }
    }

    pub fn try_get_solana(&self) -> ModelResult<&SingleChainSolanaData> {
        match self {
            SingleChainChainSpecificData::Solana(solana_data) => Ok(solana_data),
            _ => Err(report!(Error::LogicError(
                "Invalid chain-specific data".to_string()
            ))),
        }
    }

    pub fn try_get_sui(&self) -> ModelResult<&SuiData> {
        match self {
            SingleChainChainSpecificData::Sui(sui_data) => Ok(sui_data),
            _ => Err(report!(Error::LogicError(
                "Invalid chain-specific data".to_string()
            ))),
        }
    }
}
