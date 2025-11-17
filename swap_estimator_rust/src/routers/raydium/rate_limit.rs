use intents_models::network::rate_limit::RateLimitedRequest;

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

// TODO: Ideally we should have generic requests and a trait for handler fn based on router, but some router need different
// data in, so for now we keep it simple. But it will be a nice refactor for the future. We will need to add now fields to
// generic requests to cover all routers needs.
pub enum RaydiumRequest {
    Estimate {
        request: RaydiumGetQuoteRequest,
        trade_type: TradeType,
    },
    Swap {
        request: RaydiumCreateTransactionRequest,
        trade_type: TradeType,
    },
}
impl RateLimitedRequest for RaydiumRequest {
    fn cost(&self) -> std::num::NonZeroU32 {
        // In this case both request types have the same cost.
        match self {
            RaydiumRequest::Estimate { .. } => {
                // Safe: 1 is non-zero
                std::num::NonZeroU32::new(1).unwrap()
            }
            RaydiumRequest::Swap { .. } => {
                // Safe: 1 is non-zero
                std::num::NonZeroU32::new(1).unwrap()
            }
        }
    }
}

pub enum RaydiumResponse {
    Estimate(crate::routers::raydium::responses::RaydiumResponse),
    Swap(Vec<Transaction>),
}

pub async fn handle_jupiter_request(request: RaydiumRequest) -> Result<RaydiumResponse, Error> {
    match request {
        RaydiumRequest::Estimate {
            request,
            trade_type,
        } => match raydium_get_price_route(request, trade_type).await {
            Ok(estimate_response) => Ok(RaydiumResponse::Estimate(estimate_response)),
            Err(e) => Err(e.current_context().to_owned()),
        },
        RaydiumRequest::Swap {
            request,
            trade_type,
        } => match raydium_create_transaction(request, trade_type).await {
            Ok(swap_response) => Ok(RaydiumResponse::Swap(swap_response)),
            Err(e) => Err(e.current_context().to_owned()),
        },
    }
}
