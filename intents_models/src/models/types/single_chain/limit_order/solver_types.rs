use crate::models::types::single_chain::SingleChainSolverStartPermission;
use crate::models::types::single_chain::{
    SingleChainLimitOrderGenericData, SingleChainOrderExecutionDetails,
};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

/*********************************************************************/
/**************************** START ORDER ****************************/
/*********************************************************************/
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SingleChainLimitOrderSolverStartPermission {
    #[serde(flatten)]
    pub common_data: SingleChainSolverStartPermission,
    /// Contains the generic order data for the intent
    pub generic_data: SingleChainLimitOrderGenericData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Set of data to check single chain limit order execution
pub struct SingleChainLimitOrderExecutionDetails {
    #[serde(flatten)]
    pub common_data: SingleChainOrderExecutionDetails,
}
