use crate::models::types::common::CommonLimitOrderData;
use crate::models::types::cross_chain::{CrossChainChainSpecificData, CrossChainGenericData};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Cross chain Limit order intent structure
pub struct CrossChainLimitOrderIntentRequest {
    /// Contains the common data for the intent
    pub generic_data: CrossChainLimitOrderGenericData,
    /// Contains chain-specific data
    pub chain_specific_data: CrossChainChainSpecificData,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Generic data related to cross chain limit order intent
pub struct CrossChainLimitOrderGenericData {
    /// User address initiating the intent
    #[serde(flatten)]
    pub common_data: CrossChainGenericData,
    /// Common limit order data to trigger "take profit" or "stop loss" execution
    #[serde(flatten)]
    pub common_limit_order_data: CommonLimitOrderData,
    /// The amount of the input token to be used in the operation
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub amount_in: u128,
}

impl CrossChainLimitOrderGenericData {
    pub fn get_amount_out_min(&self) -> u128 {
        self.common_limit_order_data
            .get_amount_out_min(self.common_data.amount_out_min)
    }
}
