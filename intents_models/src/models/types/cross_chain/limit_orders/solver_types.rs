use crate::constants::chains::ChainId;
use crate::error::Error;
use crate::error::ModelResult;
use crate::models::types::cross_chain::CrossChainLimitOrderGenericData;
use crate::models::types::cross_chain::CrossChainLimitOrderIntentRequest;
use crate::models::types::cross_chain::CrossChainSolverStartPermission;
use crate::models::types::user_types::EVMData;
use error_stack::Report;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};
/*********************************************************************/
/**************************** START ORDER ****************************/
/*********************************************************************/

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Cross chain limit order data required for execution start
pub struct StartEvmCrossChainLimitOrderData {
    pub order_info: EvmCrossChainLimitOrderInfo,
    pub start_permission: EvmCrossChainLimitSolverPermission,
}

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
    pub src_chain_id: ChainId,
    pub token_in: String,
    pub user: String,
}

impl TryFrom<&CrossChainLimitOrderIntentRequest> for EvmCrossChainLimitOrderInfo {
    type Error = Report<Error>;
    fn try_from(intent_request: &CrossChainLimitOrderIntentRequest) -> ModelResult<Self> {
        let generic_intent_data = intent_request.generic_data.clone();
        let evm_data = intent_request.chain_specific_data.try_get_evm()?;

        Ok(EvmCrossChainLimitOrderInfo {
            user: generic_intent_data.common_data.user.clone(),
            token_in: generic_intent_data.common_data.token_in.clone(),
            src_chain_id: generic_intent_data.common_data.src_chain_id,
            deadline: generic_intent_data.common_data.deadline as u32,
            amount_in: generic_intent_data.amount_in,
            min_stablecoins_amount: generic_intent_data.common_data.min_stablecoins_amount,
            execution_details_hash: generic_intent_data
                .common_data
                .execution_details_hash
                .clone(),
            nonce: evm_data.nonce.clone(),
        })
    }
}

impl TryFrom<(&CrossChainLimitOrderGenericData, &EVMData)> for EvmCrossChainLimitOrderInfo {
    type Error = Report<Error>;

    fn try_from(
        (generic_intent_data, evm_data): (&CrossChainLimitOrderGenericData, &EVMData),
    ) -> ModelResult<Self> {
        Ok(EvmCrossChainLimitOrderInfo {
            user: generic_intent_data.common_data.user.clone(),
            token_in: generic_intent_data.common_data.token_in.clone(),
            src_chain_id: generic_intent_data.common_data.src_chain_id,
            deadline: generic_intent_data.common_data.deadline as u32,
            amount_in: generic_intent_data.amount_in,
            min_stablecoins_amount: generic_intent_data.common_data.min_stablecoins_amount,
            execution_details_hash: generic_intent_data
                .common_data
                .execution_details_hash
                .clone(),
            nonce: evm_data.nonce.clone(),
        })
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EvmCrossChainLimitSolverPermission {
    pub solver: String,
    pub order_hash: String,
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub collateral_amount: u128,
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub protocol_fee: u128,
    pub allow_swap: bool,
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub min_stablecoins_amount: u128,
    pub deadline: u32,
}


/******************************************************************************/
/**************************** SUCCESS CONFIRMATION ****************************/
/******************************************************************************/

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EvmSuccessConfirmationCrossChainLimitOrderData {
    /// Order info that should be passed to contract
    pub order_info: EvmCrossChainLimitOrderInfo,
    /// Success confirmation data that should be passed to contract
    pub success_confirmation_data: serde_json::Value,
}