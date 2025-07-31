use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;
use serde_with::{DisplayFromStr, serde_as};

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum GeckoTerminalResponse {
    Ok(GeckoTerminalOkResponse),
    Error(GeckoTerminalErrorResponse),
}

#[derive(Debug, Deserialize)]
pub struct GeckoTerminalOkResponse {
    pub data: GeckoTerminalPricesData,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum GeckoTerminalOkResponseType {
    Prices(GeckoTerminalPricesData),
    TokensInfo(GeckoTerminalTokensInfo),
}

#[derive(Debug, Deserialize)]
pub struct GeckoTerminalErrorResponse {
    pub errors: Vec<GeckoTerminalError>,
}

#[derive(Debug, Deserialize)]
pub struct GeckoTerminalError {
    pub status: String,
    pub title: String,
}

// Prices Responses

#[derive(Debug, Deserialize)]
pub struct GeckoTerminalPricesData {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub attributes: GeckoTerminalPricesAttributes,
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct GeckoTerminalPricesAttributes {
    #[serde_as(as = "HashMap<_, DisplayFromStr>")]
    token_prices: HashMap<String, f64>,
}

// Token Info Responses

#[derive(Debug, Deserialize)]
pub struct GeckoTerminalTokensInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub attributes: GeckoTerminalTokensInfoAttributes,
}

#[derive(Debug, Deserialize)]
pub struct GeckoTerminalTokensInfoAttributes {
    pub name: String,
    pub address: String,
    pub symbol: String,
    pub decimals: u8,
    pub total_supply: String,
    pub coingecko_coin_id: String,
    pub price_usd: String,
    pub fdv_usd: String,
    pub total_reserve_in_usd: String,
    pub volume_usd: Value,
    pub market_cap_usd: String,
}
