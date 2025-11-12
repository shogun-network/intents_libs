use crate::constants::chains::ChainId;
use crate::error::{Error, ModelResult};
use crate::models::types::cross_chain::{
    CrossChainDcaOrderUserIntentRequest, CrossChainLimitOrderGenericRequestData,
    CrossChainLimitOrderUserIntentRequest,
};
use crate::models::types::single_chain::{
    SingleChainDcaOrderUserIntentRequest, SingleChainLimitOrderGenericRequestData,
    SingleChainLimitOrderUserIntentRequest,
};
use crate::models::types::user_types::IntentRequest;
use error_stack::report;
use serde::{Deserialize, Serialize};
use serde_with::{StringWithSeparator, formats::CommaSeparator, serde_as};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
/// Intent request, received from the user, but not converted to `IntentRequest` enum yet
/// Main purpose is to pass data which `IntentRequest` doesn't have (like `execution_details`)
pub enum UserIntentRequest {
    SingleChainLimitOrder(SingleChainLimitOrderUserIntentRequest),
    SingleChainDcaOrder(SingleChainDcaOrderUserIntentRequest),
    CrossChainLimitOrder(CrossChainLimitOrderUserIntentRequest),
    CrossChainDcaOrder(CrossChainDcaOrderUserIntentRequest),
}

impl UserIntentRequest {
    pub fn try_into_intent_request(self) -> ModelResult<IntentRequest> {
        Ok(match self {
            UserIntentRequest::SingleChainLimitOrder(intent) => intent.into_into_intent_request(),
            UserIntentRequest::SingleChainDcaOrder(intent) => intent.into_into_intent_request(),
            UserIntentRequest::CrossChainLimitOrder(intent) => {
                intent.try_into_into_intent_request()?
            }
            UserIntentRequest::CrossChainDcaOrder(intent) => {
                intent.try_into_into_intent_request()?
            }
        })
    }
    pub fn try_get_cross_chain_execution_details(&self) -> ModelResult<String> {
        match self {
            UserIntentRequest::SingleChainLimitOrder(_)
            | UserIntentRequest::SingleChainDcaOrder(_) => Err(report!(Error::LogicError(
                "Non-cross-chain data passed".to_string()
            ))),
            UserIntentRequest::CrossChainLimitOrder(intent) => Ok(intent.execution_details.clone()),
            UserIntentRequest::CrossChainDcaOrder(intent) => Ok(intent.execution_details.clone()),
        }
    }
}

/// Generic data of request struct, received by the user
pub enum UserRequestGenericData {
    SingleChain(SingleChainLimitOrderGenericRequestData),
    CrossChain(CrossChainLimitOrderGenericRequestData),
}

impl UserRequestGenericData {
    pub fn get_src_chain_id(&self) -> ChainId {
        match self {
            UserRequestGenericData::SingleChain(data) => data.common_data.chain_id,
            UserRequestGenericData::CrossChain(data) => data.src_chain_id,
        }
    }

    pub fn get_token_in(&self) -> String {
        match self {
            UserRequestGenericData::SingleChain(data) => data.common_data.token_in.clone(),
            UserRequestGenericData::CrossChain(data) => data.token_in.clone(),
        }
    }

    pub fn set_token_in(&mut self, token_in: String) {
        match self {
            UserRequestGenericData::SingleChain(data) => data.common_data.token_in = token_in,
            UserRequestGenericData::CrossChain(data) => data.token_in = token_in,
        }
    }

    pub fn get_amount_in(&self) -> u128 {
        match self {
            UserRequestGenericData::SingleChain(data) => data.amount_in,
            UserRequestGenericData::CrossChain(data) => data.amount_in,
        }
    }

    pub fn get_user(&self) -> String {
        match self {
            UserRequestGenericData::SingleChain(data) => data.common_data.user.clone(),
            UserRequestGenericData::CrossChain(data) => data.user.clone(),
        }
    }

    pub fn get_deadline(&self) -> u64 {
        match self {
            UserRequestGenericData::SingleChain(data) => data.common_data.deadline,
            UserRequestGenericData::CrossChain(data) => data.deadline,
        }
    }
}

#[serde_as]
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetUserIntentsRequest {
    #[serde_as(as = "StringWithSeparator<CommaSeparator, String>")]
    #[serde(default)]
    pub evm_wallets: Vec<String>,
    #[serde_as(as = "StringWithSeparator<CommaSeparator, String>")]
    #[serde(default)]
    pub solana_wallets: Vec<String>,
    #[serde_as(as = "StringWithSeparator<CommaSeparator, String>")]
    #[serde(default)]
    pub sui_wallets: Vec<String>,
}
