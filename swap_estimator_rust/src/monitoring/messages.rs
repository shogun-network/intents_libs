use std::collections::{HashMap, HashSet};

use intents_models::models::types::order::OrderTypeFulfillmentData;
use tokio::sync::oneshot;

use crate::{
    error::Error,
    monitoring::manager::PendingTrade,
    prices::{TokenId, TokenPrice, estimating::OrderEstimationData},
};

type Responder<T> = oneshot::Sender<Result<T, Error>>;

#[derive(Debug)]
pub enum MonitorRequest {
    GetCoinsData {
        token_ids: HashSet<TokenId>,
        resp: Responder<HashMap<TokenId, TokenPrice>>,
    },
    CheckSwapFeasibility {
        pending_swap: PendingTrade,
        solver_last_bid: Option<u128>,
    },
    RemoveCheckSwapFeasibility {
        order_id: String,
    },
    EstimateOrdersAmountOut {
        orders: Vec<OrderEstimationData>,
        resp: Responder<HashMap<String, u128>>,
    },
    EvaluateCoins {
        tokens: Vec<(TokenId, u128)>,
        resp: Responder<(Vec<f64>, f64)>,
    },
}

#[derive(Debug, Clone)]
pub enum MonitorAlert {
    SwapIsFeasible {
        order_id: String,
        order_type_fulfillment_data: OrderTypeFulfillmentData,
    },
}
