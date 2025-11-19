use intents_models::network::rate_limit::{
    RateLimitedRequest, ThrottledApiClient, ThrottlingApiRequest,
};
use reqwest::Client;
use tokio::sync::mpsc;

use crate::{
    error::Error,
    routers::{
        estimate::{GenericEstimateRequest, GenericEstimateResponse, TradeType},
        swap::{EvmSwapResponse, GenericSwapRequest},
        zero_x::zero_x::{estimate_swap_zero_x, prepare_swap_zero_x},
    },
    utils::exact_in_reverse_quoter::ReverseQuoteResult,
};

pub type ThrottledZeroXClient =
    ThrottledApiClient<ZeroXThrottledRequest, ZeroXThrottledResponse, Error>;
pub type ThrottledZeroXSender =
    mpsc::Sender<ThrottlingApiRequest<ZeroXThrottledRequest, ZeroXThrottledResponse, Error>>;

// TODO: Ideally we should have generic requests and a trait for handler fn based on router, but some router need different
// data in, so for now we keep it simple. But it will be a nice refactor for the future. We will need to add new fields to
// generic requests to cover all routers needs.
// This can be done creating father enum with every router request as variants. But is it worth it? Will just mix all on the same file, I think that is even worse.
#[derive(Debug)]
pub enum ZeroXThrottledRequest {
    Estimate {
        client: Client,
        api_key: String,
        estimator_request: GenericEstimateRequest,
        prev_result: Option<ReverseQuoteResult>,
    },
    Swap {
        client: Client,
        api_key: String,
        swap_request: GenericSwapRequest,
        prev_result: Option<ReverseQuoteResult>,
        amount_estimated: Option<u128>,
        tx_origin: Option<String>,
    },
}

