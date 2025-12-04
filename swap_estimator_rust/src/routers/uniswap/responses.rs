use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
pub struct UniswapQuoteInput {
    pub token: String,
    pub amount: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UniswapQuoteOutput {
    pub token: String,
    pub amount: String,
    pub recipient: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniswapTransaction {
    pub to: String,
    pub from: String,
    pub data: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UniswapQuoteValue {
    pub input: UniswapQuoteInput,
    pub output: UniswapQuoteOutput,
}

// https://api-docs.uniswap.org/api-reference/swapping/quote
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UniswapQuoteResponse {
    pub quote: Value,
    pub permit_transaction: Option<UniswapTransaction>,
}

// https://api-docs.uniswap.org/api-reference/swapping/create_protocol_swap
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UniswapSwapResponse {
    // A unique ID for the request.
    pub swap: UniswapTransaction,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum UniswapResponse {
    Quote(UniswapQuoteResponse),
    Swap(UniswapSwapResponse),
    UnknownResponse(Value),
}
