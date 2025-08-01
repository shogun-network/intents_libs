use crate::{
    error::{Error, EstimatorResult},
    prices::{
        PriceProvider, TokenId, TokenPrice,
        defillama::DefiLlamaChain as _,
        gecko_terminal::{
            GECKO_TERMINAL_API_URL,
            responses::{
                GeckoTerminalOkResponseType, GeckoTerminalResponse, GeckoTerminalTokensInfo,
            },
        },
    },
};
use error_stack::{ResultExt as _, report};
use intents_models::{constants::chains::ChainId, network::http::handle_reqwest_response};
use reqwest::Client;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct GeckoTerminalProvider {
    client: Client,
}

impl GeckoTerminalProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl PriceProvider for GeckoTerminalProvider {
    async fn get_tokens_price(
        &self,
        tokens: HashSet<TokenId>,
    ) -> EstimatorResult<HashMap<TokenId, TokenPrice>> {
        // Convert argument and call for each chain
        let mut tokens_id_by_chain: HashMap<ChainId, Vec<String>> = HashMap::new();

        for token_id in tokens {
            tokens_id_by_chain
                .entry(token_id.chain)
                .and_modify(|map| map.push(token_id.address.clone()))
                .or_insert(vec![token_id.address]);
        }

        // Call token info endpoint function
        let mut tokens_info = HashMap::new();
        for (chain, tokens_address) in tokens_id_by_chain {
            let gt_tokens_info =
                gecko_terminal_get_tokens_info(&self.client, chain, tokens_address).await?;

            // Convert into result struct the response data and add it to the result
            for gt_token_info in gt_tokens_info.into_iter() {
                let token_id = TokenId {
                    chain,
                    address: gt_token_info.attributes.address.clone(),
                };
                let token_price = TokenPrice {
                    price: gt_token_info
                        .attributes
                        .price_usd
                        .parse::<f64>()
                        .change_context(Error::ParseError)
                        .attach_printable_lazy(|| {
                            format!(
                                "Failed to parse geckoterminal price as f64: {:?}",
                                gt_token_info.attributes
                            )
                        })?,
                    decimals: gt_token_info.attributes.decimals,
                };
                tokens_info.insert(token_id, token_price);
            }
        }

        Ok(tokens_info)
    }
}

// pub async fn gecko_terminal_get_tokens_prices(
//     client: &Client,
//     chain_id: ChainId,
//     tokens_address: Vec<String>,
// ) -> EstimatorResult<GeckoTerminalTokensPriceResponse> {
//     let url = format!(
//         "{}/simple/networks/{}/token_price/{}",
//         GECKO_TERMINAL_API_URL,
//         chain_id.to_defillama_chain_name(),
//         tokens_address.join(",")
//     );

//     let response = client
//         .get(&url)
//         .send()
//         .await
//         .change_context(Error::ReqwestError)
//         .attach_printable("Error in gecko terminal request")?;

//     let tokens_response: GeckoTerminalTokensPriceResponse = handle_reqwest_response(response)
//         .await
//         .change_context(Error::ModelsError)?;

//     Ok(tokens_response)
// }

pub async fn gecko_terminal_get_tokens_info(
    client: &Client,
    chain_id: ChainId,
    tokens_address: Vec<String>,
) -> EstimatorResult<Vec<GeckoTerminalTokensInfo>> {
    let url = format!(
        "{}/networks/{}/tokens/multi/{}",
        GECKO_TERMINAL_API_URL,
        chain_id.to_defillama_chain_name(),
        tokens_address.join(",")
    );

    let response = client
        .get(&url)
        .send()
        .await
        .change_context(Error::ReqwestError)
        .attach_printable("Error in gecko terminal request")?;

    let tokens_response: GeckoTerminalResponse = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;

    if let GeckoTerminalOkResponseType::TokensInfo(tokens_info) =
        handle_gecko_terminal_response(tokens_response)?
    {
        // TODO: Revisar que parte es un vector
        Ok(tokens_info)
    } else {
        tracing::error!("Unexpected response in gecko terminal request");
        Err(report!(Error::ResponseError)
            .attach_printable("Unexpected response in gecko terminal request"))
    }
}

fn handle_gecko_terminal_response(
    response: GeckoTerminalResponse,
) -> EstimatorResult<GeckoTerminalOkResponseType> {
    match response {
        GeckoTerminalResponse::Ok(gecko_terminal_ok_response) => {
            Ok(gecko_terminal_ok_response.data)
        }
        GeckoTerminalResponse::Error(gecko_terminal_error_response) => {
            tracing::error!(
                "Error in gecko terminal request: {:?}",
                gecko_terminal_error_response.errors
            );
            Err(report!(Error::ResponseError).attach_printable("Error"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gecko_terminal_get_tokens_price() {
        let gt_provider = GeckoTerminalProvider::new();

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

        let tokens_info = gt_provider
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
