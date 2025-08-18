use crate::models::types::order::OrderTypeFulfillmentData;
use crate::models::types::{order::OrderType, single_chain::SingleChainSolverExecutionDetailsEnum};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum WsSolverMessage {
    /// Register solver
    Register,
    /// Participate in auction (bid)
    Participate(ParticipateAuction),
    /// Request confirmation of cross chain order fulfillment
    SolverDstChain(SolverDstChainData),
    /// Request start permission
    GetStartPermissions(String, OrderType),
    /// Inform about single chain order fulfillment
    SingleChainOrderFulfilled(SingleChainSolverExecutionDetailsEnum),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ParticipateAuction {
    Single(SingleChainAuctionParticipate),
    Multi(CrossChainAuctionParticipate),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SingleChainAuctionParticipate {
    pub intent_id: String,
    pub order_type: OrderType,
    pub solver_address: String,
    pub amount_out: u128,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CrossChainAuctionParticipate {
    pub intent_id: String,
    pub order_type: OrderType,
    pub amount_out: u128,
    pub will_swap: bool,
    pub stablecoins_amount: u128,
    pub src_chain_solver_address: String,
    pub dest_chain_solver_address: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SolverDstChainData {
    pub intent_id: String,
    /// Fulfillment data for a specific order type
    pub order_type_specific_data: OrderTypeFulfillmentData,
    pub tx_hash: String,
    pub extra_transfers_tx_hashes: Option<Vec<String>>,
}
