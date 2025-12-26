use serde::Deserialize;
use tokio::sync::watch;

use crate::prices::{TokenId, TokenPrice};

#[derive(Debug, Clone)]
pub struct TokenSubscription {
    pub token: TokenId,
    pub updates_tx: watch::Sender<Option<TokenPrice>>,
    pub ref_count: usize,
}

#[derive(Debug, Deserialize)]
pub struct CodexGraphqlResponse<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
pub struct CodexGetTrendingTokensData {
    #[serde(rename = "filterTokens")]
    pub filter_tokens: CodexTrendingTokens,
}

#[derive(Debug, Deserialize)]
pub struct CodexTrendingTokens {
    pub results: Vec<TrendingTokenData>,
}

#[derive(Debug, Deserialize)]
pub struct TrendingTokenData {
    pub token: CodexMetadataPayload,
    #[serde(rename = "marketCap")]
    pub market_cap: String,
    pub liquidity: String, // Quoted number
    pub holders: i64,
    #[serde(rename = "volume24")]
    pub volume_24: String, // Quoted number
    #[serde(rename = "walletAgeAvg")]
    pub wallet_age_avg: String, // Quoted float
    #[serde(rename = "buyCount24")]
    pub buy_count_24: u64,
}

#[derive(Debug, Deserialize)]
struct CodexTokenInfo {}

#[derive(Debug, Deserialize)]
pub struct CodexGetPricesAndMetaData {
    pub prices: Vec<Option<CodexPricePayload>>,
    pub meta: Vec<Option<CodexMetadataPayload>>,
}

#[derive(Debug, Deserialize)]
pub struct CodexGetPricesData {
    pub prices: Vec<Option<CodexPricePayload>>,
}

#[derive(Debug, Deserialize)]
pub struct CodexGetMetadataData {
    pub meta: Vec<Option<CodexMetadataPayload>>,
}

#[derive(Debug, Deserialize)]
pub struct CodexPricePayload {
    pub address: String,
    #[serde(rename = "priceUsd")]
    pub price_usd: f64,
    #[serde(rename = "networkId")]
    pub network_id: i64,
    pub timestamp: u64,
}

#[derive(Debug, Deserialize)]
pub struct CodexMetadataPayload {
    pub address: String,
    #[serde(rename = "networkId")]
    pub network_id: i64,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

#[derive(Debug, Deserialize)]
pub struct GraphqlWsMessage {
    #[serde(rename = "type")]
    pub message_type: String,
    pub id: Option<String>,
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct NextPayload {
    pub data: Option<NextData>,
    pub errors: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
pub struct NextData {
    #[serde(rename = "onPriceUpdated")]
    pub on_price_updated: Option<OnPriceUpdated>,
}

#[derive(Debug, Deserialize)]
pub struct OnPriceUpdated {
    #[serde(rename = "priceUsd")]
    pub price_usd: f64,
}
