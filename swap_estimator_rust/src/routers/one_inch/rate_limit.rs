use intents_models::network::{
    client_rate_limit::Client,
    rate_limit::{RateLimitedRequest, ThrottledApiClient, ThrottlingApiRequest},
};
use tokio::sync::mpsc;

use crate::{
    error::Error,
    routers::{
        estimate::{GenericEstimateRequest, GenericEstimateResponse, TradeType},
        one_inch::one_inch::{estimate_swap_one_inch, prepare_swap_one_inch},
        swap::{EvmSwapResponse, GenericSwapRequest},
    },
    utils::exact_in_reverse_quoter::ReverseQuoteResult,
};

pub type ThrottledOneInchClient =
    ThrottledApiClient<OneInchThrottledRequest, OneInchThrottledResponse, Error>;
pub type ThrottledOneInchSender =
    mpsc::Sender<ThrottlingApiRequest<OneInchThrottledRequest, OneInchThrottledResponse, Error>>;

// TODO: Ideally we should have generic requests and a trait for handler fn based on router, but some router need different
// data in, so for now we keep it simple. But it will be a nice refactor for the future. We will need to add now fields to
// generic requests to cover all routers needs.
#[derive(Debug)]
pub enum OneInchThrottledRequest {
    Estimate {
        client: reqwest::Client,
        api_key: String,
        estimator_request: GenericEstimateRequest,
        prev_result: Option<ReverseQuoteResult>,
    },
    Swap {
        client: reqwest::Client,
        api_key: String,
        swap_request: GenericSwapRequest,
        prev_result: Option<ReverseQuoteResult>,
        origin: String,
    },
}

impl RateLimitedRequest for OneInchThrottledRequest {
    fn cost(&self) -> std::num::NonZeroU32 {
        match self {
            OneInchThrottledRequest::Estimate {
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
            OneInchThrottledRequest::Swap {
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
pub enum OneInchThrottledResponse {
    Estimate(GenericEstimateResponse),
    Swap(EvmSwapResponse),
}

pub async fn handle_one_inch_throttled_request(
    request: OneInchThrottledRequest,
) -> Result<OneInchThrottledResponse, Error> {
    match request {
        OneInchThrottledRequest::Estimate {
            client,
            api_key,
            estimator_request,
            prev_result,
        } => {
            match estimate_swap_one_inch(
                &Client::Unrestricted(client),
                &api_key,
                estimator_request,
                prev_result,
            )
            .await
            {
                Ok(estimate_response) => Ok(OneInchThrottledResponse::Estimate(estimate_response)),
                Err(e) => Err(e.current_context().to_owned()),
            }
        }
        OneInchThrottledRequest::Swap {
            client,
            api_key,
            swap_request,
            prev_result,
            origin,
        } => {
            match prepare_swap_one_inch(
                &Client::Unrestricted(client),
                &api_key,
                swap_request,
                prev_result,
                origin,
            )
            .await
            {
                Ok(swap_response) => Ok(OneInchThrottledResponse::Swap(swap_response)),
                Err(e) => Err(e.current_context().to_owned()),
            }
        }
    }
}
