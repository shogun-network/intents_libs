use crate::models::types::common::{CommonDcaOrderData, CommonDcaOrderState};
use crate::models::types::single_chain::{SingleChainChainSpecificData, SingleChainGenericData};
use crate::models::types::user_types::IntentRequest;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Single chain DCA order intent structure
pub struct SingleChainDcaOrderIntentRequest {
    /// Contains the common data for the intent
    pub generic_data: SingleChainDcaOrderGenericData,
    /// Contains chain-specific data
    pub chain_specific_data: SingleChainChainSpecificData,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Generic data related to the single-chain DCA order
pub struct SingleChainDcaOrderGenericData {
    /// User address initiating the intent
    #[serde(flatten)]
    pub common_data: SingleChainGenericData,
    /// Common DCA order data
    #[serde(flatten)]
    pub common_dca_order_data: CommonDcaOrderData,
    /// Common DCA order state
    #[serde(flatten)]
    pub common_dca_state: CommonDcaOrderState,
}

impl SingleChainDcaOrderIntentRequest {
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
            common_dca_order_data: CommonDcaOrderData {
                start_time: self.generic_data.common_dca_order_data.start_time,
                amount_in_per_interval: self
                    .generic_data
                    .common_dca_order_data
                    .amount_in_per_interval,
                total_intervals: self.generic_data.common_dca_order_data.total_intervals,
                interval_duration: self.generic_data.common_dca_order_data.interval_duration,
            },
            common_dca_state: CommonDcaOrderState {
                total_executed_intervals: 0,
                last_executed_interval_index: 0,
            },
        };

        IntentRequest::SingleChainDcaOrder(SingleChainDcaOrderIntentRequest {
            generic_data,
            chain_specific_data: self.chain_specific_data.clone(),
        })
    }
}
