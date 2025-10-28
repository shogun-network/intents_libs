use std::collections::{HashMap, HashSet};

use intents_models::constants::chains::{ChainId, ChainType};

use crate::error::EstimatorResult;

pub mod codex;
pub mod estimating;
pub mod gecko_terminal;

pub type TokensPriceData = HashMap<TokenId, TokenPrice>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TokenId {
    pub chain: ChainId,
    pub address: String,
}
// TODO: Normalize addresses to match similar ones with different casing or with/without 0x prefix...

impl TokenId {
    pub fn new(chain: ChainId, address: String) -> Self {
        match chain.to_chain_type() {
            ChainType::EVM => Self {
                chain,
                address: address.to_lowercase(),
            },
            _ => Self { chain, address },
        }
    }
}

// Event that is emitted for every price update observed on WS
#[derive(Debug, Clone)]
pub struct PriceEvent {
    pub token: TokenId,
    pub price: TokenPrice,
}

#[derive(Debug, Clone)]
pub struct TokenMetadata {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

#[derive(Debug, Clone)]
pub struct TokenPrice {
    pub price: f64,
    pub decimals: u8,
}

impl TokenPrice {
    pub fn default() -> Self {
        Self {
            decimals: 0,
            price: 0.0,
        }
    }
}

#[async_trait::async_trait]
pub trait PriceProvider {
    async fn get_tokens_price(
        &self,
        tokens: HashSet<TokenId>,
    ) -> EstimatorResult<HashMap<TokenId, TokenPrice>>;

    async fn get_tokens_prices_events(
        &self,
    ) -> EstimatorResult<tokio::sync::broadcast::Receiver<PriceEvent>>;

    async fn subscribe_to_token(&self, token: TokenId) -> EstimatorResult<()>;

    async fn unsubscribe_from_token(&self, token: TokenId) -> EstimatorResult<bool>;
}
