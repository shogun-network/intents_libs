use intents_models::constants::chains::ChainId;

pub mod pricing;
// https://docs.codex.io/api-reference/introduction
pub const CODEX_WS_URL: &str = "wss://graph.codex.io/graphql";
pub const CODEX_HTTP_URL: &str = "https://graph.codex.io/graphql";

pub trait CodexChain {
    fn to_codex_chain_number(&self) -> i64;
    fn from_codex_chain_number(number: i64) -> Option<Self>
    where
        Self: Sized;
}

impl CodexChain for ChainId {
    fn to_codex_chain_number(&self) -> i64 {
        match self {
            ChainId::Ethereum => *self as i64,
            ChainId::Base => *self as i64,
            ChainId::Bsc => *self as i64,
            ChainId::ArbitrumOne => *self as i64,
            ChainId::Optimism => *self as i64,
            ChainId::Solana => 1399811149,
            ChainId::Sui => *self as i64,
            ChainId::HyperEVM => *self as i64,
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
            784 => Some(ChainId::Sui),
            123 => Some(ChainId::HyperEVM),
            _ => None,
        }
    }
}
