use crate::constants::chains::ChainId;
use crate::error::{Error, ModelResult};
use crate::models::types::cross_chain::CrossChainIntentRequest;
use crate::models::types::cross_chain::CrossChainLimitOrderIntentRequest;
use crate::models::types::cross_chain::{CrossChainDcaOrderIntentRequest, CrossChainGenericData};
use crate::models::types::order::OrderType;
use crate::models::types::single_chain::SingleChainLimitOrderIntentRequest;
use crate::models::types::single_chain::{
    SingleChainDcaOrderIntentRequest, SingleChainIntentRequest,
};
use error_stack::report;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
/// Main intent request struct.
pub enum IntentRequest {
    SingleChainLimitOrder(SingleChainLimitOrderIntentRequest),
    SingleChainDcaOrder(SingleChainDcaOrderIntentRequest),
    CrossChainLimitOrder(CrossChainLimitOrderIntentRequest),
    CrossChainDcaOrder(CrossChainDcaOrderIntentRequest),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
/// Main intent request struct. (sorted by chains number)
pub enum IntentRequestChainsNum {
    SingleChain(SingleChainIntentRequest),
    CrossChain(CrossChainIntentRequest),
}

impl IntentRequest {
    pub fn get_order_type(&self) -> OrderType {
        match self {
            IntentRequest::SingleChainLimitOrder(_) => OrderType::SingleChainLimitOrder,
            IntentRequest::SingleChainDcaOrder(_) => OrderType::SingleChainDCAOrder,
            IntentRequest::CrossChainLimitOrder(_) => OrderType::CrossChainLimitOrder,
            IntentRequest::CrossChainDcaOrder(_) => OrderType::CrossChainDCAOrder,
        }
    }
    pub fn try_into_cross_chain(self) -> ModelResult<CrossChainIntentRequest> {
        match self {
            IntentRequest::CrossChainLimitOrder(intent) => {
                Ok(CrossChainIntentRequest::CrossChainLimitOrder(intent))
            }
            IntentRequest::CrossChainDcaOrder(intent) => {
                Ok(CrossChainIntentRequest::CrossChainDcaOrder(intent))
            }
            IntentRequest::SingleChainLimitOrder(_) | IntentRequest::SingleChainDcaOrder(_) => {
                Err(report!(Error::LogicError(
                    "Non-cross-chain intent passed".to_string()
                )))
            }
        }
    }
    pub fn try_into_single_chain(self) -> ModelResult<SingleChainIntentRequest> {
        match self {
            IntentRequest::CrossChainLimitOrder(_) | IntentRequest::CrossChainDcaOrder(_) => {
                Err(report!(Error::LogicError(
                    "Non-single-chain intent passed".to_string()
                )))
            }
            IntentRequest::SingleChainLimitOrder(intent) => {
                Ok(SingleChainIntentRequest::SingleChainLimitOrder(intent))
            }
            IntentRequest::SingleChainDcaOrder(intent) => {
                Ok(SingleChainIntentRequest::SingleChainDcaOrder(intent))
            }
        }
    }
    pub fn into_chains_num(self) -> IntentRequestChainsNum {
        match self {
            IntentRequest::SingleChainLimitOrder(intent) => IntentRequestChainsNum::SingleChain(
                SingleChainIntentRequest::SingleChainLimitOrder(intent),
            ),
            IntentRequest::SingleChainDcaOrder(intent) => IntentRequestChainsNum::SingleChain(
                SingleChainIntentRequest::SingleChainDcaOrder(intent),
            ),
            IntentRequest::CrossChainLimitOrder(intent) => IntentRequestChainsNum::CrossChain(
                CrossChainIntentRequest::CrossChainLimitOrder(intent),
            ),
            IntentRequest::CrossChainDcaOrder(intent) => IntentRequestChainsNum::CrossChain(
                CrossChainIntentRequest::CrossChainDcaOrder(intent),
            ),
        }
    }
    pub fn try_get_cross_chain_common_data(&self) -> ModelResult<&CrossChainGenericData> {
        match self {
            IntentRequest::CrossChainLimitOrder(intent) => Ok(&intent.generic_data.common_data),
            IntentRequest::CrossChainDcaOrder(intent) => Ok(&intent.generic_data.common_data),
            IntentRequest::SingleChainLimitOrder(_) | IntentRequest::SingleChainDcaOrder(_) => {
                Err(report!(Error::LogicError(
                    "Non-cross-chain intent passed".to_string()
                )))
            }
        }
    }
    pub fn get_src_chain(&self) -> ChainId {
        match self {
            IntentRequest::SingleChainLimitOrder(intent) => {
                intent.generic_data.common_data.chain_id
            }
            IntentRequest::SingleChainDcaOrder(intent) => intent.generic_data.common_data.chain_id,
            IntentRequest::CrossChainLimitOrder(intent) => {
                intent.generic_data.common_data.src_chain_id
            }
            IntentRequest::CrossChainDcaOrder(intent) => {
                intent.generic_data.common_data.src_chain_id
            }
        }
    }
    pub fn get_dest_chain(&self) -> ChainId {
        match self {
            IntentRequest::SingleChainLimitOrder(intent) => {
                intent.generic_data.common_data.chain_id
            }
            IntentRequest::SingleChainDcaOrder(intent) => intent.generic_data.common_data.chain_id,
            IntentRequest::CrossChainLimitOrder(intent) => {
                intent.generic_data.common_data.dest_chain_id
            }
            IntentRequest::CrossChainDcaOrder(intent) => {
                intent.generic_data.common_data.dest_chain_id
            }
        }
    }
    /// Total amount of tokens that may be spent during order execution
    pub fn get_total_amount_in(&self) -> u128 {
        match self {
            IntentRequest::SingleChainLimitOrder(intent) => intent.generic_data.amount_in,
            IntentRequest::SingleChainDcaOrder(intent) => {
                intent
                    .generic_data
                    .common_dca_order_data
                    .amount_in_per_interval
                    * intent.generic_data.common_dca_order_data.total_intervals as u128
            }
            IntentRequest::CrossChainLimitOrder(intent) => intent.generic_data.amount_in,
            IntentRequest::CrossChainDcaOrder(intent) => {
                intent
                    .generic_data
                    .common_dca_order_data
                    .amount_in_per_interval
                    * intent.generic_data.common_dca_order_data.total_intervals as u128
            }
        }
    }
    pub fn get_amount_out_min(&self) -> u128 {
        match self {
            IntentRequest::SingleChainLimitOrder(intent) => {
                intent.generic_data.get_amount_out_min()
            }
            IntentRequest::SingleChainDcaOrder(intent) => {
                intent.generic_data.common_data.amount_out_min
            }
            IntentRequest::CrossChainLimitOrder(intent) => intent.generic_data.get_amount_out_min(),
            IntentRequest::CrossChainDcaOrder(intent) => {
                intent.generic_data.common_data.amount_out_min
            }
        }
    }
    pub fn get_user_address(&self) -> &str {
        match self {
            IntentRequest::SingleChainLimitOrder(intent) => &intent.generic_data.common_data.user,
            IntentRequest::SingleChainDcaOrder(intent) => &intent.generic_data.common_data.user,
            IntentRequest::CrossChainLimitOrder(intent) => &intent.generic_data.common_data.user,
            IntentRequest::CrossChainDcaOrder(intent) => &intent.generic_data.common_data.user,
        }
    }
    pub fn get_token_in_address(&self) -> &str {
        match self {
            IntentRequest::SingleChainLimitOrder(intent) => {
                &intent.generic_data.common_data.token_in
            }
            IntentRequest::SingleChainDcaOrder(intent) => &intent.generic_data.common_data.token_in,
            IntentRequest::CrossChainLimitOrder(intent) => {
                &intent.generic_data.common_data.token_in
            }
            IntentRequest::CrossChainDcaOrder(intent) => &intent.generic_data.common_data.token_in,
        }
    }
    pub fn get_token_out_address(&self) -> &str {
        match self {
            IntentRequest::SingleChainLimitOrder(intent) => {
                &intent.generic_data.common_data.token_out
            }
            IntentRequest::SingleChainDcaOrder(intent) => {
                &intent.generic_data.common_data.token_out
            }
            IntentRequest::CrossChainLimitOrder(intent) => {
                &intent.generic_data.common_data.token_out
            }
            IntentRequest::CrossChainDcaOrder(intent) => &intent.generic_data.common_data.token_out,
        }
    }
    pub fn get_deadline(&self) -> u64 {
        match self {
            IntentRequest::SingleChainLimitOrder(intent) => {
                intent.generic_data.common_data.deadline
            }
            IntentRequest::SingleChainDcaOrder(intent) => intent.generic_data.common_data.deadline,
            IntentRequest::CrossChainLimitOrder(intent) => intent.generic_data.common_data.deadline,
            IntentRequest::CrossChainDcaOrder(intent) => intent.generic_data.common_data.deadline,
        }
    }

    /// Some orders can be fulfilled only by matching conditions
    pub fn check_order_can_be_fulfilled(&self) -> ModelResult<()> {
        match self {
            IntentRequest::SingleChainLimitOrder(intent) => intent
                .generic_data
                .common_limit_order_data
                .check_order_can_be_fulfilled(),
            IntentRequest::SingleChainDcaOrder(_) => Ok(()),
            IntentRequest::CrossChainLimitOrder(intent) => intent
                .generic_data
                .common_limit_order_data
                .check_order_can_be_fulfilled(),
            IntentRequest::CrossChainDcaOrder(_) => Ok(()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// EVM-specific data of intent request
pub struct EVMData {
    /// Nonce for Permit2 signature
    pub nonce: String,
    /// Signature for the transaction
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Sui-specific data of intent request
pub struct SuiData {
    /// Transaction hash for the Sui transaction
    pub transaction_hash: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OrderCreatedResponse {
    /// Created intent ID
    pub intent_id: String,
    /// Updated or created JWT for an order user wallet
    pub jwt: String,
}
