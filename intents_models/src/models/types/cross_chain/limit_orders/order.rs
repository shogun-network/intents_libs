use crate::models::types::cross_chain::CrossChainOnChainOrderData;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Collected on chain cross-chain limit order data about current on chain order state
pub struct CrossChainOnChainLimitOrderData {
    #[serde(flatten)]
    pub common_data: CrossChainOnChainOrderData,
}
