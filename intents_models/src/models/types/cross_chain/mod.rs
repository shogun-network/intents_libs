mod common;
mod dca_orders;
mod fulfillment;
mod limit_orders;
mod order;

use crate::constants::chains::ChainId;
use crate::models::types::order::{DcaOrderFulfillmentData, OrderType, OrderTypeFulfillmentData};
use crate::models::types::solver_types::SolverStartPermission;
use crate::models::types::user_types::IntentRequest;
pub use common::*;
pub use dca_orders::*;
pub use fulfillment::*;
pub use limit_orders::*;
pub use order::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum CrossChainIntentRequest {
    CrossChainLimitOrder(CrossChainLimitOrderIntentRequest),
    CrossChainDcaOrder(CrossChainDcaOrderIntentRequest),
}

impl CrossChainIntentRequest {
    pub fn get_order_type(&self) -> OrderType {
        match self {
            CrossChainIntentRequest::CrossChainLimitOrder(_) => OrderType::CrossChainLimitOrder,
            &CrossChainIntentRequest::CrossChainDcaOrder(_) => OrderType::CrossChainDCAOrder,
        }
    }
    pub fn get_common_data(&self) -> (&CrossChainGenericData, &CrossChainChainSpecificData) {
        match self {
            CrossChainIntentRequest::CrossChainLimitOrder(intent) => (
                &intent.generic_data.common_data,
                &intent.chain_specific_data,
            ),
            CrossChainIntentRequest::CrossChainDcaOrder(intent) => (
                &intent.generic_data.common_data,
                &intent.chain_specific_data,
            ),
        }
    }
    pub fn get_src_chain(&self) -> ChainId {
        match self {
            CrossChainIntentRequest::CrossChainLimitOrder(intent) => {
                intent.generic_data.common_data.src_chain_id
            }
            CrossChainIntentRequest::CrossChainDcaOrder(intent) => {
                intent.generic_data.common_data.src_chain_id
            }
        }
    }
    pub fn into_intent_request(self) -> IntentRequest {
        match self {
            CrossChainIntentRequest::CrossChainLimitOrder(intent) => {
                IntentRequest::CrossChainLimitOrder(intent)
            }
            CrossChainIntentRequest::CrossChainDcaOrder(intent) => {
                IntentRequest::CrossChainDcaOrder(intent)
            }
        }
    }

    pub fn get_amount_out_min(&self) -> u128 {
        match self {
            CrossChainIntentRequest::CrossChainLimitOrder(intent) => {
                intent.generic_data.common_data.amount_out_min
            }
            CrossChainIntentRequest::CrossChainDcaOrder(intent) => {
                intent.generic_data.common_data.amount_out_min
            }
        }
    }
    pub fn get_execution_amount_in(&self) -> u128 {
        match self {
            CrossChainIntentRequest::CrossChainLimitOrder(intent) => intent.generic_data.amount_in,
            CrossChainIntentRequest::CrossChainDcaOrder(intent) => {
                intent
                    .generic_data
                    .common_dca_order_data
                    .amount_in_per_interval
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum CrossChainGenericDataEnum {
    Limit(CrossChainLimitOrderGenericData),
    DCA(CrossChainDcaOrderGenericData),
}

impl CrossChainGenericDataEnum {
    pub fn get_src_chain_id(&self) -> ChainId {
        match self {
            CrossChainGenericDataEnum::Limit(data) => data.common_data.src_chain_id,
            CrossChainGenericDataEnum::DCA(data) => data.common_data.src_chain_id,
        }
    }
    pub fn get_dest_chain_id(&self) -> ChainId {
        match self {
            CrossChainGenericDataEnum::Limit(data) => data.common_data.dest_chain_id,
            CrossChainGenericDataEnum::DCA(data) => data.common_data.dest_chain_id,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum CrossChainSolverStartPermissionEnum {
    Limit(CrossChainLimitOrderSolverStartPermission),
    Dca(CrossChainDcaOrderSolverStartPermission),
}

impl CrossChainSolverStartPermissionEnum {
    pub fn get_amount_in(&self) -> u128 {
        match self {
            CrossChainSolverStartPermissionEnum::Limit(permission) => {
                permission.generic_data.amount_in
            }
            CrossChainSolverStartPermissionEnum::Dca(permission) => {
                permission
                    .generic_data
                    .common_dca_order_data
                    .amount_in_per_interval
            }
        }
    }
    pub fn get_src_chain_id(&self) -> ChainId {
        match self {
            CrossChainSolverStartPermissionEnum::Limit(permission) => {
                permission.generic_data.common_data.src_chain_id
            }
            CrossChainSolverStartPermissionEnum::Dca(permission) => {
                permission.generic_data.common_data.src_chain_id
            }
        }
    }
    pub fn get_dest_chain_id(&self) -> ChainId {
        match self {
            CrossChainSolverStartPermissionEnum::Limit(permission) => {
                permission.generic_data.common_data.dest_chain_id
            }
            CrossChainSolverStartPermissionEnum::Dca(permission) => {
                permission.generic_data.common_data.dest_chain_id
            }
        }
    }
    pub fn get_common_data(&self) -> (&CrossChainSolverStartPermission, &CrossChainGenericData) {
        match self {
            CrossChainSolverStartPermissionEnum::Limit(permission) => (
                &permission.common_data,
                &permission.generic_data.common_data,
            ),
            CrossChainSolverStartPermissionEnum::Dca(permission) => (
                &permission.common_data,
                &permission.generic_data.common_data,
            ),
        }
    }
    pub fn get_chain_specific_data(&self) -> &CrossChainSolverStartOrderData {
        match self {
            CrossChainSolverStartPermissionEnum::Limit(permission) => {
                &permission.common_data.src_chain_specific_data
            }
            CrossChainSolverStartPermissionEnum::Dca(permission) => {
                &permission.common_data.src_chain_specific_data
            }
        }
    }
    pub fn get_order_type_fulfillment_data(&self) -> OrderTypeFulfillmentData {
        match self {
            CrossChainSolverStartPermissionEnum::Limit(_) => OrderTypeFulfillmentData::Limit,
            // Wa assume next interval number is requested to be fulfilled
            CrossChainSolverStartPermissionEnum::Dca(intent) => {
                OrderTypeFulfillmentData::Dca(DcaOrderFulfillmentData {
                    interval_number: intent
                        .generic_data
                        .common_dca_state
                        .total_executed_intervals
                        + 1,
                })
            }
        }
    }

    pub fn into_generic_start_permission(self) -> SolverStartPermission {
        match self {
            CrossChainSolverStartPermissionEnum::Limit(permission) => {
                SolverStartPermission::CrossChainLimit(permission)
            }
            CrossChainSolverStartPermissionEnum::Dca(permission) => {
                SolverStartPermission::CrossChainDca(permission)
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Collected on chain order data about current on chain order state
pub enum CrossChainOnChainOrderDataEnum {
    CrossChainLimitOrder(CrossChainOnChainLimitOrderData),
    CrossChainDcaOrder(CrossChainOnChainDcaOrderData),
}

impl CrossChainOnChainOrderDataEnum {
    pub fn get_common_data(&self) -> &CrossChainOnChainOrderData {
        match self {
            CrossChainOnChainOrderDataEnum::CrossChainLimitOrder(data) => &data.common_data,
            CrossChainOnChainOrderDataEnum::CrossChainDcaOrder(data) => &data.common_data,
        }
    }
}
