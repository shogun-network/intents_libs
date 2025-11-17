use intents_models::network::rate_limit::RateLimitedRequest;

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

// TODO: Ideally we should have generic requests and a trait for handler fn based on router, but some router need different
// data in, so for now we keep it simple. But it will be a nice refactor for the future. We will need to add now fields to
// generic requests to cover all routers needs.
pub enum JupiterRequest {
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
impl RateLimitedRequest for JupiterRequest {
    fn cost(&self) -> std::num::NonZeroU32 {
        // In this case both request types have the same cost.
        match self {
            JupiterRequest::Estimate { .. } => {
                // Safe: 1 is non-zero
                std::num::NonZeroU32::new(1).unwrap()
            }
            JupiterRequest::Swap { .. } => {
                // Safe: 1 is non-zero
                std::num::NonZeroU32::new(1).unwrap()
            }
        }
    }
}

pub enum JupiterResponse {
    Estimate(GenericEstimateResponse, QuoteResponse),
    Swap(JupiterSwapResponse),
}

pub async fn handle_jupiter_request(request: JupiterRequest) -> Result<JupiterResponse, Error> {
    match request {
        JupiterRequest::Estimate {
            estimator_request,
            jupiter_url,
            jupiter_api_key,
        } => match get_jupiter_quote(&estimator_request, &jupiter_url, jupiter_api_key).await {
            Ok((estimate_response, quote_response)) => {
                Ok(JupiterResponse::Estimate(estimate_response, quote_response))
            }
            Err(e) => Err(e.current_context().to_owned()),
        },
        JupiterRequest::Swap {
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
                Ok(swap_response) => Ok(JupiterResponse::Swap(swap_response)),
                Err(e) => Err(e.current_context().to_owned()),
            }
        }
    }
}
