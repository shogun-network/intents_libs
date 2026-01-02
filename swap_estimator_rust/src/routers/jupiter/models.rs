use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// QUOTE
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SwapInfo {
    ammKey: String,
    label: String,
    inputMint: String,
    outputMint: String,
    inAmount: String,
    outAmount: String,
    feeAmount: String,
    feeMint: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RoutePlan {
    swapInfo: SwapInfo,
    percent: u64,
    bps: Option<u64>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QuoteResponse {
    pub inAmount: String,
    pub outAmount: String,
    pub otherAmountThreshold: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MostReliableAmmsQuoteReportInfo {
    pub info: HashMap<String, String>,
}

impl Default for QuoteResponse {
    fn default() -> Self {
        QuoteResponse {
            inAmount: String::new(),
            outAmount: String::new(),
            otherAmountThreshold: String::new(),
        }
    }
}

#[allow(non_snake_case)]
#[derive(Deserialize, Debug)]
pub struct JupiterSwapResponse {
    pub swapTransaction: String,
    pub computeUnitLimit: u32,
}

#[derive(Debug)]
pub enum SwapMode {
    ExactIn,
    ExactOut,
}

impl SwapMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            SwapMode::ExactIn => "ExactIn",
            SwapMode::ExactOut => "ExactOut",
        }
    }
}
