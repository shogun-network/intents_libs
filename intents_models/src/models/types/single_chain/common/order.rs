use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Collected common on chain single-chain order data about current on chain order state
pub struct SingleChainOnChainOrderData {
    /// Is order still active?
    pub active: bool,
}
