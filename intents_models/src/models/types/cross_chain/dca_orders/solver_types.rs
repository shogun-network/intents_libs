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
/// Permission, granted to Solver to start cross-chain DCA order execution
pub struct CrossChainDcaOrderSolverStartPermission {
    #[serde(flatten)]
    pub common_data: CrossChainSolverStartPermission,
    /// Contains the common data for the intent
    pub generic_data: CrossChainDcaOrderGenericData,
    /// Interval number to execute
    pub interval_number: u32,
    /// INDEX of last interval that was successfully executed
    pub previous_executed_interval_index: u32,
    /// Address of the Solver that successfully executed interval with `previous_executed_interval_index` INDEX
    /// None if there was no successful execution yet
    pub previous_executed_interval_solver: Option<String>,
}
