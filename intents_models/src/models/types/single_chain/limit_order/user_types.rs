use crate::models::types::single_chain::{SingleChainChainSpecificData, SingleChainGenericData};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};

/// Single chain Limit order intent structure
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SingleChainLimitOrderIntentRequest {
    /// Contains the common data for the intent
    pub generic_data: SingleChainLimitOrderGenericData,
    /// Contains chain-specific data
    pub chain_specific_data: SingleChainChainSpecificData,
}

/// A structure to hold generic data related to the intent
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SingleChainLimitOrderGenericData {
    /// User address initiating the intent
    #[serde(flatten)]
    pub common_data: SingleChainGenericData,
    /// The amount of the input token to be used in the operation
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub amount_in: u128,
}
