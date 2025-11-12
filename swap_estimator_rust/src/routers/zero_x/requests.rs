use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZeroXGetPriceRequest {
    pub chain_id: u32,
    pub buy_token: String,
    pub sell_token: String,
    pub sell_amount: String,
    pub slippage_bps: u32, // integer [ 0 .. 10000 ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZeroXGetQuoteRequest {
    pub chain_id: u32,
    pub buy_token: String,
    pub sell_token: String,
    pub sell_amount: String,
    pub slippage_bps: u32, // integer [ 0 .. 10000 ]
    pub taker: String,
    pub tx_origin: Option<String>,
    pub recipient: Option<String>,
}
