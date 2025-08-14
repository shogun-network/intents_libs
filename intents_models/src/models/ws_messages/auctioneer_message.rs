use crate::models::types::cross_chain::CrossChainSolverSuccessConfirmation;
use crate::models::types::solver_types::{ExecutionTerms, SolverStartPermission};
use crate::models::types::user_types::IntentRequest;
use crate::models::ws_messages::api_response::ApiResponse;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use std::ops::Deref;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WsAuctioneerMessage {
    inner: WsAuctioneerMessageInner,
}

impl WsAuctioneerMessage {
    pub fn new(message: WsAuctioneerMessageInner) -> Self {
        Self { inner: message }
    }

    pub fn register_response(register_response_data: RegisterResponseData) -> Self {
        Self {
            inner: WsAuctioneerMessageInner::RegisterResponse(register_response_data),
        }
    }

    pub fn auction_request(auction_request_data: AuctionRequest) -> Self {
        Self {
            inner: WsAuctioneerMessageInner::AuctionRequest(auction_request_data),
        }
    }

    pub fn auction_result(result: AuctionResult) -> Self {
        Self {
            inner: WsAuctioneerMessageInner::AuctionResult(result),
        }
    }

    pub fn auction_end(auction_end_data: AuctionEndData) -> Self {
        Self {
            inner: WsAuctioneerMessageInner::AuctionEnd(auction_end_data),
        }
    }

    pub fn error(error: ApiResponse) -> Self {
        Self {
            inner: WsAuctioneerMessageInner::ErrorMessage(error),
        }
    }
}

impl Deref for WsAuctioneerMessage {
    type Target = WsAuctioneerMessageInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum WsAuctioneerMessageInner {
    RegisterResponse(RegisterResponseData),
    AuctionRequest(AuctionRequest),
    AuctionResult(AuctionResult),
    AuctionEnd(AuctionEndData),
    ErrorMessage(ApiResponse), // Must uses ApiResponse type of error (e.g. bad request, internal server error, etc.)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RegisterResponseData {
    pub solver_id: String,
    pub status: String,
    pub pending_auction_results: Vec<AuctionResult>,
    pub unfinished_orders: Vec<AuctionRequest>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuctionRequest {
    pub intent_id: String,
    pub intent: IntentRequest,
    pub execution_terms: ExecutionTerms,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuctionResult {
    pub intent_id: String,
    #[serde_as(as = "DisplayFromStr")]
    pub amount_out: u128,
    pub solver_start_permission: Option<SolverStartPermission>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuctionEndData {
    pub intent_id: String,
    pub solver_success_confirmation: CrossChainSolverSuccessConfirmation,
}
