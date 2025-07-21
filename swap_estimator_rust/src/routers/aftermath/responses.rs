use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AftermathQuoteResponse {
    pub routes: Vec<AftermathRouteData>,
    pub coin_in: CoinData,
    pub coin_out: CoinData,
    pub spot_price: f64,
    pub net_trade_fee_percentage: f64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AftermathAddTrade {
    pub tx: Value,
    pub coin_out_id: Value,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AftermathRouteData {
    pub coin_in: CoinData,
    pub coin_out: CoinData,
    pub paths: Vec<AftermathPathData>,
    pub portion: String,
    pub spot_price: f64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AftermathPathData {
    pub coin_in: CoinData,
    pub coin_out: CoinData,
    pub pool_id: String,
    pub pool_metadata: Value,
    pub protocol_name: String,
    pub spot_price: f64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CoinData {
    pub amount: String,
    pub trade_fee: String,
    #[serde(rename = "type")]
    pub coin_type: String,
}
