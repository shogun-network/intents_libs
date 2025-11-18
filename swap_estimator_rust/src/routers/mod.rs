pub mod aftermath;
pub mod constants;
pub mod estimate;
pub mod jupiter;
pub mod liquidswap;
pub mod one_inch;
pub mod paraswap;
pub mod raydium;
pub mod swap;
pub mod zero_x;

use crate::error::EstimatorResult;
use intents_models::constants::chains::ChainId;
use lazy_static::lazy_static;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

lazy_static! {
    static ref HTTP_CLIENT: Arc<Client> = Arc::new(Client::new());
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Slippage {
    Percent(f64),
    AmountLimit {
        /// Min/max out/in amount accepted
        amount_limit: u128,
        /// Fallback slippage percentage in case aggregator doesn't support amount_limit (mostly on estimations)
        fallback_slippage: f64,
    },
    MaxSlippage,
}

// TODO: We can add this calculated quotes and send it to swap functions in order to save another estimation inside swap function, like:
// expanding the enum RouterType so each variant has its quotes added
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RouterType {
    /// In case no swap is required
    SimpleTransfer,
    UnwrapAndTransfer,
    Paraswap,
    OneInch,
    ZeroX,
    Liquidswap,
    Jupiter,
    Aftermath,
    LaunchPad,
    PumpFun,
}

pub fn routers_by_chain(chain: ChainId) -> EstimatorResult<Vec<RouterType>> {
    match chain {
        ChainId::Ethereum
        | ChainId::Bsc
        | ChainId::ArbitrumOne
        | ChainId::Base
        | ChainId::Optimism => Ok(vec![RouterType::OneInch, RouterType::ZeroX]),
        ChainId::HyperEVM => Ok(vec![RouterType::Liquidswap]),
        ChainId::Solana => Ok(vec![
            RouterType::Jupiter,
            RouterType::LaunchPad,
            RouterType::PumpFun,
        ]),
        ChainId::Sui => Ok(vec![RouterType::Aftermath]),
    }
}
