use intents_models::constants::chains::{
    ChainId, EVM_NULL_ADDRESS, WRAPPED_NATIVE_TOKEN_SOLANA_ADDRESS, is_native_token_evm_address,
    is_native_token_solana_address,
};

pub mod pricing;
pub mod responses;

// https://api-docs.defillama.com/#tag/tvl/get/protocols
pub const DEFILLAMA_COINS_BASE_URL: &str = "https://coins.llama.fi";

pub trait DefiLlamaChain {
    fn to_defillama_chain_name(&self) -> &str;
    fn from_defillama_chain_name(chain_name: &str) -> Option<ChainId>;
    fn to_defillama_format(&self, address: &str) -> String;
}

impl DefiLlamaChain for ChainId {
    fn to_defillama_chain_name(&self) -> &str {
        match self {
            ChainId::Ethereum => "ethereum",
            ChainId::Base => "base",
            ChainId::Bsc => "bsc",
            ChainId::ArbitrumOne => "arbitrum",
            ChainId::Optimism => "optimism",
            ChainId::Monad => "monad",
            ChainId::Solana => "solana",
            ChainId::Sui => "sui",
            ChainId::HyperEVM => "hyperliquid",
        }
    }

    fn from_defillama_chain_name(chain_name: &str) -> Option<ChainId> {
        match chain_name {
            "ethereum" => Some(ChainId::Ethereum),
            "base" => Some(ChainId::Base),
            "bsc" => Some(ChainId::Bsc),
            "arbitrum" => Some(ChainId::ArbitrumOne),
            "optimism" => Some(ChainId::Optimism),
            "solana" => Some(ChainId::Solana),
            "sui" => Some(ChainId::Sui),
            "hyperliquid" => Some(ChainId::HyperEVM),
            _ => None,
        }
    }

    fn to_defillama_format(&self, address: &str) -> String {
        let chain_name = self.to_defillama_chain_name();
        let token_address = {
            if is_native_token_evm_address(address) {
                EVM_NULL_ADDRESS.to_string()
            } else if is_native_token_solana_address(address) {
                WRAPPED_NATIVE_TOKEN_SOLANA_ADDRESS.to_string()
            } else {
                address.to_string()
            }
        };

        let defillama_token_format = format!("{chain_name}:{token_address}");

        defillama_token_format
    }
}
