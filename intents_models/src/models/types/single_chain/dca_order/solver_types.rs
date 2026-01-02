use crate::constants::chains::{EVM_NULL_ADDRESS, is_native_token_evm_address};
use crate::error::{Error, ModelResult};
use crate::models::types::common::TransferDetails;
use crate::models::types::single_chain::{
    SingleChainDcaOrderGenericData, SingleChainOrderExecutionDetails,
};
use crate::models::types::single_chain::{
    SingleChainDcaOrderIntentRequest, SingleChainSolverStartPermission,
};
use crate::models::types::user_types::EVMData;
use error_stack::Report;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};
/*********************************************************************/
/**************************** START ORDER ****************************/
/*********************************************************************/

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Single chain DCA order data required for execution start
pub struct StartEvmSingleChainDcaOrderData {
    pub order_info: EvmSingleChainDcaOrderInfo,
    pub start_permission: EvmSingleChainDcaSolverPermission,
}

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
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EvmSingleChainDcaOrderInfo {
    pub user: String,
    pub token_in: String,
    pub start_time: u32,
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub amount_in_per_interval: u128,
    pub total_intervals: u32,
    pub interval_duration: u32,
    pub requested_output: TransferDetails,
    pub extra_transfers: Vec<TransferDetails>,
    pub encoded_external_call_data: String,
    pub deadline: u32,
    pub nonce: String,
}

impl TryFrom<&SingleChainDcaOrderIntentRequest> for EvmSingleChainDcaOrderInfo {
    type Error = Report<Error>;

    fn try_from(intent_request: &SingleChainDcaOrderIntentRequest) -> ModelResult<Self> {
        let evm_data = intent_request.chain_specific_data.try_get_evm()?;
        Self::try_from((&intent_request.generic_data, evm_data))
    }
}

impl TryFrom<(&SingleChainDcaOrderGenericData, &EVMData)> for EvmSingleChainDcaOrderInfo {
    type Error = Report<Error>;

    fn try_from(
        (generic_intent_data, evm_data): (&SingleChainDcaOrderGenericData, &EVMData),
    ) -> ModelResult<Self> {
        let requested_output = TransferDetails {
            amount: generic_intent_data.common_data.amount_out_min,
            token: if is_native_token_evm_address(&generic_intent_data.common_data.token_out) {
                EVM_NULL_ADDRESS.to_owned()
            } else {
                generic_intent_data.common_data.token_out.clone()
            },
            receiver: generic_intent_data.common_data.destination_address.clone(),
        };

        let extra_transfers = generic_intent_data
            .common_data
            .extra_transfers
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|t| TransferDetails {
                token: if is_native_token_evm_address(&t.token) {
                    EVM_NULL_ADDRESS.to_owned()
                } else {
                    t.token
                },
                ..t
            })
            .collect();

        let order = EvmSingleChainDcaOrderInfo {
            user: generic_intent_data.common_data.user.clone(),
            token_in: generic_intent_data.common_data.token_in.clone(),
            start_time: generic_intent_data.common_dca_order_data.start_time,
            amount_in_per_interval: generic_intent_data
                .common_dca_order_data
                .amount_in_per_interval,
            total_intervals: generic_intent_data.common_dca_order_data.total_intervals,
            interval_duration: generic_intent_data.common_dca_order_data.interval_duration,
            requested_output,
            extra_transfers,
            encoded_external_call_data: "0x".to_string(), // Empty bytes, external calls will be implemented in the future
            deadline: u32::try_from(generic_intent_data.common_data.deadline)
                .map_err(|_| Error::ParseError)?,
            nonce: evm_data.nonce.clone(),
        };

        Ok(order)
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EvmSingleChainDcaSolverPermission {
    pub solver: String,
    pub order_hash: String,
    pub interval_number_to_execute: u32,
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub amount_out_min: u128,
    pub protocol_fee_transfer: TransferDetails,
    pub permission_deadline: u32,
}
