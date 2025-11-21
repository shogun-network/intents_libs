use intents_models::network::{
    client_rate_limit::Client,
    rate_limit::{RateLimitedRequest, ThrottledApiClient, ThrottlingApiRequest},
};
use serde_json::Value;
use tokio::sync::mpsc;

use crate::{
    error::Error,
    routers::{
        estimate::{GenericEstimateRequest, GenericEstimateResponse},
        jupiter::{
            jupiter::{get_jupiter_quote, get_jupiter_transaction},
            models::JupiterSwapResponse,
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
        client: reqwest::Client,
        estimator_request: GenericEstimateRequest,
        jupiter_url: String,
        jupiter_api_key: Option<String>,
    },
    Swap {
        client: reqwest::Client,
        generic_swap_request: GenericSwapRequest,
        quote: Value,
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
    Estimate(GenericEstimateResponse, Value),
    Swap(JupiterSwapResponse),
}

pub async fn handle_jupiter_throttled_request(
    request: JupiterThrottledRequest,
) -> Result<JupiterThrottledResponse, Error> {
    match request {
        JupiterThrottledRequest::Estimate {
            client,
            estimator_request,
            jupiter_url,
            jupiter_api_key,
        } => match get_jupiter_quote(
            &Client::Unrestricted(client),
            &estimator_request,
            &jupiter_url,
            jupiter_api_key,
        )
        .await
        {
            Ok((estimate_response, quote_response)) => Ok(JupiterThrottledResponse::Estimate(
                estimate_response,
                quote_response,
            )),
            Err(e) => Err(e.current_context().to_owned()),
        },
        JupiterThrottledRequest::Swap {
            client,
            generic_swap_request,
            quote,
            jupiter_url,
            jupiter_api_key,
            priority_fee,
            destination_token_account,
        } => {
            match get_jupiter_transaction(
                &Client::Unrestricted(client),
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
