use intents_models::network::rate_limit::{ThrottlingApiRequest, RateLimitedRequest, ThrottledApiClient};
use serde_json::Value;
use tokio::sync::mpsc;

use crate::{
    error::Error,
    routers::{
        aftermath::aftermath::{prepare_swap_ptb_with_aftermath, quote_aftermath_swap},
        estimate::{GenericEstimateRequest, GenericEstimateResponse},
        swap::GenericSwapRequest,
    },
};

pub type ThrottledAftermathClient =
    ThrottledApiClient<AftermathThrottledRequest, AftermathThrottledResponse, Error>;
pub type ThrottledAftermathSender =
    mpsc::Sender<ThrottlingApiRequest<AftermathThrottledRequest, AftermathThrottledResponse, Error>>;

// TODO: Ideally we should have generic requests and a trait for handler fn based on router, but some router need different
// data in, so for now we keep it simple. But it will be a nice refactor for the future. We will need to add now fields to
// generic requests to cover all routers needs.
pub enum AftermathThrottledRequest {
    Estimate {
        generic_estimate_request: GenericEstimateRequest,
    },
    Swap {
        generic_swap_request: GenericSwapRequest,
        routes_value: Value,
        serialized_tx_and_coin_id: Option<(Value, Value)>,
        amount_estimated: Option<u128>,
    },
}
impl RateLimitedRequest for AftermathThrottledRequest {
    fn cost(&self) -> std::num::NonZeroU32 {
        // In this case both request types have the same cost.
        match self {
            AftermathThrottledRequest::Estimate { .. } => {
                // Safe: 1 is non-zero
                std::num::NonZeroU32::new(1).unwrap()
            }
            AftermathThrottledRequest::Swap { .. } => {
                // Safe: 1 is non-zero
                std::num::NonZeroU32::new(1).unwrap()
            }
        }
    }
}

pub enum AftermathThrottledResponse {
    Estimate(GenericEstimateResponse),
    Swap(Value),
}

pub async fn handle_aftermath_throttled_request(
    request: AftermathThrottledRequest,
) -> Result<AftermathThrottledResponse, Error> {
    match request {
        AftermathThrottledRequest::Estimate {
            generic_estimate_request,
        } => match quote_aftermath_swap(generic_estimate_request).await {
            Ok(estimate_response) => Ok(AftermathThrottledResponse::Estimate(estimate_response)),
            Err(e) => Err(e.current_context().to_owned()),
        },
        AftermathThrottledRequest::Swap {
            amount_estimated,
            generic_swap_request,
            routes_value,
            serialized_tx_and_coin_id,
        } => {
            match prepare_swap_ptb_with_aftermath(
                generic_swap_request,
                routes_value,
                serialized_tx_and_coin_id,
                amount_estimated,
            )
            .await
            {
                Ok(swap_response) => Ok(AftermathThrottledResponse::Swap(swap_response)),
                Err(e) => Err(e.current_context().to_owned()),
            }
        }
    }
}
