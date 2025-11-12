use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ZeroXApiResponse {
    GetQuoteResponse(ZeroXGetQuoteResponse),
    GetPriceResponse(ZeroXGetPriceResponse),
    LiquidityResponse(ZeroXIliquidityResponse),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZeroXGetPriceResponse {
    pub buy_amount: String,
    pub min_buy_amount: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZeroXGetQuoteResponse {
    pub buy_amount: String,
    pub min_buy_amount: String,
    pub allowance_target: String,
    pub transaction: ZeroXTransaction,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZeroXTransaction {
    pub to: String,
    pub data: String,
    pub value: String,
    pub gas: Option<String>,
    pub gas_price: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZeroXIliquidityResponse {
    pub liquidity_available: bool,
}
