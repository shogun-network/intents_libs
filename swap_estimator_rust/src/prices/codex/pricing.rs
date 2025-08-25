use std::collections::{HashMap, HashSet};

use error_stack::{ResultExt as _, report};
use graphql_client::{GraphQLQuery as _, Response};
use intents_models::{constants::chains::ChainId, network::http::handle_reqwest_response};
use reqwest::Client;

use crate::{
    error::{Error, EstimatorResult},
    prices::{
        PriceProvider, TokenId, TokenPrice,
        codex::{
            CODEX_API_URL, CodexChain, TokensWithPrices,
            tokens_with_prices::{self, TokenInput},
        },
    },
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
        let mut price_inputs = Vec::new();
        let mut token_inputs = Vec::new();

        for token in tokens.into_iter() {
            price_inputs.push(tokens_with_prices::GetPriceInput {
                address: token.address.clone(),
                network_id: token.chain.to_codex_chain_number(),
                max_deviations: None,
                pool_address: None,
                timestamp: None,
            });
            token_inputs.push(TokenInput {
                address: token.address.clone(),
                network_id: token.chain.to_codex_chain_number(),
            });
        }
        let variables = tokens_with_prices::Variables {
            price_inputs: Some(price_inputs),
            token_inputs: Some(token_inputs),
        };
        let request_body = TokensWithPrices::build_query(variables);

        let response = self
            .client
            .post(CODEX_API_URL)
            .header("Authorization", self.api_key.clone())
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .change_context(Error::ReqwestError)
            .attach_printable("Error in gecko terminal request")?;

        let response_body: Response<tokens_with_prices::ResponseData> =
            handle_reqwest_response(response)
                .await
                .change_context(Error::ReqwestError)
                .attach_printable("Error in gecko terminal request")?;

        match response_body.data {
            Some(data) => {
                let Some(tokens_prices) = data.prices else {
                    return Err(report!(Error::ResponseError)
                        .attach_printable("No prices in response from Codex".to_string()));
                };
                let tokens_metadata = data.meta;

                if tokens_prices.len() != tokens_metadata.len() {
                    return Err(report!(Error::ResponseError).attach_printable(format!(
                        "Codex returned mismatched lengths: prices={}, meta={}",
                        tokens_prices.len(),
                        tokens_metadata.len()
                    )));
                }

                let mut result = HashMap::new();
                for (price_opt, meta_opt) in
                    tokens_prices.into_iter().zip(tokens_metadata.into_iter())
                {
                    if let (Some(price_data), Some(meta_data)) = (price_opt, meta_opt) {
                        let token_id = TokenId {
                            address: price_data.address,
                            chain: ChainId::from_codex_chain_number(price_data.network_id)
                                .ok_or_else(|| {
                                    report!(Error::ChainError(format!(
                                        "Unknown chain number: {}",
                                        price_data.network_id
                                    )))
                                })?,
                        };
                        let token_price = TokenPrice {
                            decimals: match meta_data.decimals.try_into() {
                                Ok(decimals) => decimals,
                                Err(_) => {
                                    tracing::error!(
                                        "Invalid decimals value: {} for token: {}, chain: {}",
                                        meta_data.decimals,
                                        token_id.address,
                                        token_id.chain
                                    );
                                    continue;
                                }
                            },
                            price: price_data.price_usd,
                        };
                        result.insert(token_id, token_price);
                    }
                }
                Ok(result)
            }
            None => Err(report!(Error::ResponseError)
                .attach_printable("No data in response from Codex".to_string())),
        }
    }
}

#[cfg(test)]
pub mod test {
    use intents_models::constants::chains::ChainId;

    use super::*;

    #[tokio::test]
    async fn test_codex_get_tokens_price() {
        dotenv::dotenv().ok();
        let codex_provider = CodexProvider::new(std::env::var("CODEX_API_KEY").unwrap());

        let tokens = HashSet::from([
            TokenId {
                chain: ChainId::Solana,
                address: "So11111111111111111111111111111111111111112".to_string(),
            },
            TokenId {
                chain: ChainId::Base,
                address: "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string(),
            },
        ]);

        let tokens_info = codex_provider
            .get_tokens_price(tokens)
            .await
            .expect("Failed to get tokens price");
        println!("Tokens Info: {:?}", tokens_info);
        // Check that we got data for both tokens
        assert_eq!(tokens_info.len(), 2);
        assert!(tokens_info.contains_key(&TokenId {
            chain: ChainId::Solana,
            address: "So11111111111111111111111111111111111111112".to_string(),
        }));
        assert!(tokens_info.contains_key(&TokenId {
            chain: ChainId::Base,
            address: "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string(),
        }));
        // Check that the prices are valid
        let sol_token_price = tokens_info
            .get(&TokenId {
                chain: ChainId::Solana,
                address: "So11111111111111111111111111111111111111112".to_string(),
            })
            .unwrap();
        assert!(sol_token_price.price > 0.0);
        let base_token_price = tokens_info
            .get(&TokenId {
                chain: ChainId::Base,
                address: "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string(),
            })
            .unwrap();
        assert!(base_token_price.price > 0.0);
    }
}
