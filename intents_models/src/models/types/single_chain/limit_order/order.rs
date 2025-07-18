use crate::models::types::single_chain::SingleChainOnChainOrderData;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Collected on chain single-chain limit order data about current on chain order state
pub struct SingleChainOnChainLimitOrderData {
    #[serde(flatten)]
    pub common_data: SingleChainOnChainOrderData,
}
