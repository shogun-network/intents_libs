use crate::models::ws_messages::auctioneer_message::{AuctionRequest, AuctionResult};

impl AuctionRequest {
    pub fn get_intent_id(&self) -> String {
        self.intent_id.clone()
    }
}

impl AuctionResult {
    pub fn get_intent_id(&self) -> String {
        self.intent_id.clone()
    }
}
