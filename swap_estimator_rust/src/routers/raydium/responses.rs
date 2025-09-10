use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RaydiumSwapType {
    BaseIn,
    BaseOut,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaydiumResponse {
    pub id: String,
    pub version: String,
    pub success: bool,
    pub data: Option<RaydiumResponseData>,
    pub msg: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RaydiumResponseData {
    GetPriceRoute(SwapResponseData),
    SwapTransactions(Vec<Transaction>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponseData {
    pub input_amount: String,
    pub input_mint: String,
    pub other_amount_threshold: String,
    pub output_amount: String,
    pub output_mint: String,
    pub price_impact_pct: f64,
    pub referrer_amount: String,
    pub route_plan: Value,
    pub slippage_bps: u32,
    pub swap_type: RaydiumSwapType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub transaction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityFeeResponse {
    pub id: String,
    pub success: bool,
    pub data: PriorityFeeData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityFeeData {
    pub default: PriorityFeeDataDefault,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityFeeDataDefault {
    pub h: u64,
    pub m: u64,
    pub vh: u64,
}
