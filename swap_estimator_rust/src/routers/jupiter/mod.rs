#[allow(clippy::module_inception)]
pub mod jupiter;
pub mod models;

pub fn get_jupiter_max_slippage() -> u64 {
    10000 // 100%
}
