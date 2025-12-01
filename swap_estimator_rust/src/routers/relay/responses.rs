use crate::error::{Error, EstimatorResult};
use crate::routers::swap::EvmTxData;
use error_stack::ResultExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
// https://docs.relay.link/references/api/get-quote

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayQuoteResponse<TxData> {
    pub steps: Vec<RelayQuoteStep<TxData>>,
    pub fees: HashMap<String, RelayQuoteFees>,
    pub details: RelayQuoteDetails,
}

pub enum RelayStepId {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayQuoteStep<TxData> {
    // Unique identifier tied to the step
    // Available options: deposit, approve, authorize, authorize1, authorize2, swap, send
    pub id: String,
    // A call to action for the step
    pub action: Option<String>,
    // A short description of the step and what it entails
    pub description: Option<String>,
    // The kind of step, can either be a transaction or a signature.
    // Transaction steps require submitting a transaction while signature steps require submitting a signature
    pub kind: Option<String>,
    // While uncommon it is possible for steps to contain multiple items of the same kind
    // (transaction/signature) grouped together that can be executed simultaneously.
    pub items: Vec<RelayStepItem<TxData>>,
    // A unique identifier for this step, tying all related transactions together
    pub request_id: Option<String>,
    // The deposit address for the bridge request
    pub deposit_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayStepItem<TxData> {
    // Can either be complete or incomplete, this can be locally controlled once the step item is
    // completed (depending on the kind) and the check object (if returned) has been verified.
    // Once all step items are complete, the bridge is complete
    pub status: Option<String>,
    pub data: TxData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayQuoteDetails {
    pub operation: Option<String>,
    pub sender: Option<String>,
    pub recipient: Option<String>,
    pub currency_in: RelayCurrencyWithAmount,
    pub currency_out: RelayCurrencyWithAmount,
    pub refund_currency: Option<RelayCurrencyWithAmount>,
    pub currency_gas_topup: Option<RelayCurrencyWithAmount>,
    pub total_impact: Option<RelayImpact>,
    pub swap_impact: Option<RelayImpact>,
    // The swap rate which is equal to 1 input unit in the output unit, e.g. 1 USDC -> x ETH.
    // This value can fluctuate based on gas and fees.
    pub rate: Option<String>,
    pub slippage_tolerance: Option<RelaySlippageTolerance>,
    pub time_estimate: Option<u64>,
    pub user_balance: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayQuoteFees {
    pub currency: RelayCurrency,
    pub amount: String,
    pub amount_formatted: Option<String>,
    pub amount_usd: Option<String>,
    pub minimum_amount: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayCurrencyWithAmount {
    pub currency: RelayCurrency,
    pub amount: String,
    pub amount_formatted: Option<String>,
    pub amount_usd: Option<String>,
    pub minimum_amount: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayCurrency {
    pub chain_id: u32,
    pub address: String,
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub decimals: Option<u8>,
    pub metadata: Option<RelayCurrencyMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayCurrencyMetadata {
    pub logo_uri: Option<String>,
    pub verified: Option<bool>,
    pub is_native: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayImpact {
    pub usd: Option<String>,
    pub percent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaySlippageTolerance {
    pub origin: Option<RelaySlippageToleranceItem>,
    pub destination: Option<RelaySlippageToleranceItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaySlippageToleranceItem {
    pub usd: Option<String>,
    pub value: Option<String>,
    pub percent: Option<String>,
}

// ============================  EVM  ============================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayEvmTxData {
    pub from: String,
    pub to: String,
    pub data: String,
    pub value: Option<String>,
    pub gas: Option<String>,
    pub max_fee_per_gas: Option<String>,
    pub max_priority_fee_per_gas: Option<String>,
    pub chain_id: Option<u32>,
}

impl RelayEvmTxData {
    pub fn to_evm_tx_data(self) -> EstimatorResult<EvmTxData> {
        Ok(EvmTxData {
            tx_to: self.to,
            tx_data: self.data,
            tx_value: self
                .value
                .map(|value| {
                    value
                        .parse::<u128>()
                        .change_context(Error::ParseError)
                        .attach_printable(format!("Failed to parse tx value: {value}"))
                })
                .transpose()?
                .unwrap_or_default(),
        })
    }
}

// ============================  Solana  ============================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaySolanaTxData {
    pub instructions: Vec<RelaySolanaInstruction>,
    pub address_lookup_table_addresses: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaySolanaInstruction {
    pub keys: Vec<RelaySolanaKey>,
    pub program_id: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaySolanaKey {
    pub pubkey: String,
    pub is_signer: bool,
    pub is_writable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RelayResponse<TxData> {
    Quote(RelayQuoteResponse<TxData>),
    UnknownResponse(Value),
}
