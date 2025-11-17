use intents_models::network::rate_limit::RateLimitedRequest;
use reqwest::Client;

use crate::{
    error::Error,
    routers::{
        estimate::{GenericEstimateRequest, GenericEstimateResponse},
        one_inch::one_inch::{estimate_swap_one_inch, prepare_swap_one_inch},
        swap::{EvmSwapResponse, GenericSwapRequest},
    },
    utils::exact_in_reverse_quoter::ReverseQuoteResult,
};

// TODO: Ideally we should have generic requests and a trait for handler fn based on router, but some router need different
// data in, so for now we keep it simple. But it will be a nice refactor for the future. We will need to add now fields to
// generic requests to cover all routers needs.
pub enum OneInchRequest {
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

impl RateLimitedRequest for OneInchRequest {
    fn cost(&self) -> std::num::NonZeroU32 {
        // In this case both request types have the same cost.
        match self {
            OneInchRequest::Estimate { .. } => {
                // Safe: 1 is non-zero
                std::num::NonZeroU32::new(1).unwrap()
            }
            OneInchRequest::Swap { .. } => {
                // Safe: 1 is non-zero
                std::num::NonZeroU32::new(1).unwrap()
            }
        }
    }
}

pub enum OneInchResponse {
    Estimate(GenericEstimateResponse),
    Swap(EvmSwapResponse),
}

pub async fn handle_one_inch_request(request: OneInchRequest) -> Result<OneInchResponse, Error> {
    match request {
        OneInchRequest::Estimate {
            client,
            api_key,
            estimator_request,
            prev_result,
        } => {
            match estimate_swap_one_inch(&client, &api_key, estimator_request, prev_result).await {
                Ok(estimate_response) => Ok(OneInchResponse::Estimate(estimate_response)),
                Err(e) => Err(e.current_context().to_owned()),
            }
        }
        OneInchRequest::Swap {
            client,
            api_key,
            swap_request,
            prev_result,
            origin,
        } => {
            match prepare_swap_one_inch(&client, &api_key, swap_request, prev_result, origin).await
            {
                Ok(swap_response) => Ok(OneInchResponse::Swap(swap_response)),
                Err(e) => Err(e.current_context().to_owned()),
            }
        }
    }
}
