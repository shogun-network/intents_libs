use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};
use crate::models::types::common::{CommonLimitOrderData, CommonLimitOrderUserRequestData};
use crate::models::types::single_chain::{SingleChainChainSpecificData, SingleChainGenericData, SingleChainLimitOrderGenericData, SingleChainLimitOrderIntentRequest};
use crate::models::types::user_types::IntentRequest;

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Intent request, received from the user
pub struct SingleChainLimitOrderUserIntentRequest {
    /// Contains the common data for the intent
    pub generic_data: SingleChainLimitOrderGenericRequestData,
    /// Contains chain-specific data
    pub chain_specific_data: SingleChainChainSpecificData,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// A structure to hold generic data related to the intent
pub struct SingleChainLimitOrderGenericRequestData {
    /// User address initiating the intent
    #[serde(flatten)]
    pub common_data: SingleChainGenericData,
    /// Common limit order data to trigger "take profit" or "stop loss" execution
    #[serde(flatten)]
    pub common_limit_order_data: CommonLimitOrderUserRequestData,
    /// The amount of the input token to be used in the operation
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub amount_in: u128,
}
impl SingleChainLimitOrderUserIntentRequest {
    pub fn into_into_intent_request(self) -> IntentRequest {
        let generic_data = SingleChainLimitOrderGenericData {
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
            common_limit_order_data: CommonLimitOrderData {
                take_profit_min_out: self
                    .generic_data
                    .common_limit_order_data
                    .take_profit_min_out,
                stop_loss_max_out: self.generic_data.common_limit_order_data.stop_loss_max_out,
                stop_loss_triggered: false,
            },
            amount_in: self.generic_data.amount_in,
        };

        IntentRequest::SingleChainLimitOrder(SingleChainLimitOrderIntentRequest {
            generic_data,
            chain_specific_data: self.chain_specific_data.clone(),
        })
    }
}
