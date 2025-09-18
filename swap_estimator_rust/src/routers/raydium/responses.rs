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
    AmmV4(AmmV4Pool),
    AmmV5(AmmV5Pool),
}

impl Pool {
    pub fn get_mints(&self) -> (MintInfo, MintInfo) {
        match &self {
            Pool::Cpmm(cpmm) => (cpmm.base.mint_a.clone(), cpmm.base.mint_b.clone()),
            Pool::Clmm(clmm) => (clmm.base.mint_a.clone(), clmm.base.mint_b.clone()),
            Pool::AmmV4(amm_v4) => (amm_v4.base.mint_a.clone(), amm_v4.base.mint_b.clone()),
            Pool::AmmV5(amm_v5) => (amm_v5.base.mint_a.clone(), amm_v5.base.mint_b.clone()),
        }
    }

    pub fn get_program_id(&self) -> String {
        match &self {
            Pool::Cpmm(cpmm) => cpmm.base.program_id.clone(),
            Pool::Clmm(clmm) => clmm.base.program_id.clone(),
            Pool::AmmV4(amm_v4) => amm_v4.base.program_id.clone(),
            Pool::AmmV5(amm_v5) => amm_v5.base.program_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PoolBase {
    pub program_id: String,
    pub id: String,
    pub mint_a: MintInfo,
    pub mint_b: MintInfo,
    pub lookup_table_account: Option<String>,
    pub open_time: String,
    pub vault: VaultAB,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmmBase {
    pub authority: String,
    pub open_orders: String,
    pub target_orders: String,
    pub mint_lp: MintInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketBase {
    pub market_program_id: String,
    pub market_id: String,
    pub market_authority: String,
    pub market_base_vault: String,
    pub market_quote_vault: String,
    pub market_bids: String,
    pub market_asks: String,
    pub market_event_queue: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmmV4Pool {
    #[serde(flatten)]
    pub base: PoolBase,
    #[serde(flatten)]
    pub amm_base: AmmBase,
    #[serde(flatten)]
    pub market_base: MarketBase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmmV5Pool {
    #[serde(flatten)]
    pub base: PoolBase,
    #[serde(flatten)]
    pub amm_base: AmmBase,
    #[serde(flatten)]
    pub market_base: MarketBase,
    pub model_data_account: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CpmmPool {
    #[serde(flatten)]
    pub base: PoolBase,
    pub authority: String,
    pub config: CpmmConfig,
    #[serde(default)]
    pub mint_lp: Option<MintInfo>,
    pub observation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CpmmConfig {
    pub create_pool_fee: String,
    pub creator_fee_rate: Option<u64>,
    pub fund_fee_rate: u64,
    pub id: String,
    pub index: u64,
    pub protocol_fee_rate: u64,
    pub trade_fee_rate: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClmmPool {
    #[serde(flatten)]
    pub base: PoolBase,
    pub config: ClmmConfig,
    pub ex_bitmap_account: String,
    #[serde(default)]
    pub reward_infos: Vec<serde_json::Value>,
    pub observation_id: String,
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
    pub description: Option<String>,
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
    pub freeze_authority: Option<String>,
    pub mint_authority: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutePlan {
    pub fee_amount: String,
    pub fee_mint: String,
    pub fee_rate: u64,
    pub input_mint: String,
    pub output_mint: String,
    pub pool_id: String,
    pub remaining_accounts: Vec<String>,
    pub last_pool_price_x64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutePlans(pub Vec<RoutePlan>);

impl RoutePlans {
    pub fn get_pool_ids(&self) -> Vec<String> {
        self.0.iter().map(|plan| plan.pool_id.clone()).collect()
    }
}
