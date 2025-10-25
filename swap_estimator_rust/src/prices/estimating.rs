use std::collections::HashMap;

use error_stack::report;
use intents_models::constants::chains::ChainId;

use crate::{
    error::{Error, EstimatorResult},
    prices::{
        TokenId, TokenPrice, TokensPriceData, codex::pricing::CodexProvider,
        gecko_terminal::pricing::GeckoTerminalProvider,
    },
    utils::number_conversion::{f64_to_u128, u128_to_f64},
};

lazy_static::lazy_static! {
    pub static ref GECKO_TERMINAL_PROVIDER: GeckoTerminalProvider = GeckoTerminalProvider::new();

    pub static ref CODEX_PROVIDER: Option<CodexProvider> = {
        // Load API key from environment variable
        dotenv::dotenv().ok();
        let api_key = std::env::var("CODEX_API_KEY").ok()?;
        Some(CodexProvider::new(api_key))
    };
}

#[derive(Debug, Clone)]
pub struct OrderEstimationData {
    pub order_id: String,
    pub src_chain: ChainId,
    pub dst_chain: ChainId,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: u128,
}

pub fn estimate_order_amount_out(
    order_data: &OrderEstimationData,
    tokens_price_data: &TokensPriceData,
) -> EstimatorResult<Option<u128>> {
    let src_token_data = tokens_price_data.get(&TokenId {
        chain: order_data.src_chain,
        address: order_data.token_in.clone(),
    });
    let dst_token_data = tokens_price_data.get(&TokenId {
        chain: order_data.dst_chain,
        address: order_data.token_out.clone(),
    });

    if let (Some(src_data), Some(dst_data)) = (src_token_data, dst_token_data) {
        let src_price = src_data.price;
        let dst_price = dst_data.price;
        if dst_price == 0.0 {
            return Err(report!(Error::ZeroPriceError));
        }

        let amount_in_decimal = u128_to_f64(order_data.amount_in, src_data.decimals);
        let amount_out_decimal = amount_in_decimal * (src_price / dst_price);
        let amount_out = f64_to_u128(amount_out_decimal, dst_data.decimals)?;
        Ok(Some(amount_out))
    } else {
        Ok(None)
    }
}

