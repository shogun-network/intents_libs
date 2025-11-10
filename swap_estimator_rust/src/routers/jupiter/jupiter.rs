use crate::error::{Error, EstimatorResult};
use crate::routers::estimate::{GenericEstimateRequest, GenericEstimateResponse, TradeType};
use crate::routers::jupiter::get_jupiter_max_slippage;
use crate::routers::swap::{GenericSwapRequest, SolanaPriorityFeeType};
use crate::routers::{HTTP_CLIENT, RouterType, Slippage};
use crate::utils::number_conversion::slippage_to_bps;
use error_stack::{ResultExt, report};
use intents_models::constants::chains::{
    WRAPPED_NATIVE_TOKEN_SOLANA_ADDRESS, is_native_token_solana_address,
};
use intents_models::network::http::{handle_reqwest_response, value_to_sorted_querystring};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::str::FromStr;

// QUOTE
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
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
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RoutePlan {
    swapInfo: SwapInfo,
    percent: u64,
    bps: Option<u64>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QuoteResponse {
    pub inputMint: String,
    pub inAmount: String,
    pub outputMint: String,
    pub outAmount: String,
    pub otherAmountThreshold: String,
    pub swapMode: String,
    pub slippageBps: u64,
    pub platformFee: Option<String>,
    pub priceImpactPct: String,
    pub routePlan: Vec<RoutePlan>,
    pub contextSlot: u64,
    pub timeTaken: f64,
    pub swapUsdValue: Option<String>,
    pub simplerRouteUsed: Option<bool>,
    pub mostReliableAmmsQuoteReport: Option<MostReliableAmmsQuoteReportInfo>,
    pub useIncurredSlippageForQuoting: Option<bool>,
    pub otherRoutePlans: Option<Vec<RoutePlan>>,
    pub loadedLongtailToken: Option<bool>,
    pub instructionVersion: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MostReliableAmmsQuoteReportInfo {
    pub info: HashMap<String, String>,
}

impl Default for QuoteResponse {
    fn default() -> Self {
        QuoteResponse {
            inputMint: String::new(),
            inAmount: String::new(),
            outputMint: String::new(),
            outAmount: String::new(),
            otherAmountThreshold: String::new(),
            swapMode: String::new(),
            slippageBps: 0,
            platformFee: None,
            priceImpactPct: String::new(),
            routePlan: Vec::new(),
            contextSlot: 0,
            timeTaken: 0.0,
            swapUsdValue: None,
            simplerRouteUsed: None,
            mostReliableAmmsQuoteReport: None,
            useIncurredSlippageForQuoting: None,
            otherRoutePlans: None,
            loadedLongtailToken: None,
            instructionVersion: None,
        }
    }
}

#[allow(non_snake_case)]
#[derive(Deserialize, Debug)]
pub struct JupiterSwapResponse {
    pub swapTransaction: String,
    pub computeUnitLimit: u32,
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
    jupiter_api_key: Option<String>,
) -> EstimatorResult<(GenericEstimateResponse, QuoteResponse)> {
    let slippage_bps = match generic_solana_estimate_request.slippage {
        Slippage::Percent(percent) => slippage_to_bps(percent)?,
        Slippage::AmountLimit {
            amount_limit: _,
            fallback_slippage,
        } => slippage_to_bps(fallback_slippage)?,
        Slippage::MaxSlippage => get_jupiter_max_slippage(),
    };
    let query_value = json!({
        "amount": generic_solana_estimate_request.amount_fixed,
        "inputMint": get_jupiter_token_mint(&generic_solana_estimate_request.src_token), // src_token_mint
        "outputMint": get_jupiter_token_mint(&generic_solana_estimate_request.dest_token), // dest_token_mint
        "swapMode": match generic_solana_estimate_request.trade_type {
            TradeType::ExactOut => SwapMode::ExactOut.as_str(),
            TradeType::ExactIn => SwapMode::ExactIn.as_str(),
        },
        "slippageBps": slippage_bps,
        "instructionVersion": "V2",
    });
    let query_string =
        value_to_sorted_querystring(&query_value).change_context(Error::ModelsError)?;
    let url = format!("{jupiter_url}quote?{query_string}");

    let mut request = HTTP_CLIENT.get(&url);
    if let Some(ref key) = jupiter_api_key {
        request = request.header("x-api-key", key.as_str());
    }

    let response = request
        .send()
        .await
        .change_context(Error::ReqwestError)?
        .text()
        .await
        .change_context(Error::Unknown)
        .attach_printable("Failed to get text from Jupiter quote response")?;

    let quote: QuoteResponse = match serde_json::from_str(&response) {
        Ok(quote) => quote,
        Err(error) => {
            tracing::error!(
                "Error deserializing Jupiter quote response: {}, response: {}",
                error,
                response
            );
            return Err(report!(Error::SerdeDeserialize(format!(
                "Error deserializing Jupiter quote response: {}",
                error
            ))));
        }
    };

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
        router: RouterType::Jupiter,
        router_data: serde_json::to_value(&quote).change_context(Error::SerdeSerialize(
            "Error serializing Jupiter quote response".to_string(),
        ))?,
    };

    Ok((generic_response, quote))
}

pub async fn get_jupiter_transaction(
    generic_swap_request: GenericSwapRequest,
    quote: QuoteResponse,
    jupiter_url: &str,
    jupiter_api_key: Option<String>,
    priority_fee: Option<SolanaPriorityFeeType>,
    destination_token_account: Option<String>,
) -> EstimatorResult<JupiterSwapResponse> {
    let token_out_is_native =
        is_native_token_solana_address(generic_swap_request.dest_token.as_str());
    let native_destination_account = if token_out_is_native {
        Some(generic_swap_request.dest_address.clone())
    } else {
        None
    };
    let mut swap_request_body = json!({
        "quoteResponse": quote,
        "userPublicKey": generic_swap_request.spender,
        "dynamicComputeUnitLimit": true,
        "wrapAndUnwrapSol": generic_swap_request.dest_token.to_string()
            .ne(&WRAPPED_NATIVE_TOKEN_SOLANA_ADDRESS),
        "destinationTokenAccount": destination_token_account,
        "nativeDestinationAccount": native_destination_account,
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

    let url = format!("{jupiter_url}swap");

    let mut request = HTTP_CLIENT.post(&url);
    if let Some(ref key) = jupiter_api_key {
        request = request.header("x-api-key", key.as_str());
    }

    let response = request
        .json(&swap_request_body)
        .send()
        .await
        .change_context(Error::ReqwestError)?;

    let mut swap_response: JupiterSwapResponse = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;
    if swap_response.computeUnitLimit == 1_400_000 {
        swap_response.computeUnitLimit = 700_000;
    }
    Ok(swap_response)
}

#[cfg(test)]
mod tests {
    use intents_models::constants::chains::ChainId;

    use super::*;

    #[tokio::test]
    async fn test_get_jupiter_quote() {
        dotenv::dotenv().ok();
        let request = GenericEstimateRequest {
            trade_type: TradeType::ExactIn,
            chain_id: ChainId::Solana,
            src_token: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            dest_token: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263".to_string(),
            amount_fixed: 1000000,
            slippage: Slippage::Percent(0.02),
        };

        let jupiter_url = std::env::var("JUPITER_URL").unwrap();

        let (response, quote) = get_jupiter_quote(&request, &jupiter_url, None)
            .await
            .unwrap();
        println!("Generic Response: {:?}", response);
        println!("Jupiter Quote: {:?}", quote);
    }

    #[tokio::test]
    async fn test_get_jupiter_quote_max_slippage() {
        dotenv::dotenv().ok();
        let request = GenericEstimateRequest {
            trade_type: TradeType::ExactIn,
            chain_id: ChainId::Solana,
            src_token: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            dest_token: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263".to_string(),
            amount_fixed: 1000000,
            slippage: Slippage::MaxSlippage,
        };

        let jupiter_url = std::env::var("JUPITER_URL").unwrap();

        let (response, quote) = get_jupiter_quote(&request, &jupiter_url, None)
            .await
            .unwrap();
        println!("Generic Response: {:?}", response);
        println!("Jupiter Quote: {:?}", quote);
    }

    #[tokio::test]
    async fn test_get_jupiter_transaction() {
        dotenv::dotenv().ok();
        let request = GenericEstimateRequest {
            trade_type: TradeType::ExactIn,
            chain_id: ChainId::Solana,
            src_token: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            dest_token: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263".to_string(),
            amount_fixed: 1000000,
            slippage: Slippage::Percent(0.005),
        };

        let jupiter_url = std::env::var("JUPITER_URL").unwrap();

        let (response, quote) = get_jupiter_quote(&request, &jupiter_url, None)
            .await
            .unwrap();
        println!("Generic Response: {:?}", response);
        println!("Jupiter Quote: {:?}", quote);

        let swap_request = GenericSwapRequest {
            trade_type: TradeType::ExactIn,
            chain_id: ChainId::Solana,
            spender: "7kDXEH3xPS5TvScR1czWvSCJMaeHHB9693mWTrdTRQVB".to_string(),
            dest_address: "G22xmTDQHKnn9TiVbqgLAiBhoVPdhL1A3NqMELWYBGXa".to_string(),
            src_token: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            dest_token: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263".to_string(),
            amount_fixed: 1000000,
            slippage: Slippage::Percent(0.005),
        };

        let jupiter_tx =
            get_jupiter_transaction(swap_request, quote, &jupiter_url, None, None, None)
                .await
                .expect("Jupiter swap transaction failed");
        println!("Jupiter Swap Transaction: {:?}", jupiter_tx);
    }

    #[tokio::test]
    async fn test_get_jupiter_transaction_max_slippage() {
        dotenv::dotenv().ok();
        let request = GenericEstimateRequest {
            trade_type: TradeType::ExactIn,
            chain_id: ChainId::Solana,
            src_token: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            dest_token: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263".to_string(),
            amount_fixed: 1000000,
            slippage: Slippage::MaxSlippage,
        };

        let jupiter_url = std::env::var("JUPITER_URL").unwrap();

        let (response, quote) = get_jupiter_quote(&request, &jupiter_url, None)
            .await
            .unwrap();
        println!("Generic Response: {:?}", response);
        println!("Jupiter Quote: {:?}", quote);

        let swap_request = GenericSwapRequest {
            trade_type: TradeType::ExactIn,
            chain_id: ChainId::Solana,
            spender: "7kDXEH3xPS5TvScR1czWvSCJMaeHHB9693mWTrdTRQVB".to_string(),
            dest_address: "G22xmTDQHKnn9TiVbqgLAiBhoVPdhL1A3NqMELWYBGXa".to_string(),
            src_token: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            dest_token: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263".to_string(),
            amount_fixed: 1000000,
            slippage: Slippage::MaxSlippage,
        };

        let jupiter_tx =
            get_jupiter_transaction(swap_request, quote, &jupiter_url, None, None, None)
                .await
                .expect("Jupiter swap transaction failed");
        println!("Jupiter Swap Transaction: {:?}", jupiter_tx);
    }

    #[tokio::test]
    async fn test_get_jupiter_modifyed_transaction() {
        dotenv::dotenv().ok();
        let request = GenericEstimateRequest {
            trade_type: TradeType::ExactIn,
            chain_id: ChainId::Solana,
            src_token: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            dest_token: "D9Rz6vFncqHo2J3zTnh2iwVGzWvPyoGP87xD8hCrbonk".to_string(),
            amount_fixed: 1000000,
            slippage: Slippage::Percent(5.0),
        };

        let jupiter_url = std::env::var("JUPITER_URL").unwrap();

        let (response, mut quote) = get_jupiter_quote(&request, &jupiter_url, None)
            .await
            .unwrap();
        println!("Generic Response: {:#?}", response);
        println!("Jupiter Quote: {:#?}", quote);
        let new_slippage = request.slippage;
        let swap_request = GenericSwapRequest {
            trade_type: TradeType::ExactIn,
            chain_id: ChainId::Solana,
            spender: "7kDXEH3xPS5TvScR1czWvSCJMaeHHB9693mWTrdTRQVB".to_string(),
            dest_address: "G22xmTDQHKnn9TiVbqgLAiBhoVPdhL1A3NqMELWYBGXa".to_string(),
            src_token: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            dest_token: "D9Rz6vFncqHo2J3zTnh2iwVGzWvPyoGP87xD8hCrbonk".to_string(),
            amount_fixed: 1000000,
            slippage: new_slippage,
        };
        let jupiter_tx =
            get_jupiter_transaction(swap_request, quote.clone(), &jupiter_url, None, None, None)
                .await
                .expect("Jupiter swap transaction failed");
        println!("Jupiter Swap Transaction: {:#?}", jupiter_tx);

        // Calculate a new otherAmountThreshold with 25% more slippage
        println!("PREVIOUS QUOTE: {:#?}", quote);
        let new_slippage = 5.0 + 25.0;
        let new_other_amount_threshold = (u128::from_str(&quote.outAmount).unwrap() as f64
            * (100.0 - new_slippage)
            / 100.0) as u128;
        let out_amount = u128::from_str(&quote.outAmount).unwrap();
        let amount_out_min = 49541325194;
        quote.otherAmountThreshold = new_other_amount_threshold.to_string();
        quote.slippageBps = ((out_amount - amount_out_min) * 10_000 / out_amount - 1) as u64;
        println!("NEW QUOTE: {:#?}", quote);

        let swap_request = GenericSwapRequest {
            trade_type: TradeType::ExactIn,
            chain_id: ChainId::Solana,
            spender: "7kDXEH3xPS5TvScR1czWvSCJMaeHHB9693mWTrdTRQVB".to_string(),
            dest_address: "G22xmTDQHKnn9TiVbqgLAiBhoVPdhL1A3NqMELWYBGXa".to_string(),
            src_token: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            dest_token: "D9Rz6vFncqHo2J3zTnh2iwVGzWvPyoGP87xD8hCrbonk".to_string(),
            amount_fixed: 1000000,
            slippage: Slippage::Percent(new_slippage),
        };

        let jupiter_tx =
            get_jupiter_transaction(swap_request, quote, &jupiter_url, None, None, None)
                .await
                .expect("Jupiter swap transaction failed");
        println!("Jupiter Swap Transaction MODIFIED: {:#?}", jupiter_tx);
    }
}
