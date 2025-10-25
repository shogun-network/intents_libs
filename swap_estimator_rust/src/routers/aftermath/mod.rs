#[allow(clippy::module_inception)]
pub mod aftermath;
pub mod responses;

pub const AFTERMATH_BASE_API_URL: &str = "https://aftermath.finance/api";

pub fn get_aftermath_max_slippage() -> f64 {
    100.0 // 100%
}
