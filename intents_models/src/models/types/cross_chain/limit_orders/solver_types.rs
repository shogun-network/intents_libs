use crate::models::types::cross_chain::CrossChainLimitOrderGenericData;
use crate::models::types::cross_chain::CrossChainSolverStartPermission;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
/*********************************************************************/
/**************************** START ORDER ****************************/
/*********************************************************************/
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Permission, granted to Solver to start cross-chain limit order execution
pub struct CrossChainLimitOrderSolverStartPermission {
    #[serde(flatten)]
    pub common_data: CrossChainSolverStartPermission,
    /// Contains the generic order data for the intent
    pub generic_data: CrossChainLimitOrderGenericData,
}
