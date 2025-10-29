use std::collections::{HashMap, HashSet};

use intents_models::constants::chains::ChainId;
use tokio::sync::oneshot;

use crate::{
    error::Error,
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
        order_id: String,
        src_chain: ChainId,
        dst_chain: ChainId,
        token_in: String,
        token_out: String,
        amount_in: u128,
        amount_out: u128,
        solver_last_bid: Option<u128>,
        extra_expenses: HashMap<TokenId, u128>,
    },
    RemoveCheckSwapFeasibility {
        order_id: String,
    },
    EstimateOrdersAmountOut {
        orders: Vec<OrderEstimationData>,
        resp: Responder<HashMap<String, u128>>,
    },
}

#[derive(Debug, Clone)]
pub enum MonitorAlert {
    SwapIsFeasible { order_id: String },
}
