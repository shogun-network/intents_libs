use intents_models::constants::chains::is_native_token_evm_address;

pub mod rate_limit;
pub mod requests;
pub mod responses;
pub mod uniswap;

pub fn update_uniswap_native_token(token_address: String) -> String {
    if is_native_token_evm_address(&token_address) {
        "0x0000000000000000000000000000000000000000".to_string()
    } else {
        token_address
    }
}