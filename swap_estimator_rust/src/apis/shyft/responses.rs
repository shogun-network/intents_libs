use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ShyftResponse {
    Data { data: ShyftResponseData },
    Error { error: Value },
    Unknown(Value),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
#[allow(non_snake_case)]
pub enum ShyftResponseData {
    PumpPoolData {
        pump_fun_amm_Pool: Vec<PumpPoolData>,
    },
    Unknown(Value),
}

#[derive(Debug, Deserialize)]
pub struct PumpPoolData {
    pub base_mint: String,
    pub creator: String,
    pub index: u64,
    pub lp_mint: String,
    pub lp_supply: u64,
    pub pool_base_token_account: String,
    pub pool_bump: u8,
    pub pool_quote_token_account: String,
    pub quote_mint: String,
    pub pubkey: String,
}
