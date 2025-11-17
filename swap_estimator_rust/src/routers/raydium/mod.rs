pub mod rate_limit;
pub mod raydium;
pub mod requests;
pub mod responses;

// SWAP API URL: https://docs.raydium.io/raydium/traders/trade-api
const SWAP_API_URL: &str = "https://transaction-v1.raydium.io";
const PRIORITY_FEE: &str = "https://api-v3.raydium.io/main/auto-fee";
const BASE_HOST_URL: &str = "https://api-v3.raydium.io";

pub fn get_raydium_format_slippage(slippage: f64) -> u32 {
    (slippage * 100.0) as u32
}
