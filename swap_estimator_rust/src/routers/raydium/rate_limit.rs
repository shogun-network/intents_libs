use intents_models::network::{
    client_rate_limit::Client,
    rate_limit::{RateLimitedRequest, ThrottledApiClient, ThrottlingApiRequest},
};
use tokio::sync::mpsc;

use crate::{
    error::Error,
    routers::{
        estimate::TradeType,
        raydium::{
            raydium::{raydium_create_transaction, raydium_get_price_route},
            requests::{RaydiumCreateTransactionRequest, RaydiumGetQuoteRequest},
            responses::Transaction,
        },
    },
};

pub type ThrottledRaydiumClient =
    ThrottledApiClient<RaydiumThrottledRequest, RaydiumThrottledResponse, Error>;
pub type ThrottledRaydiumSender =
    mpsc::Sender<ThrottlingApiRequest<RaydiumThrottledRequest, RaydiumThrottledResponse, Error>>;

// TODO: Ideally we should have generic requests and a trait for handler fn based on router, but some router need different
// data in, so for now we keep it simple. But it will be a nice refactor for the future. We will need to add now fields to
// generic requests to cover all routers needs.
#[derive(Debug)]
pub enum RaydiumThrottledRequest {
    Estimate {
        client: reqwest::Client,
        request: RaydiumGetQuoteRequest,
        trade_type: TradeType,
    },
    Swap {
        client: reqwest::Client,
        request: RaydiumCreateTransactionRequest,
        trade_type: TradeType,
    },
}
impl RateLimitedRequest for RaydiumThrottledRequest {
    fn cost(&self) -> std::num::NonZeroU32 {
        // In this case both request types have the same cost.
        match self {
            RaydiumThrottledRequest::Estimate { .. } => {
                // Safe: 1 is non-zero
                std::num::NonZeroU32::new(1).unwrap()
            }
            RaydiumThrottledRequest::Swap { .. } => {
                // Safe: 1 is non-zero
                std::num::NonZeroU32::new(1).unwrap()
            }
        }
    }
}

#[derive(Debug)]
pub enum RaydiumThrottledResponse {
    Estimate(crate::routers::raydium::responses::RaydiumResponse),
    Swap(Vec<Transaction>),
}

pub async fn handle_raydium_throttled_request(
    request: RaydiumThrottledRequest,
) -> Result<RaydiumThrottledResponse, Error> {
    match request {
        RaydiumThrottledRequest::Estimate {
            client,
            request,
            trade_type,
        } => {
            match raydium_get_price_route(&Client::Unrestricted(client), request, trade_type).await
            {
                Ok(estimate_response) => Ok(RaydiumThrottledResponse::Estimate(estimate_response)),
                Err(e) => Err(e.current_context().to_owned()),
            }
        }
        RaydiumThrottledRequest::Swap {
            client,
            request,
            trade_type,
        } => match raydium_create_transaction(&Client::Unrestricted(client), request, trade_type)
            .await
        {
            Ok(swap_response) => Ok(RaydiumThrottledResponse::Swap(swap_response)),
            Err(e) => Err(e.current_context().to_owned()),
        },
    }
}
