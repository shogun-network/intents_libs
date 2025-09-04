use crate::constants::chains::ChainId;
use crate::error::{Error, ModelResult};
use crate::models::types::cross_chain::CrossChainSolverStartPermission;
use crate::models::types::cross_chain::{
    CrossChainDcaOrderGenericData, CrossChainDcaOrderIntentRequest,
};
use crate::models::types::user_types::EVMData;
use error_stack::Report;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
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

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Cross chain DCA order data required for execution start
pub struct StartEvmCrossChainDcaOrderData {
    /// Order info struct
    pub order_info: EvmCrossChainDcaOrderInfo,
    /// Start permission struct
    pub start_permission: EvmCrossChainDcaSolverPermission,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EvmCrossChainDcaOrderInfo {
    pub user: String,
    pub token_in: String,
    pub src_chain_id: ChainId,
    pub start_time: u32,
    pub deadline: u32,
    pub total_intervals: u32,
    pub interval_duration: u32,
    #[serde_as(as = "DisplayFromStr")]
    pub amount_in_per_interval: u128,
    #[serde_as(as = "DisplayFromStr")]
    pub min_stablecoins_amount: u128,
    pub execution_details_hash: String,
    pub nonce: String,
}

impl TryFrom<&CrossChainDcaOrderIntentRequest> for EvmCrossChainDcaOrderInfo {
    type Error = Report<Error>;
    fn try_from(intent_request: &CrossChainDcaOrderIntentRequest) -> ModelResult<Self> {
        let generic_intent_data = intent_request.generic_data.clone();
        let evm_data = intent_request.chain_specific_data.try_get_evm()?;

        Self::try_from((&generic_intent_data, evm_data))
    }
}

impl TryFrom<(&CrossChainDcaOrderGenericData, &EVMData)> for EvmCrossChainDcaOrderInfo {
    type Error = Report<Error>;

    fn try_from(
        (generic_intent_data, evm_data): (&CrossChainDcaOrderGenericData, &EVMData),
    ) -> ModelResult<Self> {
        Ok(EvmCrossChainDcaOrderInfo {
            user: generic_intent_data.common_data.user.clone(),
            token_in: generic_intent_data.common_data.token_in.clone(),
            src_chain_id: generic_intent_data.common_data.src_chain_id,
            start_time: generic_intent_data.common_dca_order_data.start_time,
            deadline: generic_intent_data.common_data.deadline as u32,
            total_intervals: generic_intent_data.common_dca_order_data.total_intervals,
            interval_duration: generic_intent_data.common_dca_order_data.interval_duration,
            amount_in_per_interval: generic_intent_data
                .common_dca_order_data
                .amount_in_per_interval,
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
pub struct EvmCrossChainDcaSolverPermission {
    pub solver: String,
    pub order_hash: String,
    pub interval_number_to_execute: u32,
    #[serde_as(as = "DisplayFromStr")]
    pub collateral_amount: u128,
    #[serde_as(as = "DisplayFromStr")]
    pub protocol_fee: u128,
    pub protocol_fee_receiver: String,
    pub allow_swap: bool,
    #[serde_as(as = "DisplayFromStr")]
    pub min_stablecoins_amount: u128,
    pub previous_executed_interval_index: u32,
    pub previous_executed_interval_solver: String,
    pub deadline: u32,
}

/******************************************************************************/
/**************************** SUCCESS CONFIRMATION ****************************/
/******************************************************************************/

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EvmSuccessConfirmationCrossChainDcaOrderData {
    /// Order info that should be passed to contract
    pub order_info: EvmCrossChainDcaOrderInfo,
    /// Success confirmation data that should be passed to contract
    pub success_confirmation_data: serde_json::Value,
}
