use crate::models::types::common::{CommonDcaOrderData, CommonDcaOrderState};
use crate::models::types::single_chain::{SingleChainChainSpecificData, SingleChainGenericData};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Single chain DCA order intent structure
pub struct SingleChainDcaOrderIntentRequest {
    /// Contains the common data for the intent
    pub generic_data: SingleChainDcaOrderGenericData,
    /// Contains chain-specific data
    pub chain_specific_data: SingleChainChainSpecificData,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Generic data related to the single-chain DCA order
pub struct SingleChainDcaOrderGenericData {
    /// User address initiating the intent
    #[serde(flatten)]
    pub common_data: SingleChainGenericData,
    /// Common DCA order data
    #[serde(flatten)]
    pub common_dca_order_data: CommonDcaOrderData,
    /// Common DCA order state
    #[serde(flatten)]
    pub common_dca_state: CommonDcaOrderState,
}
