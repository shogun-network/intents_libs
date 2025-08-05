use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use crate::models::types::common::TransferDetails;

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Requested EVM fulfillment data
pub struct EvmCrossChainFulfillmentData {
    /// Requested fulfillment data
    pub requested_fulfillment: EvmCrossChainRequestedFulfillment,
    /// Auctioneer signature used to fulfill order on destination chain
    pub destination_chain_auctioneer_signature: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Requested EVM fulfillment data
pub enum EvmCrossChainRequestedFulfillment {
    SimpleFulfillment(SimpleEvmRequestedFulfillment),
    // FulfillmentWithExternalCall(), // todo
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Requested fulfillment data (without external call)
pub struct SimpleEvmRequestedFulfillment {
    /// Order ID
    pub order_id: String,
    /// Fulfillment deadline, in seconds
    pub deadline: u64,
    /// Main token address. address(0) for native token
    pub token: String,
    /// Main token destination address
    pub receiver: String,
    /// Main token amount
    #[serde_as(as = "DisplayFromStr")]
    pub requested_amount: u128,

    /// Array of requested extra transfers
    pub extra_transfers: Vec<TransferDetails>,
}