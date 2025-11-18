use intents_models::network::rate_limit::{ThrottlingApiRequest, RateLimitedRequest, ThrottledApiClient};
use tokio::sync::mpsc;

use crate::{
    error::Error,
    routers::{
        estimate::{GenericEstimateRequest, GenericEstimateResponse},
        paraswap::paraswap::{estimate_swap_paraswap_generic, prepare_swap_paraswap_generic},
        swap::{EvmSwapResponse, GenericSwapRequest},
    },
};

pub type ThrottledParaswapClient =
    ThrottledApiClient<ParaswapThrottledRequest, ParaswapThrottledResponse, Error>;
pub type ThrottledParaswapSender =
    mpsc::Sender<ThrottlingApiRequest<ParaswapThrottledRequest, ParaswapThrottledResponse, Error>>;

// TODO: Ideally we should have generic requests and a trait for handler fn based on router, but some router need different
// data in, so for now we keep it simple. But it will be a nice refactor for the future. We will need to add now fields to
// generic requests to cover all routers needs.
pub enum ParaswapThrottledRequest {
    Estimate {
        request: GenericEstimateRequest,
        src_token_decimals: u8,
        dst_token_decimals: u8,
    },
    Swap {
        generic_swap_request: GenericSwapRequest,
        src_decimals: u8,
        dest_decimals: u8,
        estimate_response: Option<GenericEstimateResponse>,
    },
}

impl RateLimitedRequest for ParaswapThrottledRequest {
    fn cost(&self) -> std::num::NonZeroU32 {
        // In this case both request types have the same cost.
        match self {
            ParaswapThrottledRequest::Estimate { .. } => {
                // Safe: 1 is non-zero
                std::num::NonZeroU32::new(1).unwrap()
            }
            ParaswapThrottledRequest::Swap {
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

pub enum ParaswapThrottledResponse {
    Estimate(GenericEstimateResponse),
    Swap(EvmSwapResponse),
}

pub async fn handle_paraswap_throttled_request(
    request: ParaswapThrottledRequest,
) -> Result<ParaswapThrottledResponse, Error> {
    match request {
        ParaswapThrottledRequest::Estimate {
            request,
            src_token_decimals,
            dst_token_decimals,
        } => {
            match estimate_swap_paraswap_generic(request, src_token_decimals, dst_token_decimals)
                .await
            {
                Ok(estimate_response) => Ok(ParaswapThrottledResponse::Estimate(estimate_response)),
                Err(e) => Err(e.current_context().to_owned()),
            }
        }
        ParaswapThrottledRequest::Swap {
            generic_swap_request,
            src_decimals,
            dest_decimals,
            estimate_response,
        } => {
            match prepare_swap_paraswap_generic(
                generic_swap_request,
                src_decimals,
                dest_decimals,
                estimate_response,
            )
            .await
            {
                Ok(swap_response) => Ok(ParaswapThrottledResponse::Swap(swap_response)),
                Err(e) => Err(e.current_context().to_owned()),
            }
        }
    }
}
