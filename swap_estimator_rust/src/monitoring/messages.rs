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
}

#[derive(Debug)]
pub enum MonitorResponse {
    CoinData, // TODO: Add actual data structure
}

#[derive(Debug, Clone)]
pub enum MonitorAlert {}
