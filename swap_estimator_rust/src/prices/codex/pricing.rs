use std::collections::{HashMap, HashSet};

use reqwest::Client;

use crate::{
    error::EstimatorResult,
    prices::{PriceProvider, TokenId, TokenPrice},
};

#[derive(Debug, Clone)]
pub struct CodexProvider {
    client: Client,
    api_key: String,
}

impl CodexProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }
}

#[async_trait::async_trait]
impl PriceProvider for CodexProvider {
    async fn get_tokens_price(
        &self,
        tokens: HashSet<TokenId>,
    ) -> EstimatorResult<HashMap<TokenId, TokenPrice>> {
    }
}
