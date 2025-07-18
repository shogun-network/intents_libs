use crate::models::types::single_chain::{SingleChainChainSpecificData, SingleChainGenericData};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};

/// Single chain DCA order intent structure
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SingleChainDcaOrderIntentRequest {
    /// Contains the common data for the intent
    pub generic_data: SingleChainDcaOrderGenericData,
    /// Contains chain-specific data
    pub chain_specific_data: SingleChainChainSpecificData,
}

/// A structure to hold generic data related to the intent
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SingleChainDcaOrderGenericData {
    /// User address initiating the intent
    #[serde(flatten)]
    pub common_data: SingleChainGenericData,
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
