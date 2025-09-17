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
pub struct GetPoolsInfo {
    pub data: Vec<Pool>,
    pub id: String,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Pool {
    Cpmm(CpmmPool),
    Clmm(ClmmPool),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CpmmPool {
    pub authority: String,
    pub config: CpmmConfig,
    pub id: String,
    pub lookup_table_account: String,
    pub mint_a: MintInfo,
    pub mint_b: MintInfo,
    #[serde(default)]
    pub mint_lp: Option<MintInfo>,
    pub observation_id: String,
    // openTime viene como string ("1757531658"); mantenlo String o deserializa desde str.
    pub open_time: String,
    pub program_id: String,
    pub vault: VaultAB,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CpmmConfig {
    pub create_pool_fee: String,
    pub creator_fee_rate: u64,
    pub fund_fee_rate: u64,
    pub id: String,
    pub index: u64,
    pub protocol_fee_rate: u64,
    pub trade_fee_rate: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClmmPool {
    pub config: ClmmConfig,
    pub ex_bitmap_account: String,
    pub id: String,
    pub lookup_table_account: String,
    pub mint_a: MintInfo,
    pub mint_b: MintInfo,
    pub observation_id: String,
    pub open_time: String,
    pub program_id: String,
    #[serde(default)]
    pub reward_infos: Vec<serde_json::Value>,
    pub vault: VaultAB,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClmmConfig {
    pub default_range: f64,
    pub default_range_point: Vec<f64>,
    pub fund_fee_rate: u64,
    pub id: String,
    pub index: u64,
    pub protocol_fee_rate: u64,
    pub tick_spacing: u64,
    pub trade_fee_rate: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MintInfo {
    pub address: String,
    pub chain_id: u64,
    pub decimals: u8,
    pub extensions: serde_json::Value,
    #[serde(rename = "logoURI")]
    pub logo_uri: String,
    pub name: String,
    pub program_id: String,
    pub symbol: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultAB {
    #[serde(rename = "A")]
    pub a: String,
    #[serde(rename = "B")]
    pub b: String,
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
    pub referrer_amount: Option<String>,
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
