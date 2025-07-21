use crate::routers::estimate::TradeType;
use intents_models::constants::chains::ChainId;

#[derive(Debug, Clone)]
pub struct GenericSwapRequest {
    pub trade_type: TradeType,
    /// Chain ID where swap should be executed
    pub chain_id: ChainId,
    /// Address of wallet/smart contract that will spend tokens
    pub spender: String,
    /// Tokens OUT receiver
    pub dest_address: String,

    /// Token IN address
    pub src_token: String,
    /// Token OUT address
    pub dest_token: String,
    /// Amount IN for exact IN trade or amount OUT for exact OUT trade
    pub amount_fixed: u128,
    /// Decimal slippage
    pub slippage: f64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct GenericSwapResponse {
    /// Amount IN for exact OUT trade or amount OUT for exact IN trade
    pub amount_quote: u128,
    /// Amount IN MAX for exact OUT trade or amount OUT MIN for exact IN trade
    pub amount_limit: u128,

    pub tx_to: String,
    pub tx_data: String,
    pub tx_value: u128,
    pub approve_address: Option<String>,
    /// Does not send tokens to required destination. Requires additional transfer
    pub require_transfer: bool,
}

#[derive(Copy, Clone)]
pub enum SolanaPriorityFeeType {
    /// (lamports)
    JitoTip(u64),
    /// (max lamports)
    PriorityFee(u64),
}
