use intents_models::constants::chains::ChainId;

pub mod estimating;
pub mod pricing;

// https://www.geckoterminal.com/dex-api
// Data Freshness
// All endpoints listed below are cached for 1 minute

// All data is updated as fast as 2-3 seconds after a transaction is confirmed on the blockchain, subject to the network's availability.

// Rate Limit
// Our free API is limited to 30 calls/minute
pub const GECKO_TERMINAL_API_URL: &str = "https://api.geckoterminal.com/api/v2";

pub trait GeckoTerminalChain {
    fn to_gecko_terminal_chain_name(&self) -> &str;
}

impl GeckoTerminalChain for ChainId {
    fn to_gecko_terminal_chain_name(&self) -> &str {
        match self {
            ChainId::Ethereum => "eth",
            ChainId::Base => "base",
            ChainId::Bsc => "bsc",
            ChainId::ArbitrumOne => "arbitrum",
            ChainId::Optimism => "optimism",
            ChainId::Solana => "solana",
            ChainId::Sui => "sui-network",
            ChainId::HyperEVM => "hyperevm",
        }
    }
}
