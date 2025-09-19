use crate::models::types::common::CommonDcaOrderState;
use crate::models::types::cross_chain::CrossChainOnChainOrderData;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CrossChainOnChainDcaOrderData {
    #[serde(flatten)]
    pub common_data: CrossChainOnChainOrderData,
    /// Common DCA order state
    #[serde(flatten)]
    pub common_dca_state: CommonDcaOrderState,
    /// Interval INDEX when latest order execution has started
    pub latest_execution_start: ExecutionStart,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ExecutionStart {
    TimestampSeconds(u32),
    IntervalIndex(u32)
}