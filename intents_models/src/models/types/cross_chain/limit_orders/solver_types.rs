use crate::models::types::cross_chain::CrossChainLimitOrderGenericData;
use crate::models::types::cross_chain::CrossChainSolverStartPermission;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};
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

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EvmCrossChainLimitOrderInfo {
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub amount_in: u128,
    pub deadline: u32,
    pub execution_details_hash: String,
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub min_stablecoins_amount: u128,
    pub nonce: String,
    pub src_chain_id: u64,
    pub token_in: String,
    pub user: String,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EvmCrossChainLimitSolverPermission {

}
struct SourceChainSolverPermission { address solver; bytes32 orderHash; uint128 collateralAmount; uint128 protocolFee; bool allowSwap; uint128 minStablecoinsAmount; uint32 deadline; }