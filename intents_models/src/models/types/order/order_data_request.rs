use crate::constants::chains::ChainId;
use crate::models::types::cross_chain::{
    CrossChainChainSpecificData, CrossChainIntentRequest, CrossChainLimitOrderIntentRequest,
};
use crate::models::types::order::OrderType;
use crate::models::types::single_chain::{SingleChainChainSpecificData, SingleChainIntentRequest, SingleChainLimitOrderIntentRequest, SingleChainUserLimitOrderResponse};
use crate::models::types::user_types::IntentRequest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Request for on chain order data
pub struct OnChainOrderDataRequest {
    pub order_id: String,
    pub chain_id: ChainId,
    pub order_type: OrderType,
    pub chain_data: OnChainOrderDataRequestChainData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Extra data required for on chain order data collection
pub enum OnChainOrderDataRequestChainData {
    EVM { user_address: String, nonce: String },
    Solana,
    Sui,
}

impl OnChainOrderDataRequest {
    pub fn new_from_intent_request(order_id: String, intent_request: &IntentRequest) -> Self {
        Self {
            order_id,
            chain_id: intent_request.get_src_chain(),
            order_type: intent_request.get_order_type(),
            chain_data: OnChainOrderDataRequestChainData::from(intent_request),
        }
    }
    pub fn new_from_single_chain_intent_request(
        order_id: String,
        intent_request: &SingleChainIntentRequest,
    ) -> Self {
        Self {
            order_id,
            chain_id: intent_request.get_chain_id(),
            order_type: intent_request.get_order_type(),
            chain_data: OnChainOrderDataRequestChainData::from(intent_request),
        }
    }
    pub fn new_from_cross_chain_intent_request(
        order_id: String,
        intent_request: &CrossChainIntentRequest,
    ) -> Self {
        Self {
            order_id,
            chain_id: intent_request.get_src_chain(),
            order_type: intent_request.get_order_type(),
            chain_data: OnChainOrderDataRequestChainData::from(intent_request),
        }
    }
}

impl From<&IntentRequest> for OnChainOrderDataRequestChainData {
    fn from(intent: &IntentRequest) -> Self {
        match intent {
            IntentRequest::SingleChainLimitOrder(i) => Self::from(i),
            IntentRequest::CrossChainLimitOrder(i) => Self::from(i),
        }
    }
}

impl From<&SingleChainIntentRequest> for OnChainOrderDataRequestChainData {
    fn from(intent: &SingleChainIntentRequest) -> Self {
        match intent {
            SingleChainIntentRequest::SingleChainLimitOrder(i) => Self::from(i),
        }
    }
}

impl From<&CrossChainIntentRequest> for OnChainOrderDataRequestChainData {
    fn from(intent: &CrossChainIntentRequest) -> Self {
        match intent {
            CrossChainIntentRequest::CrossChainLimitOrder(i) => Self::from(i),
        }
    }
}

impl From<&SingleChainLimitOrderIntentRequest> for OnChainOrderDataRequestChainData {
    fn from(intent: &SingleChainLimitOrderIntentRequest) -> Self {
        Self::from_single_chain_values(
            &intent.chain_specific_data,
            intent.generic_data.common_data.user.clone(),
        )
    }
}

impl From<&CrossChainLimitOrderIntentRequest> for OnChainOrderDataRequestChainData {
    fn from(intent: &CrossChainLimitOrderIntentRequest) -> Self {
        Self::from_cross_chain_values(
            &intent.chain_specific_data,
            intent.generic_data.common_data.user.clone(),
        )
    }
}

impl OnChainOrderDataRequestChainData {
    fn from_single_chain_values(
        chain_specific_data: &SingleChainChainSpecificData,
        user_address: String,
    ) -> Self {
        match chain_specific_data {
            SingleChainChainSpecificData::EVM(data) => Self::EVM {
                user_address,
                nonce: data.nonce.clone(),
            },
            SingleChainChainSpecificData::Sui(_) => Self::Sui,
            SingleChainChainSpecificData::Solana(_) => Self::Solana,
        }
    }
    fn from_cross_chain_values(
        chain_specific_data: &CrossChainChainSpecificData,
        user_address: String,
    ) -> Self {
        match chain_specific_data {
            CrossChainChainSpecificData::EVM(data) => Self::EVM {
                user_address,
                nonce: data.nonce.clone(),
            },
            CrossChainChainSpecificData::Sui(_) => Self::Sui,
            CrossChainChainSpecificData::Solana(_) => Self::Solana,
        }
    }
}
