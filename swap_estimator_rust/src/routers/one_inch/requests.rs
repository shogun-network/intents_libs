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
    /// contract address of a token to sell
    pub src: String,
    /// contract address of a token to buy
    pub dst: String,
    /// amount of a token to sell, set in minimal divisible units
    pub amount: String,
    /// address of a seller, make sure that this address has approved to spend src
    /// in needed amount
    pub from: String,
    /// The EOA address which initiates the transaction (for compliance KYC/AML)
    pub origin: String,
    /// limit of price slippage you are willing to accept in percentage,
    /// may be set with decimals. &slippage=0.5 means 0.5% slippage is acceptable.
    /// Low values increase chances that transaction will fail,
    /// high values increase chances of front running. Set values in the range from 0 to 50
    pub slippage: Option<f64>, // Slippage tolerance in percent. Min: 0; Max: 50.
    pub min_return: Option<String>, // Use either slippage or minReturn, not both.
    /// recipient address of a purchased token
    /// if not set, from will receive a purchased token
    pub receiver: Option<String>,
}
