use crate::models::types::single_chain::SingleChainSolverStartPermission;
use crate::models::types::single_chain::{
    SingleChainDcaOrderGenericData, SingleChainOrderExecutionDetails,
};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

/*********************************************************************/
/**************************** START ORDER ****************************/
/*********************************************************************/

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Permission, granted to Solver to start single-chain DCA order execution
pub struct SingleChainDcaOrderSolverStartPermission {
    #[serde(flatten)]
    pub common_data: SingleChainSolverStartPermission,
    /// Contains the common data for the intent
    pub generic_data: SingleChainDcaOrderGenericData,
    /// Interval number to execute
    pub interval_number: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Set of data to check single chain DCA order execution
pub struct SingleChainDcaOrderExecutionDetails {
    #[serde(flatten)]
    pub common_data: SingleChainOrderExecutionDetails,
    pub interval_number: u32,
}
