use crate::error::{Error, EstimatorResult};
use crate::prices::defillama::responses::{DefiLlamaCoinHashMap as _, DefiLlamaTokensResponse};
use crate::prices::defillama::{DEFILLAMA_COINS_BASE_URL, DefiLlamaChain as _};
use crate::prices::{PriceProvider, TokenId, TokenPrice};
use crate::utils::number_conversion::u128_to_f64;
use error_stack::{ResultExt, report};
use intents_models::constants::chains::ChainId;
use intents_models::network::http::handle_reqwest_response;
use reqwest::Client;
use std::collections::{HashMap, HashSet};

const TOKEN_PRICE_URI: &str = "/prices/current/";

#[derive(Debug, Clone)]
pub struct DefiLlamaProvider {
    client: Client,
}

impl DefiLlamaProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl PriceProvider for DefiLlamaProvider {
    async fn get_tokens_price(
        &self,
        tokens: HashSet<TokenId>,
    ) -> EstimatorResult<HashMap<TokenId, TokenPrice>> {
        let defillama_token_response = get_tokens_data(&self.client, tokens).await?;
        let mut tokens_price_data = HashMap::new();

        for (defillama_token_id, token_data) in defillama_token_response.coins {
            let (chain_name, token_address) = defillama_token_id
                .split_once(':')
                .ok_or(Error::ChainError("Invalid Defillama response".to_string()))?;
            let chain_id = ChainId::from_defillama_chain_name(chain_name).ok_or(
                Error::ChainError("Unknown DefiLlama chain name".to_string()),
            )?;
            tokens_price_data.insert(
                TokenId {
                    chain: chain_id,
                    address: token_address.to_string(),
                },
                TokenPrice {
                    price: token_data.price,
                    decimals: token_data.decimals,
                },
            );
        }

        Ok(tokens_price_data)
    }
}

/// Evaluates array of tokens amounts in USD
///
/// ### Arguments
///
/// * `tokens` - Array of (`ChainId`, `Token Address`, `amount`) tuples
///
/// ### Returns
///
/// * Array of token values
/// * Total value
pub async fn evaluate_coins(
    tokens: Vec<(ChainId, String, u128)>,
) -> EstimatorResult<(Vec<f64>, f64)> {
    let (evaluations, values_sum) = try_evaluate_coins(tokens.clone()).await?;

    let mut usd_values = vec![];
    for (evaluation, (chain_id, token_addr, _)) in evaluations.into_iter().zip(tokens.iter()) {
        let Some(usd_value) = evaluation else {
            return Err(report!(Error::ResponseError).attach_printable(format!(
                "Token {token_addr} for chain {chain_id} not found in DefiLlama response"
            )));
        };
        usd_values.push(usd_value);
    }

    Ok((usd_values, values_sum))
}

/// Tries to evaluate array of tokens amounts in USD
///
/// ### Arguments
///
/// * `tokens` - Array of (`ChainId`, `Token Address`, `amount`) tuples
///
/// ### Returns
///
/// * Array of token values. None if could not evaluate
/// * Total value
pub async fn try_evaluate_coins(
    tokens: Vec<(ChainId, String, u128)>,
) -> EstimatorResult<(Vec<Option<f64>>, f64)> {
    if tokens.is_empty() {
        return Ok((vec![], 0.0));
    }

    let mut usd_values: Vec<Option<f64>> = vec![];
    let mut values_sum: f64 = 0.0;

    let tokens_data = get_tokens_data(
        &Client::new(),
        tokens
            .iter()
            .map(|(chain_id, token_addr, _)| TokenId {
                chain: *chain_id,
                address: token_addr.clone(),
            })
            .collect(),
    )
    .await?;
    for (chain_id, token_addr, amount) in tokens {
        let token_usd_value = if let Some(token_data) = tokens_data.get((chain_id, &token_addr)) {
            let token_dec_amount = u128_to_f64(amount, token_data.decimals);
            let token_usd_value = token_dec_amount * token_data.price;
            Some(token_usd_value)
        } else {
            None
        };
        usd_values.push(token_usd_value);
        values_sum += token_usd_value.unwrap_or_default();
    }

    Ok((usd_values, values_sum))
}

