use crate::models::types::common::{EvmCallMode, TransferDetails};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Requested EVM fulfillment data
pub struct EvmCrossChainFulfillmentData {
    /// Destination chain guard address
    pub dest_chain_guard_address: String,
    /// Requested fulfillment data
    pub requested_fulfillment: EvmCrossChainRequestedFulfillment,
    /// Auctioneer signature used to fulfill order on destination chain
    pub destination_chain_auctioneer_signature: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Requested EVM fulfillment data
pub enum EvmCrossChainRequestedFulfillment {
    SimpleFulfillment(SimpleEvmRequestedFulfillment),
    FulfillmentWithExternalCall(EvmRequestedFulfillmentWithExternalCall),
}

impl EvmCrossChainRequestedFulfillment {
    pub fn get_intent_id(&self) -> String {
        match &self {
            EvmCrossChainRequestedFulfillment::SimpleFulfillment(data) => data.order_id.to_owned(),
            EvmCrossChainRequestedFulfillment::FulfillmentWithExternalCall(data) => {
                data.order_id.to_owned()
            }
        }
    }
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

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Requested fulfillment data (with external call)
pub struct EvmRequestedFulfillmentWithExternalCall {
    /// Order ID
    pub order_id: String,
    /// Fulfillment deadline, in seconds
    pub deadline: u64,
    /// Main token address. address(0) for native token
    pub token: String,
    /// Main token destination address
    /// For `CallMode.ApproveAndCall` tokens are approved to this address
    /// For `CallMode.TransferAndCall` tokens are transferred to this address
    pub token_destination: String,
    /// Main token amount
    /// For `CallMode.ApproveAndCall` this is minimum approval amount
    /// For `CallMode.TransferAndCall` this is minimum transfer amount
    #[serde_as(as = "DisplayFromStr")]
    pub requested_amount: u128,

    /// Contract address that must be called
    pub call_target: String,
    /// Call data of requested call to `callTarget`
    pub call_data: String,
    /// Requested call mode
    pub call_mode: EvmCallMode,

    /// In case of failed call, tokens will be sent to `fallbackAddress`
    /// All remaining main tokens and tokens from extra transfers will also be sent to `fallbackAddress`
    pub fallback_address: String,

    /// Array of requested extra transfers
    pub extra_transfers: Vec<TransferDetails>,
}
