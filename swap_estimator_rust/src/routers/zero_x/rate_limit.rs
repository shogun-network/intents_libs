use intents_models::network::rate_limit::RateLimitedRequest;
use reqwest::Client;

use crate::{
    error::Error,
    routers::{
        estimate::{GenericEstimateRequest, GenericEstimateResponse},
        swap::{EvmSwapResponse, GenericSwapRequest},
        zero_x::zero_x::{estimate_swap_zero_x, prepare_swap_zero_x},
    },
    utils::exact_in_reverse_quoter::ReverseQuoteResult,
};

// TODO: Ideally we should have generic requests and a trait for handler fn based on router, but some router need different
// data in, so for now we keep it simple. But it will be a nice refactor for the future. We will need to add now fields to
// generic requests to cover all routers needs.
pub enum ZeroXRequest {
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

impl RateLimitedRequest for ZeroXRequest {
    fn cost(&self) -> std::num::NonZeroU32 {
        // In this case both request types have the same cost.
        match self {
            ZeroXRequest::Estimate { .. } => {
                // Safe: 1 is non-zero
                std::num::NonZeroU32::new(1).unwrap()
            }
            ZeroXRequest::Swap { .. } => {
                // Safe: 1 is non-zero
                std::num::NonZeroU32::new(1).unwrap()
            }
        }
    }
}

pub enum ZeroXResponse {
    Estimate(GenericEstimateResponse),
    Swap(EvmSwapResponse),
}

pub async fn handle_one_inch_request(request: ZeroXRequest) -> Result<ZeroXResponse, Error> {
    match request {
        ZeroXRequest::Estimate {
            client,
            api_key,
            estimator_request,
            prev_result,
        } => match estimate_swap_zero_x(&client, &api_key, estimator_request, prev_result).await {
            Ok(estimate_response) => Ok(ZeroXResponse::Estimate(estimate_response)),
            Err(e) => Err(e.current_context().to_owned()),
        },
        ZeroXRequest::Swap {
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
                Ok(swap_response) => Ok(ZeroXResponse::Swap(swap_response)),
                Err(e) => Err(e.current_context().to_owned()),
            }
        }
    }
}
