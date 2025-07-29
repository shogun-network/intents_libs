use crate::models::types::common::CommonDcaOrderState;
use crate::models::types::single_chain::SingleChainOnChainOrderData;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SingleChainOnChainDcaOrderData {
    #[serde(flatten)]
    pub common_data: SingleChainOnChainOrderData,
    /// Common DCA order state
    #[serde(flatten)]
    pub common_dca_state: CommonDcaOrderState,
}
