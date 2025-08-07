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
    pub data: GeckoTerminalOkResponseType,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum GeckoTerminalOkResponseType {
    Prices(GeckoTerminalPricesData),
    TokensInfo(Vec<GeckoTerminalTokensInfo>),
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
    pub token_prices: HashMap<String, f64>,
}

// Token Info Responses

#[derive(Debug, Deserialize)]
pub struct GeckoTerminalTokensInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub attributes: GeckoTerminalTokensInfoAttributes,
    pub relationships: Value,
}

#[derive(Debug, Deserialize)]
pub struct GeckoTerminalTokensInfoAttributes {
    pub name: String,
    pub address: String,
    pub symbol: String,
    pub decimals: u8,
    pub total_supply: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coingecko_coin_id: Option<String>,
    pub price_usd: String,
    pub fdv_usd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_cap_usd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_reserve_in_usd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_total_supply: Option<String>,
    pub volume_usd: Value,
}
