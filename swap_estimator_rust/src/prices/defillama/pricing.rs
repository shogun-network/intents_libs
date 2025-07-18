use crate::error::{Error, EstimatorResult};
use error_stack::{ResultExt, report};
use intents_models::{
    constants::chains::{
        ChainId, EVM_NULL_ADDRESS, WRAPPED_NATIVE_TOKEN_SOLANA_ADDRESS,
        is_native_token_evm_address, is_native_token_solana_address,
    },
    network::http::handle_reqwest_response,
};
use reqwest::Client;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

use crate::utils::number_conversion::u128_to_f64;

const TOKEN_PRICE_BASE_URL: &str = "https://coins.llama.fi/prices/current/";

#[derive(Debug, Deserialize)]
pub struct DefiLlamaTokensResponse {
    pub coins: HashMap<String, DefiLlamaCoinData>,
}

#[derive(Debug, Deserialize)]
pub struct DefiLlamaCoinData {
    pub decimals: u8,
    pub symbol: String,
    pub price: f64,
    pub timestamp: u32,
    pub confidence: f64,
}

pub trait DefiLlamaChain {
    fn to_defillama_chain_name(&self) -> &str;
    fn to_defillama_format(&self, address: &str) -> String;
}

impl DefiLlamaChain for ChainId {
    fn to_defillama_chain_name(&self) -> &str {
        match self {
            ChainId::Ethereum => "ethereum",
            ChainId::Base => "base",
            ChainId::Bsc => "bsc",
            ChainId::ArbitrumOne => "arbitrum",
            ChainId::Optimism => "optimism",
            ChainId::Solana => "solana",
            ChainId::Sui => "sui",
            ChainId::HyperEVM => "hyperliquid",
        }
    }

    fn to_defillama_format(&self, address: &str) -> String {
        let chain_name = self.to_defillama_chain_name();
        let token_address = {
            if is_native_token_evm_address(address) {
                EVM_NULL_ADDRESS.to_string()
            } else if is_native_token_solana_address(address) {
                WRAPPED_NATIVE_TOKEN_SOLANA_ADDRESS.to_string()
            } else {
                address.to_string()
            }
        };

        let defillama_token_format = format!("{chain_name}:{token_address}");

        defillama_token_format
    }
}

trait DefiLlamaCoinHashMap {
    fn get(&self, token: (ChainId, &str)) -> Option<&DefiLlamaCoinData>;
}

impl DefiLlamaCoinHashMap for DefiLlamaTokensResponse {
    fn get(&self, (chain_id, token): (ChainId, &str)) -> Option<&DefiLlamaCoinData> {
        self.coins.get(&chain_id.to_defillama_format(token))
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
    tokens: Vec<(ChainId, &str, u128)>,
) -> EstimatorResult<(Vec<f64>, f64)> {
    if tokens.is_empty() {
        return Ok((vec![], 0.0));
    }

    let mut usd_values: Vec<f64> = vec![];
    let mut values_sum: f64 = 0.0;

    let tokens_data = get_tokens_data(
        tokens
            .iter()
            .map(|(chain_id, token_addr, _)| (*chain_id, *token_addr))
            .collect(),
    )
    .await?;
    for (chain_id, token_addr, amount) in tokens {
        let token_data = tokens_data.get((chain_id, token_addr)).ok_or(
            report!(Error::ResponseError).attach_printable(format!(
                "Token {token_addr} for chain {chain_id} not found in DefiLlama response"
            )),
        )?;
        let token_dec_amount = u128_to_f64(amount, token_data.decimals);
        let token_usd_value = token_dec_amount * token_data.price;
        usd_values.push(token_usd_value);
        values_sum += token_usd_value;
    }

    Ok((usd_values, values_sum))
}

/// Fetch tokens data for array of coins
///
/// ### Arguments
///
/// * `tokens` - Array of (`ChainId`, `Token Address`) tuples
pub async fn get_tokens_data(
    tokens: HashSet<(ChainId, &str)>,
) -> EstimatorResult<DefiLlamaTokensResponse> {
    let client = Client::new();

    let tokens_str: String = tokens
        .into_iter()
        .map(|(chain_id, token)| chain_id.to_defillama_format(token))
        .collect::<Vec<String>>()
        .join(",");

    let response = client
        .get(format!("{TOKEN_PRICE_BASE_URL}{tokens_str}"))
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
        let tokens: HashSet<(ChainId, &str)> = vec![
            (
                ChainId::Sui,
                "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC",
            ),
            (ChainId::Sui, "0x2::sui::SUI"),
            (
                ChainId::Solana,
                "So11111111111111111111111111111111111111112",
            ),
            (ChainId::Base, "0x0000000000000000000000000000000000000000"),
        ]
        .into_iter()
        .collect();

        let data = get_tokens_data(tokens).await.unwrap();

        let sui_native = data.get((ChainId::Sui, "0x2::sui::SUI"));
        assert!(sui_native.is_some());
        assert_eq!(sui_native.unwrap().decimals, 9);
    }

    #[tokio::test]
    async fn test_get_token_prices_wrong_token() {
        let tokens: HashSet<(ChainId, &str)> = vec![
            (
                ChainId::Sui,
                "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c159aaa42c9f7cb846e2f900e7::usdc::USDC",
            ),
            (ChainId::Sui, "0x2::sui::SUI"),
            (
                ChainId::Solana,
                "So11111111111111111111111111111111111111112",
            ),
            (ChainId::Base, "0x0000000000000000000000000000000000000000"),
        ]
        .into_iter()
        .collect();

        let data = get_tokens_data(tokens).await.unwrap();

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

        let tokens_amounts: Vec<(ChainId, &str, u128)> = vec![
            (ChainId::Sui, &sui_usdc, 10_000_000),
            (ChainId::Sui, &native_sui, 1_000_000_000),
            (ChainId::Solana, &native_sol, 1_000_000_000),
            (ChainId::Base, &native_eth1, 1000000000000000000),
            (ChainId::ArbitrumOne, &native_eth2, 1000000000000000000),
        ];

        let (values_array, _) = evaluate_coins(tokens_amounts).await.unwrap();
        assert!(values_array[0] > 9.99 && values_array[0] < 10.01);
        assert!(values_array[3] > 500.0); // let's hope 1 ETH won't be cheaper :D
        assert_eq!(values_array[3], values_array[4]);
    }
}
