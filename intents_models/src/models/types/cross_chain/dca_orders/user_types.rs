use crate::models::types::cross_chain::{CrossChainChainSpecificData, CrossChainGenericData};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Cross chain DCA order intent structure
pub struct CrossChainDcaOrderIntentRequest {
    /// Contains the common data for the intent
    pub generic_data: CrossChainDcaOrderGenericData,
    /// Contains chain-specific data
    pub chain_specific_data: CrossChainChainSpecificData,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// A structure to hold generic data related to cross chain DCA order intent
pub struct CrossChainDcaOrderGenericData {
    /// User address initiating the intent
    #[serde(flatten)]
    pub common_data: CrossChainGenericData,
    /// Timestamp (in seconds) when the user created and submitted the DCA order
    pub start_time: u32,
    /// Amount of tokens IN user is willing to spend per interval/trade
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub amount_in_per_interval: u128,
    /// Total number of intervals over which the DCA order will be executed
    pub total_intervals: u32,
    /// DCA interval duration, in seconds
    pub interval_duration: u32,
}
