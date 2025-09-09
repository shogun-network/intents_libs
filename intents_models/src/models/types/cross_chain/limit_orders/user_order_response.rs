use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};

use crate::models::types::common::DcaIntervalExecutionResponse;
use crate::models::types::cross_chain::CrossChainLimitOrderGenericData;
use crate::models::types::order::OrderStatus;

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Cross chain limit order data, provided to user on request
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
