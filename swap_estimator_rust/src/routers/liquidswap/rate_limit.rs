use intents_models::network::rate_limit::RateLimitedRequest;

use crate::{
    error::Error,
    routers::{
        estimate::{GenericEstimateRequest, GenericEstimateResponse},
        liquidswap::liquidswap::{
            estimate_swap_liquidswap_generic, prepare_swap_liquidswap_generic,
        },
        swap::{EvmSwapResponse, GenericSwapRequest},
    },
};

// TODO: Ideally we should have generic requests and a trait for handler fn based on router, but some router need different
// data in, so for now we keep it simple. But it will be a nice refactor for the future. We will need to add now fields to
// generic requests to cover all routers needs.
pub enum LiquidswapRequest {
    Estimate {
        request: GenericEstimateRequest,
    },
    Swap {
        generic_swap_request: GenericSwapRequest,
        estimate_response: Option<GenericEstimateResponse>,
    },
}

impl RateLimitedRequest for LiquidswapRequest {
    fn cost(&self) -> std::num::NonZeroU32 {
        match self {
            LiquidswapRequest::Estimate { .. } => {
                // Safe: 1 is non-zero
                std::num::NonZeroU32::new(1).unwrap()
            }
            LiquidswapRequest::Swap {
                estimate_response, ..
            } => {
                if estimate_response.is_some() {
                    // Safe: 1 is non-zero
                    std::num::NonZeroU32::new(1).unwrap()
                } else {
                    // Safe: 2 is non-zero
                    std::num::NonZeroU32::new(2).unwrap()
                }
            }
        }
    }
}

pub enum LiquidswapResponse {
    Estimate(GenericEstimateResponse),
    Swap(EvmSwapResponse),
}

pub async fn handle_one_inch_request(
    request: LiquidswapRequest,
) -> Result<LiquidswapResponse, Error> {
    match request {
        LiquidswapRequest::Estimate { request } => {
            match estimate_swap_liquidswap_generic(request).await {
                Ok(estimate_response) => Ok(LiquidswapResponse::Estimate(estimate_response)),
                Err(e) => Err(e.current_context().to_owned()),
            }
        }
        LiquidswapRequest::Swap {
            generic_swap_request,
            estimate_response,
        } => match prepare_swap_liquidswap_generic(generic_swap_request, estimate_response).await {
            Ok(swap_response) => Ok(LiquidswapResponse::Swap(swap_response)),
            Err(e) => Err(e.current_context().to_owned()),
        },
    }
}