/// Fetch tokens data for array of coins
///
/// ### Arguments
///
/// * `tokens` - Array of (`ChainId`, `Token Address`) tuples
pub async fn get_tokens_data(
    client: &Client,
    tokens: HashSet<TokenId>,
) -> EstimatorResult<DefiLlamaTokensResponse> {
    let tokens_str: String = tokens
        .into_iter()
        .map(|token_id| token_id.chain.to_defillama_format(&token_id.address))
        .collect::<Vec<String>>()
        .join(",");

    let response = client
        .get(format!(
            "{DEFILLAMA_COINS_BASE_URL}{TOKEN_PRICE_URI}{tokens_str}"
        ))
        .send()
        .await
        .change_context(Error::ReqwestError)
        .attach_printable("Failed to fetch token prices")?;

    let data: DefiLlamaTokensResponse = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;

    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_id_to_defillama_chain_name() {
        assert_eq!(ChainId::Base.to_defillama_chain_name(), "base");
        assert_eq!(ChainId::ArbitrumOne.to_defillama_chain_name(), "arbitrum");
        assert_eq!(ChainId::Sui.to_defillama_chain_name(), "sui");
    }

    #[test]
    fn test_to_defillama_token() {
        let base_chain = ChainId::Base;
        let solana_chain = ChainId::Solana;
        assert_eq!(
            base_chain.to_defillama_format("0xeeeEeeEeeEeeeEeeEeeEeeeEeeEeeEeeeEeeEeeE"),
            "base:0x0000000000000000000000000000000000000000"
        );
        assert_eq!(
            solana_chain.to_defillama_format("So11111111111111111111111111111111111111111"),
            "solana:So11111111111111111111111111111111111111112"
        );
    }

    #[tokio::test]
    async fn test_get_token_prices() {
        let tokens: HashSet<TokenId> = vec![
            TokenId {
                chain: ChainId::Sui,
                address:
                    "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC"
                        .to_string(),
            },
            TokenId {
                chain: ChainId::Sui,
                address: "0x2::sui::SUI".to_string(),
            },
            TokenId {
                chain: ChainId::Solana,
                address: "So11111111111111111111111111111111111111112".to_string(),
            },
            TokenId {
                chain: ChainId::Base,
                address: "0x0000000000000000000000000000000000000000".to_string(),
            },
        ]
        .into_iter()
        .collect();

        let data = get_tokens_data(&Client::new(), tokens).await.unwrap();

        let sui_native = data.get((ChainId::Sui, "0x2::sui::SUI"));
        assert!(sui_native.is_some());
        assert_eq!(sui_native.unwrap().decimals, 9);
    }

    #[tokio::test]
    async fn test_get_token_prices_wrong_token() {
        let tokens: HashSet<TokenId> = vec![
            TokenId {
                chain: ChainId::Sui,
                address: "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c159aaa42c9f7cb846e2f900e7::usdc::USDC"
                    .to_string(),
            },
            TokenId {
                chain: ChainId::Sui,
                address: "0x2::sui::SUI".to_string(),
            },
            TokenId {
                chain: ChainId::Solana,
                address: "So11111111111111111111111111111111111111112".to_string(),
            },
            TokenId {
                chain: ChainId::Base,
                address: "0x0000000000000000000000000000000000000000".to_string(),
            },
        ]
        .into_iter()
        .collect();

        let data = get_tokens_data(&Client::new(), tokens).await.unwrap();
        println!("{:#?}", data);

        let sui_native = data.get((ChainId::Sui, "0x2::sui::SUI"));
        assert!(sui_native.is_some());
        assert_eq!(sui_native.unwrap().decimals, 9);
        let wrong_token = data.get((
            ChainId::Sui,
            "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c159aaa42c9f7cb846e2f900e7::usdc::USDC",
        ));
        assert!(wrong_token.is_none());
    }

    #[tokio::test]
    async fn test_evaluate_coins() {
        let sui_usdc = String::from(
            "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC",
        );
        let native_sui = String::from("0x2::sui::SUI");
        let native_sol = String::from("So11111111111111111111111111111111111111112");
        let native_eth1 = String::from("0x0000000000000000000000000000000000000000");
        let native_eth2 = String::from("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");

        let tokens_amounts: Vec<(ChainId, String, u128)> = vec![
            (ChainId::Sui, sui_usdc.clone(), 10_000_000),
            (ChainId::Sui, native_sui.clone(), 1_000_000_000),
            (ChainId::Solana, native_sol.clone(), 1_000_000_000),
            (ChainId::Base, native_eth1.clone(), 1000000000000000000),
            (
                ChainId::ArbitrumOne,
                native_eth2.clone(),
                1000000000000000000,
            ),
        ];

        let (values_array, _) = evaluate_coins(tokens_amounts).await.unwrap();
        assert!(values_array[0] > 9.99 && values_array[0] < 10.01);
        assert!(values_array[3] > 500.0); // let's hope 1 ETH won't be cheaper :D
        assert_eq!(values_array[3], values_array[4]);
    }

    #[tokio::test]
    async fn test_try_evaluate_coins() {
        let sui_usdc = String::from(
            "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC",
        );
        let native_sui = String::from("0x2::sui::SUI");
        let native_sol = String::from("So11111111111111111111111111111111111111112");
        let native_eth1 = String::from("0x0000000000000000000000000000000000000000");
        let native_eth2 = String::from("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
        let token_without_price = String::from("0xa18fe27545d3b6a6d71e6289d096adb15b98341a");

        let tokens_amounts: Vec<(ChainId, String, u128)> = vec![
            (ChainId::Sui, sui_usdc.clone(), 10_000_000),
            (ChainId::Sui, native_sui.clone(), 1_000_000_000),
            (ChainId::Solana, native_sol.clone(), 1_000_000_000),
            (ChainId::Base, native_eth1.clone(), 1000000000000000000),
            (
                ChainId::ArbitrumOne,
                native_eth2.clone(),
                1000000000000000000,
            ),
            (ChainId::Base, token_without_price.clone(), 777),
        ];

        let (values_array, _) = try_evaluate_coins(tokens_amounts).await.unwrap();
        assert!(values_array[0].unwrap() > 9.99 && values_array[0].unwrap() < 10.01);
        assert!(values_array[3].unwrap() > 500.0); // let's hope 1 ETH won't be cheaper :D
        assert_eq!(values_array[3], values_array[4]);
        assert_eq!(values_array[5], None);
    }
}
