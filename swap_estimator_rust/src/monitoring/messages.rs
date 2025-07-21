use intents_models::constants::chains::ChainId;
use tokio::sync::oneshot;

use crate::{error::Error, prices::defillama::pricing::DefiLlamaCoinData};

type Responder<T> = oneshot::Sender<Result<T, Error>>;

#[derive(Debug)]
pub enum MonitorRequest {
    GetCoinData {
        chain: ChainId,
        address: String,
        resp: Responder<DefiLlamaCoinData>,
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
}

#[derive(Debug, Clone)]
pub enum MonitorAlert {
    SwapIsFeasible { order_id: String },
}
