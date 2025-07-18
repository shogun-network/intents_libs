use crate::error::{Error, ModelResult};
use crate::models::types::cross_chain::{
    CrossChainOnChainLimitOrderData, CrossChainOnChainOrderDataEnum,
};
use crate::models::types::single_chain::{
    SingleChainOnChainLimitOrderData, SingleChainOnChainOrderDataEnum,
};
use error_stack::report;
use serde::{Deserialize, Serialize};

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
