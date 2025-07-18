use crate::{
    constants::chains::ChainId,
    models::types::{
        cross_chain::{CrossChainChainSpecificData, CrossChainGenericData},
        user_types::TransferDetails,
    },
};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};

/// Cross chain Limit order intent structure
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CrossChainLimitOrderIntentRequest {
    /// Contains the common data for the intent
    pub generic_data: CrossChainLimitOrderGenericData,
    /// Contains chain-specific data
    pub chain_specific_data: CrossChainChainSpecificData,
}

/// A structure to hold generic data related to cross chain limit order intent
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CrossChainLimitOrderGenericData {
    /// User address initiating the intent
    #[serde(flatten)]
    pub common_data: CrossChainGenericData,
    /// The amount of the input token to be used in the operation
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub amount_in: u128,
}

/// Intent request, received from the user
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CrossChainLimitOrderUserIntentRequest {
    /// Contains the common data for the intent
    pub generic_data: CrossChainLimitOrderGenericRequestData,
    /// Contains chain-specific data
    pub chain_specific_data: CrossChainChainSpecificData,
    /// JSON string of additional execution details
    pub execution_details: String,
}

/// A structure to hold generic data related to the intent
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CrossChainLimitOrderGenericRequestData {
    /// User address initiating the intent
    pub user: String,

    /// Source chain identifier (e.g., Ethereum, Solana)
    pub src_chain_id: ChainId,
    /// The token being spent in the operation (e.g., "ETH", "BTC")
    pub token_in: String,
    /// The amount of the input token to be used in the operation
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub amount_in: u128,
    /// Minimum amount of stablecoins that Tokens IN may be swapped for
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub min_stablecoins_amount: u128,

    /// Deadline for the operation, in Unix timestamp format, in SECONDS
    pub deadline: u64,
    /// SHA-256 hash of `execution_details` JSON String (hex format)
    pub execution_details_hash: String,
}

/// A structure to hold generic data related to the intent
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CrossChainLimitOrderExecutionDetails {
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
}
