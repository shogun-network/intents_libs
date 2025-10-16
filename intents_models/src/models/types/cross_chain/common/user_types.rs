use crate::constants::chains::ChainId;
use crate::error::{Error, ModelResult};
use crate::models::types::common::TransferDetails;
use crate::models::types::user_types::{EVMData, SuiData};
use crate::models::types::utils::get_number_of_unique_receivers;
use error_stack::report;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Common data for all cross chain orders
pub struct CrossChainGenericData {
    /// User address initiating the intent
    pub user: String,

    /// Source chain identifier (e.g., Ethereum, Solana)
    pub src_chain_id: ChainId,
    /// The token being spent in the operation (e.g., "ETH", "BTC")
    pub token_in: String,
    /// Minimum amount of stablecoins that Tokens IN may be swapped for
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub min_stablecoins_amount: u128,

    /// Destination chain identifier
    pub dest_chain_id: ChainId,
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
    /// SHA-256 hash of `execution_details` JSON String (hex format)
    pub execution_details_hash: String,
}

impl CrossChainGenericData {
    pub fn get_number_of_unique_receivers(&self) -> usize {
        get_number_of_unique_receivers(
            &self.token_out,
            &self.destination_address,
            &self.extra_transfers,
        )
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Chain-specific data cross chain orders
pub enum CrossChainChainSpecificData {
    /// EVM-based chain data (e.g., Ethereum, Binance Smart Chain)
    EVM(EVMData),
    /// Sui-based chain data
    Sui(SuiData),
    /// Solana-based chain data
    Solana(CrossChainSolanaData),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Solana-specific data cross chain orders
pub struct CrossChainSolanaData {
    /// Order account public key
    pub order_pubkey: String,
}

impl CrossChainChainSpecificData {
    pub fn try_get_evm(&self) -> ModelResult<&EVMData> {
        match self {
            CrossChainChainSpecificData::EVM(evm_data) => Ok(evm_data),
            _ => Err(report!(Error::LogicError(
                "Invalid chain-specific data".to_string()
            ))),
        }
    }

    pub fn try_get_solana(&self) -> ModelResult<&CrossChainSolanaData> {
        match self {
            CrossChainChainSpecificData::Solana(solana_data) => Ok(solana_data),
            _ => Err(report!(Error::LogicError(
                "Invalid chain-specific data".to_string()
            ))),
        }
    }

    pub fn try_get_sui(&self) -> ModelResult<&SuiData> {
        match self {
            CrossChainChainSpecificData::Sui(sui_data) => Ok(sui_data),
            _ => Err(report!(Error::LogicError(
                "Invalid chain-specific data".to_string()
            ))),
        }
    }
}
