use intents_models::constants::chains::ChainId;

pub mod models;
pub mod pricing;
pub mod utils;

// https://docs.codex.io/api-reference/introduction
pub const CODEX_WS_URL: &str = "wss://graph.codex.io/graphql";
pub const CODEX_HTTP_URL: &str = "https://graph.codex.io/graphql";

pub trait CodexChain {
    fn to_codex_chain_number(self) -> i64;
    fn from_codex_chain_number(number: i64) -> Option<Self>
    where
        Self: Sized;
    fn to_codex_address(self, address: &str) -> String;
}

impl CodexChain for ChainId {
    fn to_codex_chain_number(self) -> i64 {
        match self {
            ChainId::Ethereum => self as i64,
            ChainId::Base => self as i64,
            ChainId::Bsc => self as i64,
            ChainId::ArbitrumOne => self as i64,
            ChainId::Optimism => self as i64,
            ChainId::Monad => self as i64,
            ChainId::Solana => 1399811149,
            ChainId::Sui => self as i64,
            ChainId::HyperEVM => self as i64,
        }
    }

    fn from_codex_chain_number(number: i64) -> Option<ChainId> {
        match number {
            1 => Some(ChainId::Ethereum),
            8453 => Some(ChainId::Base),
            56 => Some(ChainId::Bsc),
            42161 => Some(ChainId::ArbitrumOne),
            10 => Some(ChainId::Optimism),
            1399811149 => Some(ChainId::Solana),
            101 => Some(ChainId::Sui),
            999 => Some(ChainId::HyperEVM),
            _ => None,
        }
    }

    fn to_codex_address(self, address: &str) -> String {
        if self.is_native_token(&address) {
            if let ChainId::Sui = self {
                return "0x0000000000000000000000000000000000000000000000000000000000000002::sui::SUI".to_string();
            }
            let wrapped_address = self.wrapped_native_token_address();
            wrapped_address
        } else {
            address.to_string()
        }
    }
}
