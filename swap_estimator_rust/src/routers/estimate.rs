use intents_models::constants::chains::ChainId;
use serde::{Deserialize, Serialize};

use crate::routers::{RouterType, Slippage, swap::GenericSwapRequest};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeType {
    ExactIn,
    ExactOut,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenericEstimateRequest {
    pub trade_type: TradeType,
    /// Chain ID where swap should be executed
    pub chain_id: ChainId,

    /// Token IN address
    pub src_token: String,
    /// Token OUT address
    pub dest_token: String,
    /// Amount IN for exact IN trade and amount OUT for exact OUT trade
    pub amount_fixed: u128,
    /// Decimal slippage
    pub slippage: Slippage,
}

impl From<GenericSwapRequest> for GenericEstimateRequest {
    fn from(request: GenericSwapRequest) -> Self {
        Self {
            trade_type: request.trade_type,
            chain_id: request.chain_id,
            src_token: request.src_token,
            dest_token: request.dest_token,
            amount_fixed: request.amount_fixed,
            slippage: request.slippage,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenericEstimateResponse {
    /// Amount IN for exact OUT trade or amount OUT for exact IN trade
    pub amount_quote: u128,
    /// Amount IN MAX for exact OUT trade or amount OUT MIN for exact IN trade
    pub amount_limit: u128,
    /// Router type used for the swap
    pub router: RouterType,
    /// Response data specific to router
    pub router_data: serde_json::Value,
}
