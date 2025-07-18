use crate::models::types::cross_chain::CrossChainDcaOrderGenericData;
use crate::models::types::cross_chain::CrossChainSolverStartPermission;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
/*********************************************************************/
/**************************** START ORDER ****************************/
/*********************************************************************/

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CrossChainDcaOrderSolverStartPermission {
    #[serde(flatten)]
    pub common_data: CrossChainSolverStartPermission,
    /// Contains the common data for the intent
    pub generic_data: CrossChainDcaOrderGenericData,
    /// Interval number to execute
    pub interval_number: u32,
}
