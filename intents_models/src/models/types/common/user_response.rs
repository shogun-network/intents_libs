use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// DCA interval execution data, provided to user on request
pub struct DcaIntervalExecutionResponse {
    /// Fulfilled interval number
    pub interval_number: u32,
    /// Timestamp of DCA interval fulfillment
    pub interval_fulfilled_timestamp: u32,
    /// Fulfillment transaction hash
    pub transaction_hash: String,
    /// Received amount OUT
    pub amount_out: u128,
}
