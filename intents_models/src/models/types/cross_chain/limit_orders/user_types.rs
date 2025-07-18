use crate::models::types::cross_chain::{CrossChainChainSpecificData, CrossChainGenericData};
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
