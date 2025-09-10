use serde::{Deserialize, Serialize};

use crate::routers::raydium::responses::RaydiumResponse;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaydiumGetQuote {
    /// Input token mint address
    pub input_mint: String,
    /// Output token mint address
    pub output_mint: String,
    /// Either inputAmount or outpoutAmount depending on the swap mode.
    pub amount: u128,
    /// Slippage tolerance in base points (0.01%).
    pub slippage_bps: u32,
    pub tx_version: String, // Only V0 works
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaydiumCreateTransaction {
    /// Use 'V0' for versioned transaction, and 'LEGACY' for legacy transaction.
    pub tx_version: String,
    /// Need to be true to accept SOL as inputToken.
    pub wrap_sol: bool,
    /// Need to set to true to unwrap wSol received as outputToken.
    pub unwrap_sol: bool,
    /// The 'h' here stands for high priority.
    /// 'vh' for very high and 'm' for medium are also accepted value.
    pub compute_unit_price_micro_lamports: String,
    /// pubkey
    pub wallet: String,
    /// account always needs to be passed if inputToken ≠ SOL
    pub input_account: String,
    /// default to ATA
    pub output_account: String,
    /// computed by the API, no modification needed.
    pub swap_response: RaydiumResponse,
}
