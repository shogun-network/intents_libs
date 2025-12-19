use crate::constants::chains::ChainId;
use crate::error::{Error, ModelResult};
use crate::models::types::cross_chain::{
    CrossChainOnChainDcaOrderData, CrossChainOnChainLimitOrderData, CrossChainOnChainOrderDataEnum,
    CrossChainUserDcaOrderResponse, CrossChainUserLimitOrderResponse,
};
use crate::models::types::single_chain::{
    SingleChainOnChainDcaOrderData, SingleChainOnChainLimitOrderData,
    SingleChainOnChainOrderDataEnum, SingleChainUserDcaOrderResponse,
    SingleChainUserLimitOrderResponse,
};
use error_stack::report;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

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
    CrossChainDcaOrder(CrossChainOnChainDcaOrderData),
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
            OnChainOrderDataEnum::CrossChainLimitOrder(_)
            | OnChainOrderDataEnum::CrossChainDcaOrder(_) => Err(report!(Error::LogicError(
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
            OnChainOrderDataEnum::CrossChainDcaOrder(data) => {
                Ok(CrossChainOnChainOrderDataEnum::CrossChainDcaOrder(data))
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
            OnChainOrderDataEnum::CrossChainDcaOrder(order_data) => {
                let deactivated = order_data.common_data.deactivated.unwrap_or(false);
                !deactivated
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Eq, Hash, Copy)]
pub enum OrderType {
    CrossChainLimitOrder,
    CrossChainDCAOrder,
    SingleChainLimitOrder,
    SingleChainDCAOrder,
}

impl fmt::Display for OrderType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let value = match self {
            OrderType::CrossChainLimitOrder => "CrossChainLimitOrder",
            OrderType::CrossChainDCAOrder => "CrossChainDCAOrder",
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

impl FromStr for OrderStatus {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Auction" => Ok(OrderStatus::Auction),
            "NoBids" => Ok(OrderStatus::NoBids),
            "Executing" => Ok(OrderStatus::Executing),
            "DcaIntervalFulfilled" => Ok(OrderStatus::DcaIntervalFulfilled),
            "Fulfilled" => Ok(OrderStatus::Fulfilled),
            "Cancelled" => Ok(OrderStatus::Cancelled),
            "Outdated" => Ok(OrderStatus::Outdated),
            _ => Err(Error::ParseError),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
/// List of orders provided to user on request
pub struct UserOrders {
    pub single_chain_limit_orders: Vec<SingleChainUserLimitOrderResponse>,
    pub single_chain_dca_orders: Vec<SingleChainUserDcaOrderResponse>,
    pub cross_chain_limit_orders: Vec<CrossChainUserLimitOrderResponse>,
    pub cross_chain_dca_orders: Vec<CrossChainUserDcaOrderResponse>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum UserOrderType {
    CrossChainLimitOrder(CrossChainUserLimitOrderResponse),
    CrossChainDCAOrder(CrossChainUserDcaOrderResponse),
    SingleChainLimitOrder(SingleChainUserLimitOrderResponse),
    SingleChainDCAOrder(SingleChainUserDcaOrderResponse),
}

impl UserOrderType {
    pub fn order_type(&self) -> OrderType {
        match self {
            UserOrderType::CrossChainLimitOrder(_) => OrderType::CrossChainLimitOrder,
            UserOrderType::CrossChainDCAOrder(_) => OrderType::CrossChainDCAOrder,
            UserOrderType::SingleChainLimitOrder(_) => OrderType::SingleChainLimitOrder,
            UserOrderType::SingleChainDCAOrder(_) => OrderType::SingleChainDCAOrder,
        }
    }

    pub fn order_id(&self) -> &String {
        match self {
            UserOrderType::CrossChainLimitOrder(order) => &order.order_id,
            UserOrderType::CrossChainDCAOrder(order) => &order.order_id,
            UserOrderType::SingleChainLimitOrder(order) => &order.order_id,
            UserOrderType::SingleChainDCAOrder(order) => &order.order_id,
        }
    }

    pub fn src_chain_id(&self) -> ChainId {
        match self {
            UserOrderType::CrossChainLimitOrder(order) => {
                order.generic_data.common_data.src_chain_id
            }
            UserOrderType::CrossChainDCAOrder(order) => order.generic_data.common_data.src_chain_id,
            UserOrderType::SingleChainLimitOrder(order) => order.generic_data.common_data.chain_id,
            UserOrderType::SingleChainDCAOrder(order) => order.generic_data.common_data.chain_id,
        }
    }

    pub fn dest_chain_id(&self) -> ChainId {
        match self {
            UserOrderType::CrossChainLimitOrder(order) => {
                order.generic_data.common_data.dest_chain_id
            }
            UserOrderType::CrossChainDCAOrder(order) => {
                order.generic_data.common_data.dest_chain_id
            }
            UserOrderType::SingleChainLimitOrder(order) => order.generic_data.common_data.chain_id,
            UserOrderType::SingleChainDCAOrder(order) => order.generic_data.common_data.chain_id,
        }
    }

    pub fn order_status(&self) -> &OrderStatus {
        match self {
            UserOrderType::CrossChainLimitOrder(order) => &order.order_status,
            UserOrderType::CrossChainDCAOrder(order) => &order.order_status,
            UserOrderType::SingleChainLimitOrder(order) => &order.order_status,
            UserOrderType::SingleChainDCAOrder(order) => &order.order_status,
        }
    }

    pub fn token_in(&self) -> &String {
        match self {
            UserOrderType::CrossChainLimitOrder(order) => &order.generic_data.common_data.token_in,
            UserOrderType::CrossChainDCAOrder(order) => &order.generic_data.common_data.token_in,
            UserOrderType::SingleChainLimitOrder(order) => &order.generic_data.common_data.token_in,
            UserOrderType::SingleChainDCAOrder(order) => &order.generic_data.common_data.token_in,
        }
    }

    pub fn token_out(&self) -> &String {
        match self {
            UserOrderType::CrossChainLimitOrder(order) => &order.generic_data.common_data.token_out,
            UserOrderType::CrossChainDCAOrder(order) => &order.generic_data.common_data.token_out,
            UserOrderType::SingleChainLimitOrder(order) => {
                &order.generic_data.common_data.token_out
            }
            UserOrderType::SingleChainDCAOrder(order) => &order.generic_data.common_data.token_out,
        }
    }

    pub fn amount_in(&self) -> u128 {
        match self {
            UserOrderType::CrossChainLimitOrder(order) => order.generic_data.amount_in,
            UserOrderType::CrossChainDCAOrder(order) => {
                order
                    .generic_data
                    .common_dca_order_data
                    .amount_in_per_interval
            }
            UserOrderType::SingleChainLimitOrder(order) => order.generic_data.amount_in,
            UserOrderType::SingleChainDCAOrder(order) => {
                order
                    .generic_data
                    .common_dca_order_data
                    .amount_in_per_interval
            }
        }
    }

    pub fn amount_out(&self) -> Option<u128> {
        match self {
            UserOrderType::CrossChainLimitOrder(order) => order.amount_out,
            UserOrderType::CrossChainDCAOrder(_) => None,
            UserOrderType::SingleChainLimitOrder(order) => order.amount_out,
            UserOrderType::SingleChainDCAOrder(_) => None,
        }
    }

    pub fn order_creation_time(&self) -> u64 {
        match self {
            UserOrderType::CrossChainLimitOrder(order) => order.order_creation_time,
            UserOrderType::CrossChainDCAOrder(order) => order.order_creation_time,
            UserOrderType::SingleChainLimitOrder(order) => order.order_creation_time,
            UserOrderType::SingleChainDCAOrder(order) => order.order_creation_time,
        }
    }

    pub fn order_fulfillment_timestamp(&self) -> Option<u64> {
        match self {
            UserOrderType::CrossChainLimitOrder(order) => order.order_fulfillment_timestamp,
            UserOrderType::CrossChainDCAOrder(_) => None,
            UserOrderType::SingleChainLimitOrder(order) => order.order_fulfillment_timestamp,
            UserOrderType::SingleChainDCAOrder(_) => None,
        }
    }

    pub fn get_amount_out_min(&self) -> Option<u128> {
        match self {
            UserOrderType::CrossChainLimitOrder(order) => {
                Some(order.generic_data.get_amount_out_min())
            }
            UserOrderType::CrossChainDCAOrder(_) => None,
            UserOrderType::SingleChainLimitOrder(order) => {
                Some(order.generic_data.get_amount_out_min())
            }
            UserOrderType::SingleChainDCAOrder(_) => None,
        }
    }

    pub fn stop_loss_trigger_price(&self) -> Option<f64> {
        match self {
            UserOrderType::CrossChainLimitOrder(order) => {
                order
                    .generic_data
                    .common_limit_order_data
                    .stop_loss_trigger_price
            }
            UserOrderType::CrossChainDCAOrder(_) => None,
            UserOrderType::SingleChainLimitOrder(order) => {
                order
                    .generic_data
                    .common_limit_order_data
                    .stop_loss_trigger_price
            }
            UserOrderType::SingleChainDCAOrder(_) => None,
        }
    }

    pub fn stop_loss_triggered(&self) -> Option<bool> {
        match self {
            UserOrderType::CrossChainLimitOrder(order) => Some(
                order
                    .generic_data
                    .common_limit_order_data
                    .stop_loss_triggered,
            ),
            UserOrderType::CrossChainDCAOrder(_) => None,
            UserOrderType::SingleChainLimitOrder(order) => Some(
                order
                    .generic_data
                    .common_limit_order_data
                    .stop_loss_triggered,
            ),
            UserOrderType::SingleChainDCAOrder(_) => None,
        }
    }

    pub fn take_profit_min_out(&self) -> Option<u128> {
        match self {
            UserOrderType::CrossChainLimitOrder(order) => {
                order
                    .generic_data
                    .common_limit_order_data
                    .take_profit_min_out
            }
            UserOrderType::CrossChainDCAOrder(_) => None,
            UserOrderType::SingleChainLimitOrder(order) => {
                order
                    .generic_data
                    .common_limit_order_data
                    .take_profit_min_out
            }
            UserOrderType::SingleChainDCAOrder(_) => None,
        }
    }
}
