use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneInchGetQuoteResponse {
    pub dst_amount: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneInchSwapResponse {
    pub dst_amount: String,
    pub tx: OneInchTx,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneInchTx {
    pub data: String,
    pub from: String,
    pub gas: u64,
    pub to: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OneInchApproveResponse {
    pub address: String,
}
