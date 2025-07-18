use crate::constants::chains::ChainId;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TradeType {
    ExactIn,
    ExactOut,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TokenDataRequest {
    Token(TokenType),
    StablecoinInfo,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TokenType {
    Token(String),
    Stablecoin,
}
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct GenericTokenData {
    pub decimals: u8,
    pub is_whitelisted: bool,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct GasPrices {
    pub gas_price: u128,        // Gas price per unit
    pub gas_price_decimals: u8, // Gas price decimals (e.g. 1 ETH = 10^18 WEI)
    pub chain: ChainId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenericEstimateRequest {
    pub trade_type: TradeType,
    /// Chain ID where swap should be executed
    pub chain_id: ChainId,

    /// Token IN address
    pub src_token: TokenType,
    /// Token OUT address
    pub dest_token: TokenType,
    /// Amount IN for exact IN trade and amount OUT for exact OUT trade
    pub amount_fixed: u128,
    /// Decimal slippage
    pub slippage: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenericEstimateResponse {
    /// Amount IN for exact OUT trade or amount OUT for exact IN trade
    pub amount_quote: u128,
    /// Amount IN MAX for exact OUT trade or amount OUT MIN for exact IN trade
    pub amount_limit: u128,
}
