use crate::error::{Error, EstimatorResult};
use crate::routers::HTTP_CLIENT;
use crate::routers::estimate::{GenericEstimateRequest, GenericEstimateResponse, TradeType};
use crate::routers::swap::{GenericSwapRequest, SolanaPriorityFeeType};
use error_stack::ResultExt;
use intents_models::constants::chains::{
    WRAPPED_NATIVE_TOKEN_SOLANA_ADDRESS, is_native_token_solana_address,
};
use intents_models::network::http::{handle_reqwest_response, value_to_sorted_querystring};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::str::FromStr;

// QUOTE
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
pub struct SwapInfo {
    ammKey: String,
    label: String,
    inputMint: String,
    outputMint: String,
    inAmount: String,
    outAmount: String,
    feeAmount: String,
    feeMint: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
pub struct RoutePlan {
    swapInfo: SwapInfo,
    percent: u64,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
pub struct QuoteResponse {
    inputMint: String,
    inAmount: String,
    outputMint: String,
    outAmount: String,
    otherAmountThreshold: String,
    swapMode: String,
    slippageBps: u64,
    platformFee: Option<String>,
    priceImpactPct: String,
    routePlan: Vec<RoutePlan>,
    contextSlot: u64,
    timeTaken: f64,
}

#[allow(non_snake_case)]
#[derive(Deserialize, Debug)]
pub struct SwapResponse {
    pub swapTransaction: String,
}

#[derive(Debug)]
pub enum SwapMode {
    ExactIn,
    ExactOut,
}

impl SwapMode {
    fn as_str(&self) -> &'static str {
        match self {
            SwapMode::ExactIn => "ExactIn",
            SwapMode::ExactOut => "ExactOut",
        }
    }
}

/// Replaces native Sol with wSol address
pub fn get_jupiter_token_mint(token_mint: &str) -> String {
    if is_native_token_solana_address(token_mint) {
        WRAPPED_NATIVE_TOKEN_SOLANA_ADDRESS.to_string()
    } else {
        token_mint.to_string()
    }
}

/// Fetches a quote from Jupiter for a token swap.
///
/// # Arguments
///
/// * `generic_solana_estimate_request` - Generic Solana estimate request data
pub async fn get_jupiter_quote(
    generic_solana_estimate_request: &GenericEstimateRequest,
    jupiter_url: &str,
) -> EstimatorResult<(GenericEstimateResponse, QuoteResponse)> {
    let query_value = json!({
        "amount": generic_solana_estimate_request.amount_fixed,
        "inputMint": get_jupiter_token_mint(&generic_solana_estimate_request.src_token), // src_token_mint
        "outputMint": get_jupiter_token_mint(&generic_solana_estimate_request.dest_token), // dest_token_mint
        "swapMode": match generic_solana_estimate_request.trade_type {
            TradeType::ExactOut => SwapMode::ExactOut.as_str(),
            TradeType::ExactIn => SwapMode::ExactIn.as_str(),
        },
        "slippageBps": (generic_solana_estimate_request.slippage * 100.0) as u16,
    });
    let query_string =
        value_to_sorted_querystring(&query_value).change_context(Error::ModelsError)?;
    let url = format!("{jupiter_url}quote?{query_string}");

    let response = HTTP_CLIENT
        .get(&url)
        .send()
        .await
        .change_context(Error::ReqwestError)?
        .text()
        .await
        .change_context(Error::Unknown)
        .attach_printable("Failed to get text from Jupiter quote response")?;

    let quote: QuoteResponse = serde_json::from_str(&response).change_context(
        Error::SerdeDeserialize("Error deserializing Jupiter quote response".to_string()),
    )?;

    let generic_response = GenericEstimateResponse {
        amount_quote: u128::from_str(match generic_solana_estimate_request.trade_type {
            TradeType::ExactIn => &quote.outAmount,
            TradeType::ExactOut => &quote.inAmount,
        })
        .change_context(Error::SerdeSerialize(
            "Error serializing Jupiter quote response".to_string(),
        ))?,
        amount_limit: u128::from_str(&quote.otherAmountThreshold).change_context(
            Error::SerdeSerialize("Error serializing Jupiter quote response".to_string()),
        )?,
    };

    Ok((generic_response, quote))
}

pub async fn get_jupiter_transaction(
    generic_swap_request: GenericSwapRequest,
    jupiter_url: &str,
    priority_fee: Option<SolanaPriorityFeeType>,
    destination_token_account: Option<String>,
) -> EstimatorResult<SwapResponse> {
    let generic_estimate_request = generic_swap_request.clone().into();
    let (_, quote) = get_jupiter_quote(&generic_estimate_request, jupiter_url).await?;
    let mut swap_request_body = json!({
        "quoteResponse": quote,
        "userPublicKey": generic_swap_request.spender,
        "dynamicComputeUnitLimit": true,
        "dynamicSlippage": true,
        // if destination token mint is requested as wSol, we don't want to unwrap it
        "wrapAndUnwrapSol": generic_swap_request.dest_token.to_string()
            .ne(&WRAPPED_NATIVE_TOKEN_SOLANA_ADDRESS),
        "destinationTokenAccount": destination_token_account,
    });
    if let Some(priority_fee) = priority_fee {
        swap_request_body["prioritizationFeeLamports"] = match priority_fee {
            SolanaPriorityFeeType::JitoTip(jito_tip_amount) => json!({
                "jitoTipLamports": jito_tip_amount
            }),
            SolanaPriorityFeeType::PriorityFee(max_priority_fee) => json!({
                "priorityLevelWithMaxLamports": {
                    "maxLamports": max_priority_fee,
                    "global": false,
                    "priorityLevel": "veryHigh"
                }
            }),
        };
    };

    let response = HTTP_CLIENT
        .post(format!("{jupiter_url}swap"))
        .json(&swap_request_body)
        .send()
        .await
        .change_context(Error::ReqwestError)?;

    let swap_response: SwapResponse = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;
    Ok(swap_response)
}
