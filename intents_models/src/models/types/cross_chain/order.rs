use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
/// List of transaction hashes, provided by the Solver to Auctioneer after fulfillment of cross chain order
pub struct FulfillmentTxHashes {
    /// Transaction hash of main order fulfillment
    pub main_tx_hash: String,
    /// Transaction hashes array. Each for requested extra transfers
    pub extra_transfers_tx_hashes: Option<Vec<String>>,
}
