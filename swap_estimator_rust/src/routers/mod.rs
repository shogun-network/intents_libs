pub mod aftermath;
pub mod constants;
pub mod estimate;
pub mod jupiter;
pub mod liquidswap;
pub mod paraswap;
pub mod swap;

use crate::error::EstimatorResult;
use intents_models::constants::chains::ChainId;
use lazy_static::lazy_static;
use reqwest::Client;
use std::sync::Arc;

lazy_static! {
    static ref HTTP_CLIENT: Arc<Client> = Arc::new(Client::new());
}

#[derive(Clone, Copy, Debug)]
pub enum RouterType {
    /// In case no swap is required
    SimpleTransfer,
    UnwrapAndTransfer,
    Paraswap,
    Liquidswap,
    Jupiter,
    Aftermath,
}

pub fn routers_by_chain(chain: ChainId) -> EstimatorResult<Vec<RouterType>> {
    match chain {
        ChainId::Ethereum
        | ChainId::Bsc
        | ChainId::ArbitrumOne
        | ChainId::Base
        | ChainId::Optimism => Ok(vec![RouterType::Paraswap]),
        ChainId::HyperEVM => Ok(vec![RouterType::Liquidswap]),
        ChainId::Solana => Ok(vec![RouterType::Jupiter]),
        ChainId::Sui => Ok(vec![RouterType::Aftermath]),
    }
}
