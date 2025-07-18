use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LiquidswapRequest {
    #[serde(untagged)]
    GetPriceRoute(GetPriceRouteRequest),
    #[serde(untagged)]
    GetTokenList(GetTokenListRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPriceRouteRequest {
    /// Address of the input token
    pub token_in: String,

    /// Address of the output token
    pub token_out: String,

    /// Amount of input token (human readable, e.g., 1.5)
    pub amount_in: Option<f64>,

    /// Desired output amount (for reverse routing)
    pub amount_out: Option<f64>,

    /// Set to "true" to enable multi-hop routing
    pub multi_hop: Option<bool>,

    /// Comma-separated list of router indices to exclude
    pub exclude_dexes: Option<String>,

    /// Automatically unwrap WHYPE to native HYPE
    #[serde(rename = "unwrapWHYPE")]
    pub unwrap_whype: Option<bool>,

    // Approve and use WHYPE tokens
    #[serde(rename = "useNativeHYPE")]
    pub use_native_hype: Option<bool>,

    /// Slippage tolerance in percentage (e.g., 0.5 for 0.5%)
    pub slippage: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTokenListRequest {
    /// Filter tokens by address, name, or symbol
    pub search: Option<String>,

    /// Maximum number of tokens to return
    pub limit: Option<u32>,

    /// When "false", returns only addresses
    pub metadata: Option<bool>,
}
