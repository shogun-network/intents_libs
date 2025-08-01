use crate::models::types::single_chain::SingleChainSolverStartPermission;
use crate::models::types::single_chain::{
    SingleChainLimitOrderGenericData, SingleChainOrderExecutionDetails,
};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};

/*********************************************************************/
/**************************** START ORDER ****************************/
/*********************************************************************/
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Permission, granted to Solver to start single-chain limit order execution
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

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EvmSingleChainLimitOrderInfo {
    pub user: String,
    pub token_in: String,
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub amount_in: u128,
    pub requested_output: TransferData,
    pub extra_transfers: Vec<TransferData>,
    pub encoded_external_call_data: String,
    pub deadline: u32,
    pub nonce: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransferData {
    pub token: String,
    pub receiver: String,
    pub amount: String,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EvmSingleChainLimitSolverPermission {}

struct SingleChainLimitSolverPermission { address solver; bytes32 orderHash; uint256 amountOutMin; TransferData protocolFeeTransfer; uint32 permissionDeadline; }