use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};

use crate::models::types::{
    cross_chain::{CrossChainGenericData, CrossChainLimitOrderGenericRequestData},
    order::OrderStatus,
};

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CrossChainUserLimitOrderResponse {
    /// Unique identifier for the order (intent ID).
    pub order_id: String,

    #[serde(flatten)]
    pub generic_data: CrossChainLimitOrderGenericData,

    pub execution_details: String,

    /// Timestamp when the order was created.
    pub order_creation_time: u64,

    /// Current domain-level status of the order.
    pub order_status: OrderStatus,

    /// Flag to indicate if tokens in were swapped to stablecoins.
    pub tokens_in_were_swapped_to_stablecoins: bool,

    /// Amount of stablecoins swapped from token in
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub locked_stablecoins: u128,

    /// Permit2 nonce, used for the order creation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_fulfillment_timestamp: Option<u64>,

    /// Link to the transaction details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_hash: Option<String>,

    /// The output amount
    #[serde_as(as = "Option<PickFirst<(DisplayFromStr, _)>>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_out: Option<u128>,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CrossChainLimitOrderGenericData {
    /// User address initiating the intent
    #[serde(flatten)]
    pub common_data: CrossChainGenericData,
    /// The amount of the input token to be used in the operation
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub amount_in: u128,
}

impl From<CrossChainLimitOrderGenericData> for CrossChainLimitOrderGenericRequestData {
    fn from(value: CrossChainLimitOrderGenericData) -> Self {
        Self {
            user: value.common_data.user,
            src_chain_id: value.common_data.src_chain_id,
            token_in: value.common_data.token_in,
            amount_in: value.amount_in,
            min_stablecoins_amount: value.common_data.min_stablecoins_amount,
            deadline: value.common_data.deadline,
            execution_details_hash: value.common_data.execution_details_hash,
        }
    }
}
