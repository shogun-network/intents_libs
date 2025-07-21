use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum MonitorRequest {
    GetCoinData { chain: String, address: String },
}

pub enum MonitorResponse {
    CoinData, // TODO: Add actual data structure
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum MonitorAlert {}
