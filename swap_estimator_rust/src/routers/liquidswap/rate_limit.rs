use intents_models::network::rate_limit::{
    RateLimitedRequest, ThrottledApiClient, ThrottlingApiRequest,
};
use tokio::sync::mpsc;

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

// TODO: This might actually not be needed (https://docs.liqd.ag/liquidswap-integration/open-access-philosophy#no-rate-limits)

pub type ThrottledLiquidswapClient =
    ThrottledApiClient<LiquidswapThrottledRequest, LiquidswapThrottledResponse, Error>;
pub type ThrottledLiquidswapSender = mpsc::Sender<
    ThrottlingApiRequest<LiquidswapThrottledRequest, LiquidswapThrottledResponse, Error>,
>;

// TODO: Ideally we should have generic requests and a trait for handler fn based on router, but some router need different
// data in, so for now we keep it simple. But it will be a nice refactor for the future. We will need to add now fields to
// generic requests to cover all routers needs.
pub enum LiquidswapThrottledRequest {
    Estimate {
        request: GenericEstimateRequest,
    },
    Swap {
        generic_swap_request: GenericSwapRequest,
        estimate_response: Option<GenericEstimateResponse>,
    },
}

impl RateLimitedRequest for LiquidswapThrottledRequest {
    fn cost(&self) -> std::num::NonZeroU32 {
        match self {
            LiquidswapThrottledRequest::Estimate { .. } => {
                // Safe: 1 is non-zero
                std::num::NonZeroU32::new(1).unwrap()
            }
            LiquidswapThrottledRequest::Swap {
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

pub enum LiquidswapThrottledResponse {
    Estimate(GenericEstimateResponse),
    Swap(EvmSwapResponse),
}

pub async fn handle_liquidswap_throttled_request(
    request: LiquidswapThrottledRequest,
) -> Result<LiquidswapThrottledResponse, Error> {
    match request {
        LiquidswapThrottledRequest::Estimate { request } => {
            match estimate_swap_liquidswap_generic(request).await {
                Ok(estimate_response) => {
                    Ok(LiquidswapThrottledResponse::Estimate(estimate_response))
                }
                Err(e) => Err(e.current_context().to_owned()),
            }
        }
        LiquidswapThrottledRequest::Swap {
            generic_swap_request,
            estimate_response,
        } => match prepare_swap_liquidswap_generic(generic_swap_request, estimate_response).await {
            Ok(swap_response) => Ok(LiquidswapThrottledResponse::Swap(swap_response)),
            Err(e) => Err(e.current_context().to_owned()),
        },
    }
}
