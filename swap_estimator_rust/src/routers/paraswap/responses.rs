use serde::{Deserialize, Serialize};
use serde_json::Value;

// https://developers.paraswap.network/api/paraswap-api/paraswap-market-api/build-parameters-for-transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]

pub enum ParaswapResponse {
    Prices(GetPriceRouteResponse),
    Transactions(TransactionsResponse),
    Tokens(TokensResponse),
    RequestError { error: String },
    UnknownResponse(Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokensResponse {
    pub tokens: Vec<TokenInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub symbol: String,
    pub address: String,
    pub decimals: u8,
    pub img: Option<String>,
    pub network: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetPriceRouteResponse {
    #[serde(rename = "priceRoute")]
    pub price_route: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceRoute {
    #[serde(rename = "srcAmount")]
    pub src_amount: String,
    #[serde(rename = "destAmount")]
    pub dest_amount: String,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionsResponse {
    pub from: String,
    pub to: String,
    pub data: String,
    pub value: String,
    #[serde(rename = "gasPrice")]
    pub gas_price: String,
    #[serde(rename = "chainId")]
    pub chain_id: u32,
}