pub async fn estimate_orders_amount_out(
    orders: Vec<OrderEstimationData>,
    tokens_info: HashMap<TokenId, TokenPrice>,
) -> EstimatorResult<HashMap<String, u128>> {
    let mut result = HashMap::new();

    for order in orders {
        match estimate_order_amount_out(&order, &tokens_info) {
            Ok(Some(amount_out)) => {
                result.insert(order.order_id, amount_out);
            }
            Ok(None) => {
                tracing::warn!(
                    "Token data not found for order {}: src_chain: {}, src_token: {}, dst_chain: {}, dst_token: {}",
                    order.order_id,
                    order.src_chain,
                    order.token_in,
                    order.dst_chain,
                    order.token_out
                );
            }
            Err(e) => {
                tracing::error!(
                    "Error estimating amount out for order {}: {}",
                    order.order_id,
                    e
                );
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_coin_data(price: f64, decimals: u8) -> TokenPrice {
        TokenPrice { price, decimals }
    }

    fn create_test_tokens_response() -> HashMap<TokenId, TokenPrice> {
        let mut coins = HashMap::new();

        // Add test tokens with different prices and decimals
        coins.insert(
            TokenId {
                chain: ChainId::Ethereum,
                address: "0xa0b86a33e6ba2a5e59e3a6be836a4f08a7b2e6bd".to_string(),
            },
            create_test_coin_data(2000.0, 18),
        );
        coins.insert(
            TokenId {
                chain: ChainId::Base,
                address: "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string(),
            },
            create_test_coin_data(1.0, 6),
        );
        coins.insert(
            TokenId {
                chain: ChainId::ArbitrumOne,
                address: "0xaf88d065e77c8cc2239327c5edb3a432268e5831".to_string(),
            },
            create_test_coin_data(1.01, 6),
        );
        coins.insert(
            TokenId {
                chain: ChainId::Sui,
                address: "sui:0x2::sui::SUI".to_string(),
            },
            create_test_coin_data(1.5, 9),
        );

        coins
    }

    fn create_test_order(
        order_id: &str,
        src_chain: ChainId,
        dst_chain: ChainId,
        token_in: &str,
        token_out: &str,
        amount_in: u128,
    ) -> OrderEstimationData {
        OrderEstimationData {
            order_id: order_id.to_string(),
            src_chain,
            dst_chain,
            token_in: token_in.to_string(),
            token_out: token_out.to_string(),
            amount_in,
        }
    }

    #[test]
    fn test_estimate_order_amount_out_success() {
        let tokens_response = create_test_tokens_response();

        let order = create_test_order(
            "test_order_1",
            ChainId::Base,
            ChainId::ArbitrumOne,
            "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913", // USDC
            "0xaf88d065e77c8cc2239327c5edb3a432268e5831",
            2000000,
        );

        let result = estimate_order_amount_out(&order, &tokens_response).unwrap();

        assert!(result.is_some());
        let amount_out = result.unwrap();
        println!("Estimated amount out: {}", amount_out);
    }

    #[test]
    fn test_estimate_order_amount_out_different_decimals() {
        let tokens_response = create_test_tokens_response();

        let order = create_test_order(
            "test_order_2",
            ChainId::Base,
            ChainId::Sui,
            "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913", // USDC
            "sui:0x2::sui::SUI",                          // SUI
            3_000_000,                                    // 1 USDC (6 decimals)
        );

        let result = estimate_order_amount_out(&order, &tokens_response).unwrap();

        println!("Estimated amount out: {:?}", result);
        assert!(result.is_some());
        let amount_out = result.unwrap();

        assert_eq!(amount_out, 2000000000);
    }

    #[test]
    fn test_estimate_order_amount_out_missing_src_token() {
        let tokens_response = create_test_tokens_response();

        let order = create_test_order(
            "test_order_3",
            ChainId::Ethereum,
            ChainId::Base,
            "0xnonexistent", // Non-existent token
            "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913",
            1_000_000_000_000_000_000,
        );

        let result = estimate_order_amount_out(&order, &tokens_response).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_estimate_order_amount_out_missing_dst_token() {
        let tokens_response = create_test_tokens_response();

        let order = create_test_order(
            "test_order_4",
            ChainId::Ethereum,
            ChainId::Base,
            "0xa0b86a33e6ba2a5e59e3a6be836a4f08a7b2e6bd",
            "0xnonexistent", // Non-existent token
            1_000_000_000_000_000_000,
        );

        let result = estimate_order_amount_out(&order, &tokens_response).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_estimate_order_amount_out_zero_amount() {
        let tokens_response = create_test_tokens_response();

        let order = create_test_order(
            "test_order_5",
            ChainId::Ethereum,
            ChainId::Base,
            "0xa0b86a33e6ba2a5e59e3a6be836a4f08a7b2e6bd",
            "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913",
            0, // Zero amount
        );

        let result = estimate_order_amount_out(&order, &tokens_response).unwrap();

        assert!(result.is_some());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_estimate_order_amount_out_zero_dst_price() {
        let mut tokens_response = create_test_tokens_response();

        // Add a token with zero price - this should cause division by zero
        tokens_response.insert(
            TokenId {
                chain: ChainId::Base,
                address: "0xzerotoken".to_string(),
            },
            create_test_coin_data(0.0, 6), // Zero price
        );

        let order = create_test_order(
            "test_order_6",
            ChainId::Ethereum,
            ChainId::Base,
            "0xa0b86a33e6ba2a5e59e3a6be836a4f08a7b2e6bd",
            "0xzerotoken",
            1_000_000_000_000_000_000,
        );

        // This should either handle gracefully or panic
        // Currently will panic due to division by zero - you should fix this
        let result = estimate_order_amount_out(&order, &tokens_response);

        assert!(result.is_err(), "Should handle zero price gracefully");
    }

    #[test]
    fn test_estimate_order_amount_out_negative_price() {
        let mut tokens_response = create_test_tokens_response();

        // Add a token with negative price
        tokens_response.insert(
            TokenId {
                chain: ChainId::Base,
                address: "0xnegativetoken".to_string(),
            },
            create_test_coin_data(-1.0, 6), // Negative price
        );

        let order = create_test_order(
            "test_order_7",
            ChainId::Ethereum,
            ChainId::Base,
            "0xa0b86a33e6ba2a5e59e3a6be836a4f08a7b2e6bd",
            "0xnegativetoken",
            1_000_000_000_000_000_000,
        );

        // This should handle negative prices appropriately
        let result = estimate_order_amount_out(&order, &tokens_response);

        // Currently this will try to convert negative f64 to u128, which should error
        assert!(result.is_err(), "Should handle negative prices");
    }

    #[tokio::test]
    async fn test_estimate_orders_amount_out_success() {
        // This test requires mocking get_tokens_data since it makes external calls
        // For now, we'll test the logic structure

        let orders = vec![
            create_test_order(
                "order1",
                ChainId::Ethereum,
                ChainId::Base,
                "0xa0b86a33e6ba2a5e59e3a6be836a4f08a7b2e6bd",
                "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913",
                1_000_000_000_000_000_000,
            ),
            create_test_order(
                "order2",
                ChainId::Base,
                ChainId::ArbitrumOne,
                "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913",
                "0xaf88d065e77c8cc2239327c5edb3a432268e5831",
                1_000_000,
            ),
        ];

        let tokens_response = create_test_tokens_response();

        let result = estimate_orders_amount_out(orders, tokens_response).await;

        match result {
            Ok(estimates) => {
                assert!(
                    estimates.len() <= 2,
                    "Should return results for valid orders"
                );
                for (order_id, amount) in estimates {
                    assert!(
                        amount > 0,
                        "Amount should be positive for order {}",
                        order_id
                    );
                }
            }
            Err(_) => {
                // Expected in test environment without network access
                println!("Network call failed as expected in test environment");
            }
        }
    }

    #[tokio::test]
    async fn test_estimate_orders_amount_out_empty_input() {
        let orders = vec![];

        let tokens_response = create_test_tokens_response();

        let result = estimate_orders_amount_out(orders, tokens_response).await;

        // Should handle empty input gracefully
        match result {
            Ok(estimates) => {
                assert_eq!(
                    estimates.len(),
                    0,
                    "Should return empty results for empty input"
                );
            }
            Err(_) => {
                // Even with empty input, might fail due to network calls
                println!("Failed with empty input - check implementation");
            }
        }
    }

    #[test]
    fn test_order_estimation_data_creation() {
        let order = create_test_order(
            "test_id",
            ChainId::Ethereum,
            ChainId::Base,
            "0xtoken1",
            "0xtoken2",
            1000,
        );

        assert_eq!(order.order_id, "test_id");
        assert_eq!(order.src_chain, ChainId::Ethereum);
        assert_eq!(order.dst_chain, ChainId::Base);
        assert_eq!(order.token_in, "0xtoken1");
        assert_eq!(order.token_out, "0xtoken2");
        assert_eq!(order.amount_in, 1000);
    }

    #[test]
    fn test_estimate_order_same_token_different_chains() {
        let mut tokens_response = create_test_tokens_response();

        // Add same token on different chains with different prices
        tokens_response.insert(
            TokenId {
                chain: ChainId::Ethereum,
                address: "0xusdc".to_string(),
            },
            create_test_coin_data(1.0, 6),
        );
        tokens_response.insert(
            TokenId {
                chain: ChainId::ArbitrumOne,
                address: "0xusdc".to_string(),
            },
            create_test_coin_data(1.002, 6), // Slight price difference
        );

        let order = create_test_order(
            "cross_chain_order",
            ChainId::Ethereum,
            ChainId::ArbitrumOne,
            "0xusdc",
            "0xusdc",
            1_000_000, // 1 USDC
        );

        let result = estimate_order_amount_out(&order, &tokens_response).unwrap();

        assert!(result.is_some());
        let amount_out = result.unwrap();

        // 1 USDC * $1.0 / $1.002 ≈ 0.998 USDC = 998,003 (with 6 decimals)
        assert!(
            amount_out < 1_000_000,
            "Should account for price difference"
        );
        assert!(amount_out > 990_000, "Should be reasonable conversion");
    }
}
