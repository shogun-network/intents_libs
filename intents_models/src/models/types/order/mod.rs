use crate::error::{Error, ModelResult};
use crate::models::types::cross_chain::{
    CrossChainOnChainLimitOrderData, CrossChainOnChainOrderDataEnum,
    CrossChainUserLimitOrderResponse,
};
use crate::models::types::single_chain::{
    SingleChainOnChainDcaOrderData, SingleChainOnChainLimitOrderData,
    SingleChainOnChainOrderDataEnum, SingleChainUserDcaOrderResponse,
    SingleChainUserLimitOrderResponse,
};
use error_stack::{ResultExt, report};
use serde::{Deserialize, Serialize};
use std::fmt;

mod execution;
mod order_data_request;

pub use execution::*;
pub use order_data_request::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Collected on chain order data about current on chain order state
pub enum OnChainOrderDataEnum {
    SingleChainLimitOrder(SingleChainOnChainLimitOrderData),
    SingleChainDcaOrder(SingleChainOnChainDcaOrderData),
    CrossChainLimitOrder(CrossChainOnChainLimitOrderData),
    // CrossChainDcaOrder(CrossChainOnChainDcaOrderData), todo
}

impl OnChainOrderDataEnum {
    pub fn try_into_single_chain(self) -> ModelResult<SingleChainOnChainOrderDataEnum> {
        match self {
            OnChainOrderDataEnum::SingleChainLimitOrder(data) => {
                Ok(SingleChainOnChainOrderDataEnum::SingleChainLimitOrder(data))
            }
            OnChainOrderDataEnum::SingleChainDcaOrder(data) => {
                Ok(SingleChainOnChainOrderDataEnum::SingleChainDcaOrder(data))
            }
            OnChainOrderDataEnum::CrossChainLimitOrder(_) => Err(report!(Error::LogicError(
                "Non-single-chain intent passed".to_string()
            ))),
        }
    }
    pub fn try_into_cross_chain(self) -> ModelResult<CrossChainOnChainOrderDataEnum> {
        match self {
            OnChainOrderDataEnum::SingleChainLimitOrder(_)
            | OnChainOrderDataEnum::SingleChainDcaOrder(_) => Err(report!(Error::LogicError(
                "Non-cross-chain intent passed".to_string()
            ))),
            OnChainOrderDataEnum::CrossChainLimitOrder(data) => {
                Ok(CrossChainOnChainOrderDataEnum::CrossChainLimitOrder(data))
            }
        }
    }

    pub fn is_active(&self) -> bool {
        match &self {
            OnChainOrderDataEnum::SingleChainLimitOrder(order_data) => {
                order_data.common_data.active
            }
            OnChainOrderDataEnum::SingleChainDcaOrder(order_data) => order_data.common_data.active,
            OnChainOrderDataEnum::CrossChainLimitOrder(order_data) => {
                let deactivated = order_data.common_data.deactivated.unwrap_or(false);
                !deactivated
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Eq, Hash, Copy)]
pub enum OrderType {
    CrossChainLimitOrder,
    // CrossChainDCAOrder,
    SingleChainLimitOrder,
    SingleChainDCAOrder,
}

impl fmt::Display for OrderType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let value = match self {
            OrderType::CrossChainLimitOrder => "CrossChainLimitOrder",
            OrderType::SingleChainLimitOrder => "SingleChainLimitOrder",
            OrderType::SingleChainDCAOrder => "SingleChainDCAOrder",
        };
        write!(f, "{value}")
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
/// Represents the lifecycle status of an order from a domain perspective.
pub enum OrderStatus {
    /// In auction stage, waiting for bids.
    Auction,

    /// No bids were received for the order. Is set as limit order.
    NoBids,

    /// The order got a winner bid and the solver is going to execute it.
    Executing,

    /// Dca interval was fulfilled successfully.
    /// Waiting for next interval
    DcaIntervalFulfilled,

    /// The order was correctly executed.
    Fulfilled,

    /// The order was cancelled before execution.
    Cancelled,

    /// The order was not fulfilled before its deadline.
    Outdated,
}

impl fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            OrderStatus::Auction => "Auction",
            OrderStatus::NoBids => "NoBids",
            OrderStatus::Executing => "Executing",
            OrderStatus::DcaIntervalFulfilled => "DcaIntervalFulfilled",
            OrderStatus::Fulfilled => "Fulfilled",
            OrderStatus::Cancelled => "Cancelled",
            OrderStatus::Outdated => "Outdated",
        };
        write!(f, "{value}")
    }
}

// Helper functions to parse string status into enums
pub fn parse_order_status(status: &str) -> ModelResult<OrderStatus> {
    Ok(match status {
        "Auction" => OrderStatus::Auction,
        "NoBids" => OrderStatus::NoBids,
        "Executing" => OrderStatus::Executing,
        "DcaIntervalFulfilled" => OrderStatus::DcaIntervalFulfilled,
        "Fulfilled" => OrderStatus::Fulfilled,
        "Cancelled" => OrderStatus::Cancelled,
        "Outdated" => OrderStatus::Outdated,
        _ => Err(Error::ParseError).attach_printable(format!("Invalid order status: {status}"))?,
    })
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// List of orders provided to user on request
pub struct UserOrders {
    pub single_chain_limit_orders: Vec<SingleChainUserLimitOrderResponse>,
    pub single_chain_dca_orders: Vec<SingleChainUserDcaOrderResponse>,
    pub cross_chain_limit_orders: Vec<CrossChainUserLimitOrderResponse>,
    // todo cross chain dca
}
