use std::collections::HashMap;

use intents_models::constants::chains::ChainId;
use tokio::sync::oneshot;

use crate::{
    error::Error,
    prices::{TokenId, TokenPrice},
};

type Responder<T> = oneshot::Sender<Result<T, Error>>;

#[derive(Debug)]
pub enum MonitorRequest {
    GetCoinsData {
        token_ids: Vec<TokenId>,
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
    },
    RemoveCheckSwapFeasibility {
        order_id: String,
    },
}

#[derive(Debug, Clone)]
pub enum MonitorAlert {
    SwapIsFeasible { order_id: String },
}
