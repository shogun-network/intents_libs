use crate::error::{Error, ModelResult};
use crate::models::types::common::TransferDetails;
use crate::models::types::single_chain::{
    SingleChainLimitOrderGenericData, SingleChainOrderExecutionDetails,
};
use crate::models::types::single_chain::{
    SingleChainLimitOrderIntentRequest, SingleChainSolverStartPermission,
};
use crate::models::types::user_types::EVMData;
use error_stack::Report;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};

/*********************************************************************/
/**************************** START ORDER ****************************/
/*********************************************************************/

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StartEvmSingleChainLimitOrderData {
    order_info: EvmSingleChainLimitOrderInfo,
    start_permission: EvmSingleChainLimitSolverPermission,
}

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

impl TryFrom<&SingleChainLimitOrderIntentRequest> for EvmSingleChainLimitOrderInfo {
    type Error = Report<Error>;

    fn try_from(intent_request: &SingleChainLimitOrderIntentRequest) -> ModelResult<Self> {
        let evm_data = intent_request.chain_specific_data.try_get_evm()?;
        Self::try_from((&intent_request.generic_data, evm_data))
    }
}

impl TryFrom<(&SingleChainLimitOrderGenericData, &EVMData)> for EvmSingleChainLimitOrderInfo {
    type Error = Report<Error>;

    fn try_from(
        (generic_intent_data, evm_data): (&SingleChainLimitOrderGenericData, &EVMData),
    ) -> ModelResult<Self> {
        let requested_output = TransferData {
            amount: generic_intent_data.common_data.amount_out_min.to_string(),
            token: generic_intent_data.common_data.token_out.clone(),
            receiver: generic_intent_data.common_data.destination_address.clone(),
        };

        let extra_transfers = match generic_intent_data.common_data.extra_transfers.as_ref() {
            Some(transfers) => transfers
                .iter()
                .map(|transfer| TryInto::<TransferData>::try_into(transfer.clone()))
                .collect::<ModelResult<Vec<TransferData>>>()?,
            None => vec![],
        };

        let order = EvmSingleChainLimitOrderInfo {
            user: generic_intent_data.common_data.user.clone(),
            token_in: generic_intent_data.common_data.token_in.clone(),
            amount_in: generic_intent_data.amount_in,
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

impl TryFrom<TransferDetails> for TransferData {
    type Error = Report<Error>;

    fn try_from(transfer_details: TransferDetails) -> ModelResult<Self> {
        Ok(Self {
            amount: transfer_details.amount.to_string(),
            token: transfer_details.token.clone(),
            receiver: transfer_details.receiver.clone(),
        })
    }
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
pub struct EvmSingleChainLimitSolverPermission {
    pub solver: String,
    pub order_hash: String,
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub amount_out_min: u128,
    pub protocol_fee_transfer: TransferData,
    pub permission_deadline: u32,
}
