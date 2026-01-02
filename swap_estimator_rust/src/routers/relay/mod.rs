use intents_models::constants::chains::{
    ChainId, is_native_token_evm_address, is_native_token_solana_address,
};

pub mod evm;
pub mod relay;
pub mod requests;
pub mod responses;

pub fn update_relay_native_token(token_address: String) -> String {
    if is_native_token_evm_address(&token_address) {
        "0x0000000000000000000000000000000000000000".to_string()
    } else if is_native_token_solana_address(&token_address) {
        "11111111111111111111111111111111".to_string()
    } else {
        token_address
    }
}

pub fn update_relay_chain_id(chain_id: ChainId) -> u32 {
    // https://docs.relay.link/references/api/api_resources/supported-chains
    match chain_id {
        ChainId::Solana => 792703809,
        _ => chain_id as u32,
    }
}

pub fn get_relay_max_slippage() -> u32 {
    10_000 // 100% in basis points
}
