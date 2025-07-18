use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_with::skip_serializing_none;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LiquidswapResponse {
    GetTokenList(GetTokenListResponse),
    GetPriceRoute(GetPriceRouteResponse),
    UnknownResponse(Value),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetPriceRouteResponse {
    pub success: bool,
    pub tokens: RouteTokenInfo,
    pub amount_in: String,
    pub amount_out: String,
    pub average_price_impact: String,
    pub execution: RouteExecution,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RouteExecution {
    pub to: String,
    pub calldata: String,
    pub details: RouteDetails,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RouteDetails {
    pub path: Option<Vec<String>>,
    pub amount_in: String,
    pub amount_out: String,
    pub min_amount_out: String,
    pub hop_swaps: Vec<Vec<RouteHopAllocation>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RouteHopAllocation {
    pub token_in: String,
    pub token_out: String,
    pub router_index: u8,
    pub router_name: String,
    pub fee: u32,
    pub amount_in: String,
    pub amount_out: String,
    pub stable: bool,
    pub price_impact: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RouteTokenInfo {
    pub token_in: LiquidswapTokenData,
    pub token_out: LiquidswapTokenData,
    pub intermediate: Option<LiquidswapTokenData>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetTokenListResponse {
    pub success: bool,
    pub data: GetTokenListData,
}

#[skip_serializing_none]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetTokenListData {
    pub tokens: Vec<LiquidswapTokenData>,
    pub count: u32,
    pub limited_count: u32,
    pub search_applied: bool,
    pub limit_applied: bool,
    pub service_status: String,
    pub last_processed_block: Option<u64>,
}

#[skip_serializing_none]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LiquidswapTokenData {
    pub address: String,
    pub name: Option<String>,
    pub symbol: String,
    pub decimals: u8,
    pub transfers24h: Option<u64>,
    pub is_e_r_c20_verified: Option<bool>,
    pub total_transfers: Option<u64>,
}
