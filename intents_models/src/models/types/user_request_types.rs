use crate::constants::chains::ChainId;
use crate::error::{Error, ModelResult};
use crate::models::types::cross_chain::{
    CrossChainLimitOrderGenericRequestData, CrossChainLimitOrderUserIntentRequest,
};
use crate::models::types::single_chain::{
    SingleChainLimitOrderGenericRequestData, SingleChainLimitOrderUserIntentRequest,
};
use crate::models::types::user_types::IntentRequest;
use error_stack::report;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
/// Intent request, received from the user, but not converted to `IntentRequest` enum yet
/// Main purpose is to pass data which `IntentRequest` doesn't have (like `execution_details`)
pub enum UserIntentRequest {
    SingleChainLimitOrder(SingleChainLimitOrderUserIntentRequest),
    // SingleChainDcaOrder(SingleChainDcaOrderIntentRequest),
    CrossChainLimitOrder(CrossChainLimitOrderUserIntentRequest),
    // CrossChainDcaOrder(CrossChainDcaOrderUserIntentRequest),
}

impl UserIntentRequest {
    pub fn try_into_intent_request(self) -> ModelResult<IntentRequest> {
        Ok(match self {
            UserIntentRequest::SingleChainLimitOrder(intent) => intent.into_into_intent_request(),
            UserIntentRequest::CrossChainLimitOrder(intent) => {
                intent.try_into_into_intent_request()?
            }
        })
    }
    pub fn try_get_cross_chain_execution_details(&self) -> ModelResult<String> {
        match self {
            UserIntentRequest::SingleChainLimitOrder(_) => Err(report!(Error::LogicError(
                "Non-cross-chain data passed".to_string()
            ))),
            UserIntentRequest::CrossChainLimitOrder(intent) => Ok(intent.execution_details.clone()),
        }
    }
}

pub enum GenericData {
    SingleChain(SingleChainLimitOrderGenericRequestData),
    CrossChain(CrossChainLimitOrderGenericRequestData),
}

impl GenericData {
    pub fn get_src_chain_id(&self) -> ChainId {
        match self {
            GenericData::SingleChain(data) => data.common_data.chain_id,
            GenericData::CrossChain(data) => data.src_chain_id,
        }
    }

    pub fn get_token_in(&self) -> String {
        match self {
            GenericData::SingleChain(data) => data.common_data.token_in.clone(),
            GenericData::CrossChain(data) => data.token_in.clone(),
        }
    }

    pub fn set_token_in(&mut self, token_in: String) {
        match self {
            GenericData::SingleChain(data) => data.common_data.token_in = token_in,
            GenericData::CrossChain(data) => data.token_in = token_in,
        }
    }

    pub fn get_amount_in(&self) -> u128 {
        match self {
            GenericData::SingleChain(data) => data.amount_in,
            GenericData::CrossChain(data) => data.amount_in,
        }
    }

    pub fn get_user(&self) -> String {
        match self {
            GenericData::SingleChain(data) => data.common_data.user.clone(),
            GenericData::CrossChain(data) => data.user.clone(),
        }
    }

    pub fn get_deadline(&self) -> u64 {
        match self {
            GenericData::SingleChain(data) => data.common_data.deadline,
            GenericData::CrossChain(data) => data.deadline,
        }
    }
}
