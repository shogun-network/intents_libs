use crate::models::types::cross_chain::{
    CrossChainLimitOrderGenericRequestData, CrossChainLimitOrderUserIntentRequest,
};
use crate::models::types::single_chain::SingleChainLimitOrderGenericData;
use crate::models::types::user_types::TransferDetails;
use crate::{
    constants::chains::ChainId, models::types::single_chain::SingleChainLimitOrderIntentRequest,
};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
/// Intent request, received from the user, but not converted to `IntentRequest` enum yet
/// Main purpose is to pass data which `IntentRequest` doesn't have (like `execution_details`)
pub enum UserIntentRequest {
    SingleChainLimitOrder(SingleChainLimitOrderIntentRequest),
    // SingleChainDcaOrder(SingleChainDcaOrderIntentRequest),
    CrossChainLimitOrder(CrossChainLimitOrderUserIntentRequest),
    // CrossChainDcaOrder(CrossChainDcaOrderUserIntentRequest),
}

pub enum GenericData {
    SingleChain(SingleChainLimitOrderGenericData),
    CrossChain(CrossChainLimitOrderGenericRequestData),
}

impl GenericData {
    pub fn get_src_chain_id(&self) -> ChainId {
        match self {
            GenericData::SingleChain(data) => data.common_data.chain_id,
            GenericData::CrossChain(data) => data.src_chain_id,
        }
    }

    pub fn get_token_in(&self) -> String {
        match self {
            GenericData::SingleChain(data) => data.common_data.token_in.clone(),
            GenericData::CrossChain(data) => data.token_in.clone(),
        }
    }

    pub fn set_token_in(&mut self, token_in: String) {
        match self {
            GenericData::SingleChain(data) => data.common_data.token_in = token_in,
            GenericData::CrossChain(data) => data.token_in = token_in,
        }
    }

    pub fn get_amount_in(&self) -> u128 {
        match self {
            GenericData::SingleChain(data) => data.amount_in,
            GenericData::CrossChain(data) => data.amount_in,
        }
    }

    pub fn get_user(&self) -> String {
        match self {
            GenericData::SingleChain(data) => data.common_data.user.clone(),
            GenericData::CrossChain(data) => data.user.clone(),
        }
    }

    pub fn get_deadline(&self) -> u64 {
        match self {
            GenericData::SingleChain(data) => data.common_data.deadline,
            GenericData::CrossChain(data) => data.deadline,
        }
    }
}

/// A structure to hold generic data related to the intent
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionDetails {
    /// Destination chain identifier
    pub dest_chain_id: ChainId,
    /// Token to be received after the operation (e.g., "USDT", "DAI")
    pub token_out: String,
    /// The minimum amount of the output token to be received after the operation
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub amount_out_min: u128,
    /// Destination address for the operation (e.g., recipient address)
    pub destination_address: String,
    /// Requested array of extra transfers with fixed amounts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_transfers: Option<Vec<TransferDetails>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Eq, Hash, Copy)]
pub enum OrderType {
    CrossChainLimitOrder,
    // CrossChainDCAOrder,
    SingleChainLimitOrder,
    // SingleChainDCAOrder,
}
