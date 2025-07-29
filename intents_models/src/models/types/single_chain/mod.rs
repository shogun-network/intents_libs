use serde::{Deserialize, Serialize};

mod common;
mod dca_order;
mod limit_order;
mod solver_types;

use crate::constants::chains::ChainId;
use crate::models::types::order::OrderType;
use crate::models::types::solver_types::SolverStartPermission;
use crate::models::types::user_types::IntentRequest;
pub use common::*;
pub use dca_order::*;
pub use limit_order::*;
pub use solver_types::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum SingleChainIntentRequest {
    SingleChainLimitOrder(SingleChainLimitOrderIntentRequest),
    SingleChainDcaOrder(SingleChainDcaOrderIntentRequest),
}

impl SingleChainIntentRequest {
    pub fn get_order_type(&self) -> OrderType {
        match self {
            SingleChainIntentRequest::SingleChainLimitOrder(_) => OrderType::SingleChainLimitOrder,
            &SingleChainIntentRequest::SingleChainDcaOrder(_) => OrderType::SingleChainDCAOrder,
        }
    }
    pub fn get_common_data(&self) -> &SingleChainGenericData {
        match self {
            SingleChainIntentRequest::SingleChainLimitOrder(request) => {
                &request.generic_data.common_data
            }
            SingleChainIntentRequest::SingleChainDcaOrder(request) => {
                &request.generic_data.common_data
            }
        }
    }
    pub fn get_chain_specific_data(&self) -> &SingleChainChainSpecificData {
        match self {
            SingleChainIntentRequest::SingleChainLimitOrder(request) => {
                &request.chain_specific_data
            }
            SingleChainIntentRequest::SingleChainDcaOrder(request) => &request.chain_specific_data,
        }
    }
    pub fn get_amount_in(&self) -> u128 {
        match self {
            SingleChainIntentRequest::SingleChainLimitOrder(request) => {
                request.generic_data.amount_in
            }
            SingleChainIntentRequest::SingleChainDcaOrder(request) => {
                request.generic_data.common_dca_order_data.amount_in_per_interval
            }
        }
    }
    pub fn to_intent_request(self) -> IntentRequest {
        match self {
            SingleChainIntentRequest::SingleChainLimitOrder(request) => {
                IntentRequest::SingleChainLimitOrder(request)
            }
            SingleChainIntentRequest::SingleChainDcaOrder(request) => {
                IntentRequest::SingleChainDcaOrder(request)
            }
        }
    }

    pub fn get_chain_id(&self) -> ChainId {
        match self {
            SingleChainIntentRequest::SingleChainLimitOrder(request) => {
                request.generic_data.common_data.chain_id
            }
            SingleChainIntentRequest::SingleChainDcaOrder(request) => {
                request.generic_data.common_data.chain_id
            }
        }
    }

    pub fn get_amount_out_min(&self) -> u128 {
        match self {
            SingleChainIntentRequest::SingleChainLimitOrder(request) => {
                request.generic_data.common_data.amount_out_min
            }
            SingleChainIntentRequest::SingleChainDcaOrder(request) => {
                request.generic_data.common_data.amount_out_min
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
/// Permission, granted to Solver single-chain order execution
pub enum SingleChainSolverStartPermissionEnum {
    Limit(SingleChainLimitOrderSolverStartPermission),
    Dca(SingleChainDcaOrderSolverStartPermission),
}

impl SingleChainSolverStartPermissionEnum {
    pub fn get_permission_deadline(&self) -> u64 {
        match self {
            SingleChainSolverStartPermissionEnum::Limit(data) => data.common_data.solver_deadline,
            SingleChainSolverStartPermissionEnum::Dca(data) => data.common_data.solver_deadline,
        }
    }
    pub fn get_common_data(&self) -> (&SingleChainSolverStartPermission, &SingleChainGenericData) {
        match self {
            SingleChainSolverStartPermissionEnum::Limit(data) => {
                (&data.common_data, &data.generic_data.common_data)
            }
            SingleChainSolverStartPermissionEnum::Dca(data) => {
                (&data.common_data, &data.generic_data.common_data)
            }
        }
    }
    pub fn into_generic_start_permission(self) -> SolverStartPermission {
        match self {
            SingleChainSolverStartPermissionEnum::Limit(data) => {
                SolverStartPermission::SingleChainLimit(data)
            }
            SingleChainSolverStartPermissionEnum::Dca(data) => {
                SolverStartPermission::SingleChainDca(data)
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Collected on chain order data about current on chain order state
pub enum SingleChainOnChainOrderDataEnum {
    SingleChainLimitOrder(SingleChainOnChainLimitOrderData),
    SingleChainDcaOrder(SingleChainOnChainDcaOrderData),
}

impl SingleChainOnChainOrderDataEnum {
    pub fn get_common_data(&self) -> &SingleChainOnChainOrderData {
        match self {
            SingleChainOnChainOrderDataEnum::SingleChainLimitOrder(data) => &data.common_data,
            SingleChainOnChainOrderDataEnum::SingleChainDcaOrder(data) => &data.common_data,
        }
    }
}
