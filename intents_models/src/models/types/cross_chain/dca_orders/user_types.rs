use crate::models::types::common::{CommonDcaOrderData, CommonDcaOrderState};
use crate::models::types::cross_chain::{CrossChainChainSpecificData, CrossChainGenericData};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

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
    /// Common DCA order data
    #[serde(flatten)]
    pub common_dca_order_data: CommonDcaOrderData,
    /// Common DCA order state
    #[serde(flatten)]
    pub common_dca_state: CommonDcaOrderState,

    /// INDEX of last interval that was successfully executed
    ///
    /// 0 if there was no successful execution yet
    pub previous_executed_interval_index: u32,

    /// Address of the Solver that successfully executed interval with `previous_executed_interval_index` INDEX
    /// None if there was no successful execution yet
    pub previous_executed_interval_solver: Option<String>,
}
