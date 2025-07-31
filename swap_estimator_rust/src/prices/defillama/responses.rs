use std::collections::HashMap;

use intents_models::constants::chains::ChainId;
use serde::{Deserialize, Serialize};

use crate::prices::defillama::DefiLlamaChain as _;

#[derive(Debug, Deserialize)]
pub struct DefiLlamaTokensResponse {
    pub coins: HashMap<String, DefiLlamaCoinData>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct DefiLlamaCoinData {
    pub decimals: u8,
    pub symbol: String,
    pub price: f64,
    pub timestamp: u32,
    pub confidence: f64,
}

impl DefiLlamaCoinData {
    pub fn default() -> Self {
        Self {
            decimals: 0,
            symbol: String::new(),
            price: 0.0,
            timestamp: 0,
            confidence: 0.0,
        }
    }
}

pub trait DefiLlamaCoinHashMap {
    fn get(&self, token: (ChainId, &str)) -> Option<&DefiLlamaCoinData>;
}

impl DefiLlamaCoinHashMap for DefiLlamaTokensResponse {
    fn get(&self, (chain_id, token): (ChainId, &str)) -> Option<&DefiLlamaCoinData> {
        self.coins.get(&chain_id.to_defillama_format(token))
    }
}
