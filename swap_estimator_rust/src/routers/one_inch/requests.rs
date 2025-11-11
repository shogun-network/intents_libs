use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneInchGetQuoteRequest {
    pub chain: u32,
    pub src: String,
    pub dst: String,
    pub amount: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneInchSwapRequest {
    pub chain: u32,
    pub src: String,
    pub dst: String,
    pub amount: String,
    pub from: String,
    pub origin: String,
    pub slippage: Option<u32>, // Slippage tolerance in percent. Min: 0; Max: 50.
    pub min_return: Option<String>, // Use either slippage or minReturn, not both.
}
