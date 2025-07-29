use crate::models::types::common::CommonDcaOrderData;
use crate::models::types::single_chain::{
    SingleChainChainSpecificData, SingleChainDcaOrderGenericData, SingleChainDcaOrderIntentRequest,
    SingleChainGenericData,
};
use crate::models::types::user_types::IntentRequest;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Single chain dca order intent request, received from the user
pub struct SingleChainDcaOrderUserIntentRequest {
    /// Contains the common data for the intent
    pub generic_data: SingleChainDcaOrderGenericRequestData,
    /// Contains chain-specific data
    pub chain_specific_data: SingleChainChainSpecificData,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// A structure to hold generic data related to the single chain dca order intent
pub struct SingleChainDcaOrderGenericRequestData {
    /// User address initiating the intent
    #[serde(flatten)]
    pub common_data: SingleChainGenericData,
    /// Common dca order data to trigger "take profit" or "stop loss" execution
    #[serde(flatten)]
    pub common_dca_order_data: CommonDcaOrderData,
}

impl SingleChainDcaOrderUserIntentRequest {
    pub fn into_into_intent_request(self) -> IntentRequest {
        let generic_data = SingleChainDcaOrderGenericData {
            common_data: SingleChainGenericData {
                user: self.generic_data.common_data.user.clone(),
                chain_id: self.generic_data.common_data.chain_id,
                token_in: self.generic_data.common_data.token_in.clone(),
                token_out: self.generic_data.common_data.token_out.clone(),
                amount_out_min: self.generic_data.common_data.amount_out_min,
                destination_address: self.generic_data.common_data.destination_address.clone(),
                extra_transfers: self.generic_data.common_data.extra_transfers,
                deadline: self.generic_data.common_data.deadline,
            },
            common_dca_order_data: self.generic_data.common_dca_order_data,
        };

        IntentRequest::SingleChainDcaOrder(SingleChainDcaOrderIntentRequest {
            generic_data,
            chain_specific_data: self.chain_specific_data.clone(),
        })
    }
}

impl From<SingleChainDcaOrderGenericData> for SingleChainDcaOrderGenericRequestData {
    fn from(value: SingleChainDcaOrderGenericData) -> Self {
        Self {
            common_data: SingleChainGenericData {
                user: value.common_data.user,
                chain_id: value.common_data.chain_id,
                token_in: value.common_data.token_in,
                token_out: value.common_data.token_out,
                amount_out_min: value.common_data.amount_out_min,
                destination_address: value.common_data.destination_address,
                extra_transfers: value.common_data.extra_transfers,
                deadline: value.common_data.deadline,
            },
            common_dca_order_data: value.common_dca_order_data,
        }
    }
}
