use std::collections::{HashMap, HashSet};

use intents_models::constants::chains::ChainId;

use crate::error::EstimatorResult;

pub mod defillama;
pub mod estimating;
pub mod gecko_terminal;

pub type TokensPriceData = HashMap<TokenId, TokenPrice>;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct TokenId {
    pub chain: ChainId,
    pub address: String,
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
}
