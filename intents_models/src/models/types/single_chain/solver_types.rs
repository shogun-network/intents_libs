use crate::models::types::single_chain::{
    SingleChainLimitOrderExecutionDetails, SingleChainOrderExecutionDetails,
};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
/// Set of data to check single chain order execution
pub enum SingleChainSolverExecutionDetailsEnum {
    Limit(SingleChainLimitOrderExecutionDetails),
    // Dca(SingleChainDcaOrderExecutionDetails),
}

impl SingleChainSolverExecutionDetailsEnum {
    pub fn get_common_data(&self) -> &SingleChainOrderExecutionDetails {
        match self {
            SingleChainSolverExecutionDetailsEnum::Limit(details) => &details.common_data,
        }
    }
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Result data of checking single chain order execution
pub struct SingleChainSolverSuccessConfirmation {
    /// Amount of main tokens OUT that were actually received by the user
    #[serde_as(as = "DisplayFromStr")]
    pub amount_out: u128,
    pub tx_timestamp: u64,
}
