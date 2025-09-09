use crate::constants::chains::ChainId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
/// Enum that has all possible variants of execution search requests
pub enum ExecutionSearchRequest {
    SingleChainDca(DcaIntervalExecutionSearchRequest),
}

impl ExecutionSearchRequest {
    pub fn get_chain_id(&self) -> ChainId {
        match self {
            ExecutionSearchRequest::SingleChainDca(request) => request.chain_id,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Request to search fo DCA interval execution
pub struct DcaIntervalExecutionSearchRequest {
    /// Chain ID where order interval execution should be fulfilled
    pub chain_id: ChainId,
    /// Order unique identifier
    pub order_id: String,
    /// Interval number to search execution for
    pub interval_number: u32,
    /// Start timestamp of solver's permission.
    pub permission_start_timestamp: u64,
    /// End timestamp of solver's permission
    pub permission_end_timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Collected order execution data
pub struct OrderExecutionData {
    /// Chain ID where execution was fulfilled
    pub chain_id: ChainId,
    /// Order unique identifier
    pub order_id: String,
    /// Fulfillment transaction hash
    pub tx_hash: String,
    /// Main token amount OUT
    pub amount_out: u128,
    /// Transaction timestamp, in seconds
    pub tx_timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
/// Fulfillment data for a specific order type
pub enum OrderTypeFulfillmentData {
    /// Limit order (no extra data).
    Limit,
    /// DCA order fulfillment details.
    Dca(DcaOrderFulfillmentData),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// DCA order fulfillment details.
pub struct DcaOrderFulfillmentData {
    /// Fulfilled interval number
    pub interval_number: u32,
}
