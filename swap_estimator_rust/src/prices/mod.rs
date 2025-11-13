use std::collections::HashMap;

use intents_models::constants::chains::{ChainId, ChainType};
use serde::{Deserialize, Serialize};

use crate::{error::EstimatorResult, prices::codex::CodexChain as _};

pub mod codex;
pub mod defillama;
pub mod estimating;
pub mod gecko_terminal;

pub type TokensPriceData = HashMap<TokenId, TokenPrice>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
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

    pub fn new_for_codex(chain: ChainId, address: &str) -> Self {
        let codex_address = chain.to_codex_address(address);
        Self::new(chain, codex_address)
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

#[derive(Debug, Clone, Copy)]
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
        tokens: &[TokenId],
        with_subscriptions: bool,
    ) -> EstimatorResult<HashMap<TokenId, TokenPrice>>;

    async fn get_tokens_prices_events(
        &self,
    ) -> EstimatorResult<tokio::sync::broadcast::Receiver<PriceEvent>>;

    async fn subscribe_to_token(&self, token: TokenId) -> EstimatorResult<()>;

    async fn unsubscribe_from_token(&self, token: TokenId) -> EstimatorResult<bool>;
}