impl RateLimitedRequest for ZeroXThrottledRequest {
    fn cost(&self) -> std::num::NonZeroU32 {
        // In this case both request types have the same cost.
        match self {
            ZeroXThrottledRequest::Estimate {
                prev_result,
                estimator_request,
                ..
            } => {
                // Safe: 1 and 2 are non-zero
                if estimator_request.trade_type == TradeType::ExactOut && prev_result.is_none() {
                    std::num::NonZeroU32::new(2).unwrap()
                } else {
                    std::num::NonZeroU32::new(1).unwrap()
                }
            }
            ZeroXThrottledRequest::Swap {
                prev_result,
                swap_request,
                ..
            } => {
                // Safe: 1 and 2 are non-zero
                if swap_request.trade_type == TradeType::ExactOut && prev_result.is_none() {
                    std::num::NonZeroU32::new(2).unwrap()
                } else {
                    std::num::NonZeroU32::new(1).unwrap()
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum ZeroXThrottledResponse {
    Estimate(GenericEstimateResponse),
    Swap(EvmSwapResponse),
}

pub async fn handle_zero_x_throttled_request(
    request: ZeroXThrottledRequest,
) -> Result<ZeroXThrottledResponse, Error> {
    match request {
        ZeroXThrottledRequest::Estimate {
            client,
            api_key,
            estimator_request,
            prev_result,
        } => match estimate_swap_zero_x(&client, &api_key, estimator_request, prev_result).await {
            Ok(estimate_response) => Ok(ZeroXThrottledResponse::Estimate(estimate_response)),
            Err(e) => Err(e.current_context().to_owned()),
        },
        ZeroXThrottledRequest::Swap {
            client,
            api_key,
            swap_request,
            prev_result,
            amount_estimated,
            tx_origin,
        } => {
            match prepare_swap_zero_x(
                &client,
                &api_key,
                swap_request,
                prev_result,
                amount_estimated,
                tx_origin,
            )
            .await
            {
                Ok(swap_response) => Ok(ZeroXThrottledResponse::Swap(swap_response)),
                Err(e) => Err(e.current_context().to_owned()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routers::estimate::GenericEstimateRequest;
    use crate::routers::estimate::TradeType;
    use intents_models::constants::chains::ChainId;
    use intents_models::network::RateLimitWindow;
    use intents_models::network::rate_limit::ApiClientError;
    use std::num::NonZeroU32;
    use std::sync::Arc;
    use tokio::task::JoinSet;

    fn build_estimate_request(chain_id: ChainId, amount: u128) -> ZeroXThrottledRequest {
        let client = Client::new();
        let src_token = match chain_id {
            ChainId::Base => "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913", // USDC Base
            ChainId::Ethereum => "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48", // USDC Mainnet
            _ => panic!("Unsupported chain for this test"),
        }
        .to_string();

        let dst_token = match chain_id {
            ChainId::Base => "0x4200000000000000000000000000000000000006", // WETH Base
            ChainId::Ethereum => "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2", // WETH Mainnet
            _ => panic!("Unsupported chain for this test"),
        }
        .to_string();

        let req = GenericEstimateRequest {
            trade_type: TradeType::ExactIn,
            chain_id,
            src_token,
            dest_token: dst_token,
            amount_fixed: amount,
            slippage: crate::routers::Slippage::Percent(1.0),
        };

        ZeroXThrottledRequest::Estimate {
            client,
            api_key: std::env::var("ZERO_X_API_KEY")
                .expect("ZERO_X_API_KEY must be set for this test"),
            estimator_request: req,
            prev_result: None,
        }
    }

    fn build_swap_request(chain_id: ChainId, amount: u128) -> ZeroXThrottledRequest {
        let client = Client::new();
        let src_token = match chain_id {
            ChainId::Base => "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913", // USDC Base
            ChainId::Ethereum => "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48", // USDC Mainnet
            _ => panic!("Unsupported chain for this test"),
        }
        .to_string();

        let dst_token = match chain_id {
            ChainId::Base => "0x4200000000000000000000000000000000000006", // WETH Base
            ChainId::Ethereum => "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2", // WETH Mainnet
            _ => panic!("Unsupported chain for this test"),
        }
        .to_string();

        let req = GenericSwapRequest {
            trade_type: TradeType::ExactIn,
            chain_id,
            src_token,
            dest_token: dst_token,
            amount_fixed: amount,
            slippage: crate::routers::Slippage::Percent(1.0),
            spender: "0x9ecDC9aF2a8254DdE8bbce8778eFAe695044cC9F".to_string(),
            dest_address: "0x9ecDC9aF2a8254DdE8bbce8778eFAe695044cC9F".to_string(),
        };

        ZeroXThrottledRequest::Swap {
            client,
            api_key: std::env::var("ZERO_X_API_KEY")
                .expect("ZERO_X_API_KEY must be set for this test"),
            swap_request: req,
            prev_result: None,
            amount_estimated: None,
            tx_origin: None,
        }
    }

    /// Manual / experimental test:
    /// - Creates two throttled clients with 10 req/s each.
    /// - Sends 10 estimates per client to different chains.
    ///
    /// This primarily verifies that our local rate limiter allows 10 requests per client,
    /// and gives a rough idea whether 0x starts returning rate‑limit errors.
    ///
    /// Run it manually with `cargo test -- --ignored` and a valid ZERO_X_API_KEY.
    #[tokio::test]
    #[ignore]
    async fn test_zero_x_rate_limit_two_clients_two_chains() {
        dotenv::dotenv().ok();

        let rl_window = RateLimitWindow::PerSecond(NonZeroU32::new(10).unwrap());
        let queue_capacity = 32;

        let client_base = Arc::new(ThrottledZeroXClient::new(
            rl_window,
            None,
            queue_capacity,
            handle_zero_x_throttled_request,
        ));

        let client_eth = Arc::new(ThrottledZeroXClient::new(
            rl_window,
            None,
            queue_capacity,
            handle_zero_x_throttled_request,
        ));

        let mut join_set = JoinSet::new();

        // 10 requests for Base
        for i in 0..10 {
            let client = Arc::clone(&client_base);
            let req = build_estimate_request(ChainId::Base, 1_000_000u128 + i);
            join_set.spawn(async move { client.send(req).await });
        }

        // 10 requests for Ethereum
        for i in 0..10 {
            let client = Arc::clone(&client_eth);
            let req = build_estimate_request(ChainId::Ethereum, 1_000_000u128 + i);
            join_set.spawn(async move { client.send(req).await });
        }

        let mut success = 0usize;

        while let Some(res) = join_set.join_next().await {
            match res {
                Ok(Ok(ZeroXThrottledResponse::Estimate(e))) => {
                    println!("Received estimate response: dest amount = {:#?}", e);
                    success += 1;
                }
                Ok(Ok(_)) => {
                    panic!("Unexpected response type in throttled task");
                }
                Ok(Err(e)) => {
                    println!("Error in throttled task: {e:?}");
                }
                Err(join_err) => {
                    panic!("Join error in throttled task: {join_err:?}");
                }
            }
        }

        println!("ZeroX throttling test results: success={success}");
    }

    #[tokio::test]
    #[ignore]
    async fn test_zero_x_rate_limit_single_client_twenty_requests_limit_fifteen() {
        dotenv::dotenv().ok();

        let rl_window = RateLimitWindow::PerSecond(NonZeroU32::new(100).unwrap());
        let queue_capacity = 10000;

        let client = Arc::new(ThrottledZeroXClient::new(
            rl_window,
            None,
            queue_capacity,
            handle_zero_x_throttled_request,
        ));

        let mut join_set = JoinSet::new();

        // 20 concurrent requests on a single client
        for i in 0..300 {
            let client = Arc::clone(&client);
            let req = build_swap_request(ChainId::Base, 1_000_000u128 + i);
            join_set.spawn(async move { client.send(req).await });
        }

        let mut success = 0usize;
        let mut insufficient_capacity = 0usize;
        let mut other_errors = 0usize;

        while let Some(res) = join_set.join_next().await {
            match res {
                Ok(Ok(ZeroXThrottledResponse::Swap(e))) => {
                    println!("Received swap response: dest amount = {:#?}", e);
                    success += 1;
                }
                Ok(Ok(_)) => {
                    panic!("Unexpected response type in throttled task");
                }
                Ok(Err(ApiClientError::InsufficientCapacity)) => {
                    insufficient_capacity += 1;
                }
                Ok(Err(e)) => {
                    println!("Unexpected error in throttled task: {e:?}");
                    other_errors += 1;
                }
                Err(join_err) => {
                    panic!("Join error in throttled task: {join_err:?}");
                }
            }
        }

        println!(
            "ZeroX throttling 300-req test: success={}, insufficient_capacity={}, other_errors={}",
            success, insufficient_capacity, other_errors
        );
    }
}
