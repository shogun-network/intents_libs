use crate::models::types::common::CommonDcaOrderState;
use crate::models::types::cross_chain::CrossChainOnChainOrderData;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CrossChainOnChainDcaOrderData {
    #[serde(flatten)]
    pub common_data: CrossChainOnChainOrderData,
    /// Common DCA order state
    #[serde(flatten)]
    pub common_dca_state: CommonDcaOrderState,
    // todo previous interval? solver? etc?
}
