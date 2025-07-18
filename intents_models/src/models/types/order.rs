use crate::error::{Error, ModelResult};
use crate::models::types::cross_chain::{
    CrossChainOnChainLimitOrderData, CrossChainOnChainOrderDataEnum,
    CrossChainUserLimitOrderResponse,
};
use crate::models::types::single_chain::{
    SingleChainOnChainLimitOrderData, SingleChainOnChainOrderDataEnum,
    SingleChainUserLimitOrderResponse,
};
use error_stack::report;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Collected on chain order data about current on chain order state
pub enum OnChainOrderDataEnum {
    SingleChainLimitOrder(SingleChainOnChainLimitOrderData),
    // SingleChainDcaOrder(SingleChainOnChainDcaOrderData), todo
    CrossChainLimitOrder(CrossChainOnChainLimitOrderData),
    // CrossChainDcaOrder(CrossChainOnChainDcaOrderData), todo
}

impl OnChainOrderDataEnum {
    pub fn try_into_single_chain(self) -> ModelResult<SingleChainOnChainOrderDataEnum> {
        match self {
            OnChainOrderDataEnum::SingleChainLimitOrder(data) => {
                Ok(SingleChainOnChainOrderDataEnum::SingleChainLimitOrder(data))
            }
            OnChainOrderDataEnum::CrossChainLimitOrder(_) => Err(report!(Error::LogicError(
                "Non-single-chain intent passed".to_string()
            ))),
        }
    }
    pub fn try_into_cross_chain(self) -> ModelResult<CrossChainOnChainOrderDataEnum> {
        match self {
            OnChainOrderDataEnum::SingleChainLimitOrder(_) => Err(report!(Error::LogicError(
                "Non-cross-chain intent passed".to_string()
            ))),
            OnChainOrderDataEnum::CrossChainLimitOrder(data) => {
                Ok(CrossChainOnChainOrderDataEnum::CrossChainLimitOrder(data))
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Eq, Hash, Copy)]
pub enum OrderType {
    CrossChainLimitOrder,
    // CrossChainDCAOrder,
    SingleChainLimitOrder,
    // SingleChainDCAOrder,
}

impl fmt::Display for OrderType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let value = match self {
            OrderType::CrossChainLimitOrder => "CrossChainLimitOrder",
            OrderType::SingleChainLimitOrder => "SingleChainLimitOrder",
        };
        write!(f, "{value}")
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
/// Represents the lifecycle status of an order from a domain perspective.
pub enum OrderStatus {
    /// In auction stage, waiting for bids.
    Auction,

    /// No bids were received for the order. Is set as limit order.
    NoBids,

    /// The order got a winner bid and the solver is going to execute it.
    Executing,

    /// The order was correctly executed.
    Fulfilled,

    /// The order was cancelled before execution.
    Cancelled,

    /// The order was not fulfilled before its deadline.
    Outdated,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// List of orders provided to user on request
pub struct UserOrders {
    pub single_chain_limit_orders: Vec<SingleChainUserLimitOrderResponse>,
    pub cross_chain_limit_orders: Vec<CrossChainUserLimitOrderResponse>,
}
