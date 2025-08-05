use crate::constants::chains::ChainId;
use crate::models::types::order::OrderType;
use serde::{Deserialize, Serialize};

mod common;
mod dca_orders;
mod fulfillment;
mod limit_orders;
mod order;

use crate::models::types::solver_types::SolverStartPermission;
use crate::models::types::user_types::IntentRequest;
pub use common::*;
pub use dca_orders::*;
pub use fulfillment::*;
pub use limit_orders::*;
pub use order::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum CrossChainIntentRequest {
    CrossChainLimitOrder(CrossChainLimitOrderIntentRequest),
    // CrossChainDcaOrder(CrossChainDcaOrderIntentRequest), todo
}

impl CrossChainIntentRequest {
    pub fn get_order_type(&self) -> OrderType {
        match self {
            CrossChainIntentRequest::CrossChainLimitOrder(_) => OrderType::CrossChainLimitOrder,
        }
    }
    pub fn get_common_data(&self) -> (&CrossChainGenericData, &CrossChainChainSpecificData) {
        match self {
            CrossChainIntentRequest::CrossChainLimitOrder(intent) => (
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
        }
    }
    pub fn into_intent_request(self) -> IntentRequest {
        match self {
            CrossChainIntentRequest::CrossChainLimitOrder(intent) => {
                IntentRequest::CrossChainLimitOrder(intent)
            }
        }
    }

    pub fn get_amount_out_min(&self) -> u128 {
        match self {
            CrossChainIntentRequest::CrossChainLimitOrder(intent) => {
                intent.generic_data.common_data.amount_out_min
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
    // Dca(CrossChainDcaOrderSolverStartPermission), todo
}

impl CrossChainSolverStartPermissionEnum {
    pub fn get_amount_in(&self) -> u128 {
        match self {
            CrossChainSolverStartPermissionEnum::Limit(permission) => {
                permission.generic_data.amount_in
            }
        }
    }
    pub fn get_src_chain_id(&self) -> ChainId {
        match self {
            CrossChainSolverStartPermissionEnum::Limit(permission) => {
                permission.generic_data.common_data.src_chain_id
            }
        }
    }
    pub fn get_dest_chain_id(&self) -> ChainId {
        match self {
            CrossChainSolverStartPermissionEnum::Limit(permission) => {
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
        }
    }
    pub fn get_chain_specific_data(&self) -> &CrossChainSolverStartOrderData {
        match self {
            CrossChainSolverStartPermissionEnum::Limit(permission) => {
                &permission.common_data.chain_specific_data
            }
        }
    }

    pub fn into_generic_start_permission(self) -> SolverStartPermission {
        match self {
            CrossChainSolverStartPermissionEnum::Limit(permission) => {
                SolverStartPermission::CrossChainLimit(permission)
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Collected on chain order data about current on chain order state
pub enum CrossChainOnChainOrderDataEnum {
    CrossChainLimitOrder(CrossChainOnChainLimitOrderData),
    // CrossChainDcaOrder(CrossChainOnChainDcaOrderData), todo
}

impl CrossChainOnChainOrderDataEnum {
    pub fn get_common_data(&self) -> &CrossChainOnChainOrderData {
        match self {
            CrossChainOnChainOrderDataEnum::CrossChainLimitOrder(data) => &data.common_data,
        }
    }
}
