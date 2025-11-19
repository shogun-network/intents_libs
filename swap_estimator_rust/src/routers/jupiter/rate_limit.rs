use intents_models::network::rate_limit::{
    RateLimitedRequest, ThrottledApiClient, ThrottlingApiRequest,
};
use tokio::sync::mpsc;

use crate::{
    error::Error,
    routers::{
        estimate::{GenericEstimateRequest, GenericEstimateResponse},
        jupiter::jupiter::{
            JupiterSwapResponse, QuoteResponse, get_jupiter_quote, get_jupiter_transaction,
        },
        swap::{GenericSwapRequest, SolanaPriorityFeeType},
    },
};

pub type ThrottledJupiterClient =
    ThrottledApiClient<JupiterThrottledRequest, JupiterThrottledResponse, Error>;
pub type ThrottledJupiterSender =
    mpsc::Sender<ThrottlingApiRequest<JupiterThrottledRequest, JupiterThrottledResponse, Error>>;

// TODO: Ideally we should have generic requests and a trait for handler fn based on router, but some router need different
// data in, so for now we keep it simple. But it will be a nice refactor for the future. We will need to add now fields to
// generic requests to cover all routers needs.
#[derive(Debug)]
pub enum JupiterThrottledRequest {
    Estimate {
        estimator_request: GenericEstimateRequest,
        jupiter_url: String,
        jupiter_api_key: Option<String>,
    },
    Swap {
        generic_swap_request: GenericSwapRequest,
        quote: QuoteResponse,
        jupiter_url: String,
        jupiter_api_key: Option<String>,
        priority_fee: Option<SolanaPriorityFeeType>,
        destination_token_account: Option<String>,
    },
}
impl RateLimitedRequest for JupiterThrottledRequest {
    fn cost(&self) -> std::num::NonZeroU32 {
        // In this case both request types have the same cost.
        match self {
            JupiterThrottledRequest::Estimate { .. } => {
                // Safe: 1 is non-zero
                std::num::NonZeroU32::new(1).unwrap()
            }
            JupiterThrottledRequest::Swap { .. } => {
                // Safe: 1 is non-zero
                std::num::NonZeroU32::new(1).unwrap()
            }
        }
    }
}

#[derive(Debug)]
pub enum JupiterThrottledResponse {
    Estimate(GenericEstimateResponse, QuoteResponse),
    Swap(JupiterSwapResponse),
}

pub async fn handle_jupiter_throttled_request(
    request: JupiterThrottledRequest,
) -> Result<JupiterThrottledResponse, Error> {
    match request {
        JupiterThrottledRequest::Estimate {
            estimator_request,
            jupiter_url,
            jupiter_api_key,
        } => match get_jupiter_quote(&estimator_request, &jupiter_url, jupiter_api_key).await {
            Ok((estimate_response, quote_response)) => Ok(JupiterThrottledResponse::Estimate(
                estimate_response,
                quote_response,
            )),
            Err(e) => Err(e.current_context().to_owned()),
        },
        JupiterThrottledRequest::Swap {
            generic_swap_request,
            quote,
            jupiter_url,
            jupiter_api_key,
            priority_fee,
            destination_token_account,
        } => {
            match get_jupiter_transaction(
                generic_swap_request,
                quote,
                &jupiter_url,
                jupiter_api_key,
                priority_fee,
                destination_token_account,
            )
            .await
            {
                Ok(swap_response) => Ok(JupiterThrottledResponse::Swap(swap_response)),
                Err(e) => Err(e.current_context().to_owned()),
            }
        }
    }
}
