use crate::models::types::single_chain::SingleChainOnChainOrderData;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SingleChainOnChainDcaOrderData {
    #[serde(flatten)]
    pub common_data: SingleChainOnChainOrderData,
    /// Total number of already executed intervals
    pub total_executed_intervals: u32,
    /// INDEX of last executed interval
    pub last_executed_interval_index: u32,
}
