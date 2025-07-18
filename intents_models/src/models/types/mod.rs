use serde::{Deserialize, Serialize};
use std::fmt;

pub mod cross_chain;
pub mod order;
pub mod single_chain;
pub mod solver_types;
pub mod user_types;

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
