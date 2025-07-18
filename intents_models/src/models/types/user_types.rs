use crate::error::{Error, ModelResult};
use error_stack::report;

use crate::models::types::OrderType;
use crate::models::types::cross_chain::CrossChainGenericData;
use crate::models::types::cross_chain::CrossChainIntentRequest;
use crate::models::types::cross_chain::CrossChainLimitOrderIntentRequest;
use crate::models::types::single_chain::SingleChainIntentRequest;
use crate::models::types::single_chain::SingleChainLimitOrderIntentRequest;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum IntentRequest {
    SingleChainLimitOrder(SingleChainLimitOrderIntentRequest),
    // SingleChainDcaOrder(SingleChainDcaOrderIntentRequest), todo
    CrossChainLimitOrder(CrossChainLimitOrderIntentRequest),
    // CrossChainDcaOrder(CrossChainDcaOrderIntentRequest), todo
}

impl IntentRequest {
    pub fn get_order_type(&self) -> OrderType {
        match self {
            IntentRequest::SingleChainLimitOrder(_) => OrderType::SingleChainLimitOrder,
            IntentRequest::CrossChainLimitOrder(_) => OrderType::CrossChainLimitOrder,
        }
    }
    pub fn try_into_cross_chain(self) -> ModelResult<CrossChainIntentRequest> {
        match self {
            IntentRequest::CrossChainLimitOrder(intent) => {
                Ok(CrossChainIntentRequest::CrossChainLimitOrder(intent))
            }
            IntentRequest::SingleChainLimitOrder(_) => Err(report!(Error::LogicError(
                "Non-cross-chain intent passed".to_string()
            ))),
        }
    }
    pub fn try_get_cross_chain_common_data(&self) -> ModelResult<&CrossChainGenericData> {
        match self {
            IntentRequest::CrossChainLimitOrder(intent) => Ok(&intent.generic_data.common_data),
            IntentRequest::SingleChainLimitOrder(_) => Err(report!(Error::LogicError(
                "Non-cross-chain intent passed".to_string()
            ))),
        }
    }
    pub fn try_into_single_chain(self) -> ModelResult<SingleChainIntentRequest> {
        match self {
            IntentRequest::CrossChainLimitOrder(_) => Err(report!(Error::LogicError(
                "Non-single-chain intent passed".to_string()
            ))),
            IntentRequest::SingleChainLimitOrder(intent) => {
                Ok(SingleChainIntentRequest::SingleChainLimitOrder(intent))
            }
        }
    }
}

/// EVM-specific data
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EVMData {
    /// Nonce for Permit2 signature
    pub nonce: String,
    /// Signature for the transaction
    pub signature: String,
}

/// Sui-specific data
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SuiData {
    /// Transaction hash for the Sui transaction
    pub transaction_hash: String,
}

/// Transfer details with fixed amount
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransferDetails {
    /// Address of token to send
    pub token: String,
    /// Tokens receiver address
    pub receiver: String,
    /// Amount of tokens to send
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub amount: u128,
}
