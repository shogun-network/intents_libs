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
// data in, so for now we keep it simple. But it will be a nice refactor for the future. We will need to add now fields to
// generic requests to cover all routers needs.
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
