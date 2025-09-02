use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::requests::ParaswapSide;

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
    #[serde(rename = "blockNumber")]
    pub block_number: u64,
    pub network: u32,
    #[serde(rename = "srcToken")]
    pub src_token: String,
    #[serde(rename = "srcDecimals")]
    pub src_decimals: u8,
    #[serde(rename = "srcAmount")]
    pub src_amount: String,
    #[serde(rename = "destToken")]
    pub dest_token: String,
    #[serde(rename = "destDecimals")]
    pub dest_decimals: u8,
    #[serde(rename = "destAmount")]
    pub dest_amount: String,
    #[serde(rename = "bestRoute")]
    pub best_route: Vec<RouteSegment>,
    #[serde(rename = "gasCostUSD")]
    pub gas_cost_usd: String,
    #[serde(rename = "gasCost")]
    pub gas_cost: String,
    pub side: ParaswapSide,
    pub version: String,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    #[serde(rename = "tokenTransferProxy")]
    pub token_transfer_proxy: String,
    #[serde(rename = "contractMethod")]
    pub contract_method: String,
    #[serde(rename = "partnerFee")]
    pub partner_fee: u32,
    #[serde(rename = "srcUSD")]
    pub src_usd: String,
    #[serde(rename = "destUSD")]
    pub dest_usd: String,
    pub partner: String,
    #[serde(rename = "maxImpact", skip_serializing_if = "Option::is_none")]
    pub max_impact: Option<u32>,
    #[serde(rename = "maxImpactReached")]
    pub max_impact_reached: bool,
    pub hmac: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteSegment {
    pub percent: u32,
    pub swaps: Vec<Swap>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Swap {
    #[serde(rename = "srcToken")]
    pub src_token: String,
    #[serde(rename = "srcDecimals")]
    pub src_decimals: u8,
    #[serde(rename = "destToken")]
    pub dest_token: String,
    #[serde(rename = "destDecimals")]
    pub dest_decimals: u8,
    #[serde(rename = "swapExchanges")]
    pub swap_exchanges: Vec<SwapExchange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapExchange {
    pub exchange: String,
    #[serde(rename = "srcAmount")]
    pub src_amount: String,
    #[serde(rename = "destAmount")]
    pub dest_amount: String,
    pub percent: f64,
    #[serde(rename = "poolAddresses")]
    pub pool_addresses: Vec<String>,
    #[serde(default)]
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExchangeDataEnum {
    #[serde(untagged)]
    ExchangeData(ExchangeData),
    #[serde(untagged)]
    Unknown(Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeData {
    pub path: Vec<PathStep>,
    #[serde(rename = "gasUSD")]
    pub gas_usd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathStep {
    #[serde(rename = "tokenIn")]
    pub token_in: String,
    #[serde(rename = "tokenOut")]
    pub token_out: String,
    #[serde(rename = "tickSpacing", skip_serializing_if = "Option::is_none")]
    pub tick_spacing: Option<String>,
    pub fee: String,
    #[serde(rename = "currentFee", skip_serializing_if = "Option::is_none")]
    pub current_fee: Option<String>,
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
