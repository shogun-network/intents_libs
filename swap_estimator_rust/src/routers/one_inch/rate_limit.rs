use intents_models::network::rate_limit::{ApiRequest, RateLimitedRequest, ThrottledApiClient};
use reqwest::Client;
use tokio::sync::mpsc;

use crate::{
    error::Error,
    routers::{
        estimate::{GenericEstimateRequest, GenericEstimateResponse},
        one_inch::one_inch::{estimate_swap_one_inch, prepare_swap_one_inch},
        swap::{EvmSwapResponse, GenericSwapRequest},
    },
    utils::exact_in_reverse_quoter::ReverseQuoteResult,
};

pub type ThrottledOneInchClient =
    ThrottledApiClient<OneInchThrottledRequest, OneInchThrottledResponse, Error>;
pub type ThrottledOneInchSender =
    mpsc::Sender<ApiRequest<OneInchThrottledRequest, OneInchThrottledResponse, Error>>;

// TODO: Ideally we should have generic requests and a trait for handler fn based on router, but some router need different
// data in, so for now we keep it simple. But it will be a nice refactor for the future. We will need to add now fields to
// generic requests to cover all routers needs.
pub enum OneInchThrottledRequest {
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
        origin: String,
    },
}

impl RateLimitedRequest for OneInchThrottledRequest {
    fn cost(&self) -> std::num::NonZeroU32 {
        // In this case both request types have the same cost.
        match self {
            OneInchThrottledRequest::Estimate { .. } => {
                // Safe: 1 is non-zero
                std::num::NonZeroU32::new(1).unwrap()
            }
            OneInchThrottledRequest::Swap { .. } => {
                // Safe: 1 is non-zero
                std::num::NonZeroU32::new(1).unwrap()
            }
        }
    }
}

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
            match estimate_swap_one_inch(&client, &api_key, estimator_request, prev_result).await {
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
            match prepare_swap_one_inch(&client, &api_key, swap_request, prev_result, origin).await
            {
                Ok(swap_response) => Ok(OneInchThrottledResponse::Swap(swap_response)),
                Err(e) => Err(e.current_context().to_owned()),
            }
        }
    }
}
