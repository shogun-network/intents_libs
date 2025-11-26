use crate::error::{Error, EstimatorResult};
use crate::routers::estimate::{GenericEstimateRequest, GenericEstimateResponse, TradeType};
use crate::routers::jupiter::get_jupiter_max_slippage;
use crate::routers::jupiter::models::{JupiterSwapResponse, QuoteResponse, SwapMode};
use crate::routers::swap::{GenericSwapRequest, SolanaPriorityFeeType};
use crate::routers::{RouterType, Slippage};
use crate::utils::number_conversion::slippage_to_bps;
use error_stack::{ResultExt, report};
use intents_models::constants::chains::{
    WRAPPED_NATIVE_TOKEN_SOLANA_ADDRESS, is_native_token_solana_address,
};
use intents_models::network::client_rate_limit::Client;
use intents_models::network::http::{handle_reqwest_response, value_to_sorted_querystring};
use serde_json::{Value, json};
use std::str::FromStr;

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
    client: &Client,
    generic_solana_estimate_request: &GenericEstimateRequest,
    jupiter_url: &str,
    jupiter_api_key: Option<String>,
) -> EstimatorResult<(GenericEstimateResponse, Value)> {
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

    let request = {
        let client = client.inner_client();
        let mut request = client.get(&url);
        if let Some(ref key) = jupiter_api_key {
            request = request.header("x-api-key", key.as_str());
        }
        request
            .build()
            .change_context(Error::ReqwestError)
            .attach_printable("Error building Jupiter request")?
    };

    let response: Value = client
        .execute(request)
        .await
        .change_context(Error::ReqwestError)?
        .json()
        .await
        .change_context(Error::Unknown)
        .attach_printable("Failed to get text from Jupiter quote response")?;

    let quote: QuoteResponse = match serde_json::from_value(response.clone()) {
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
        router_data: response.clone(),
    };

    Ok((generic_response, response))
}

pub async fn get_jupiter_transaction(
    client: &Client,
    generic_swap_request: GenericSwapRequest,
    quote: Value,
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

    let request = {
        let client = client.inner_client();
        let mut request = client.post(&url);
        if let Some(ref key) = jupiter_api_key {
            request = request.header("x-api-key", key.as_str());
        }
        request
            .json(&swap_request_body)
            .build()
            .change_context(Error::ReqwestError)
            .attach_printable("Error building Jupiter swap request")?
    };

    let response = client
        .execute(request)
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
    use serde_json::Number;

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

        let client = Client::Unrestricted(reqwest::Client::new());
        let (response, quote) = get_jupiter_quote(&client, &request, &jupiter_url, None)
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

        let client = Client::Unrestricted(reqwest::Client::new());
        let (response, quote) = get_jupiter_quote(&client, &request, &jupiter_url, None)
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

        let client = Client::Unrestricted(reqwest::Client::new());
        let (response, quote) = get_jupiter_quote(&client, &request, &jupiter_url, None)
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

        let client = Client::Unrestricted(reqwest::Client::new());
        let jupiter_tx =
            get_jupiter_transaction(&client, swap_request, quote, &jupiter_url, None, None, None)
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

        let client = Client::Unrestricted(reqwest::Client::new());
        let (response, quote) = get_jupiter_quote(&client, &request, &jupiter_url, None)
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

        let client = Client::Unrestricted(reqwest::Client::new());
        let jupiter_tx =
            get_jupiter_transaction(&client, swap_request, quote, &jupiter_url, None, None, None)
                .await
                .expect("Jupiter swap transaction failed");
        println!("Jupiter Swap Transaction: {:?}", jupiter_tx);
    }

    fn increase_jupiter_quote_slippage(
        quote: &mut Value,
        extra_slippage_percent: f64,
    ) -> EstimatorResult<()> {
        if extra_slippage_percent < 0.0 {
            return Err(report!(Error::Unknown)
                .attach_printable("extra_slippage_percent cannot be negative"));
        }
        let out_amount_str = quote
            .get("outAmount")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                report!(Error::SerdeDeserialize(
                    "outAmount missing or not string".to_string()
                ))
            })?;
        let threshold_str = quote
            .get("otherAmountThreshold")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                report!(Error::SerdeDeserialize(
                    "otherAmountThreshold missing or not string".to_string()
                ))
            })?;
        let out_amount = u128::from_str(out_amount_str).change_context(Error::SerdeDeserialize(
            "Failed parsing outAmount".to_string(),
        ))?;
        let current_threshold = u128::from_str(threshold_str).change_context(
            Error::SerdeDeserialize("Failed parsing otherAmountThreshold".to_string()),
        )?;

        if out_amount == 0 {
            return Err(report!(Error::Unknown).attach_printable("outAmount must be > 0"));
        }

        let current_slippage = 100.0 - (current_threshold as f64 * 100.0 / out_amount as f64);
        let mut new_slippage = current_slippage + extra_slippage_percent;
        if new_slippage >= 99.999 {
            // Clamp to avoid degenerate threshold
            new_slippage = 99.999;
        }

        let new_threshold = ((out_amount as f64) * (100.0 - new_slippage) / 100.0).round() as u128;

        let slippage_bps = ((out_amount - new_threshold) as u128 * 10_000 / out_amount) as u64;

        quote["otherAmountThreshold"] = Value::String(new_threshold.to_string());
        quote["slippageBps"] = Value::Number(Number::from(slippage_bps));

        Ok(())
    }

    #[tokio::test]
    async fn test_get_jupiter_modified_transaction() {
        dotenv::dotenv().ok();
        let request = GenericEstimateRequest {
            trade_type: TradeType::ExactIn,
            chain_id: ChainId::Solana,
            src_token: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            dest_token: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263".to_string(),
            amount_fixed: 1_000_000,
            slippage: Slippage::Percent(5.0),
        };
        let client = Client::Unrestricted(reqwest::Client::new());
        let jupiter_url = std::env::var("JUPITER_URL").unwrap();

        let (_est, mut quote) = get_jupiter_quote(&client, &request, &jupiter_url, None)
            .await
            .expect("Initial quote failed");
        // Increase slippage by +25%
        increase_jupiter_quote_slippage(&mut quote, 25.0).expect("Failed to increase slippage");

        // Basic sanity checks after modification
        let out_amount = u128::from_str(quote.get("outAmount").unwrap().as_str().unwrap()).unwrap();
        let new_threshold =
            u128::from_str(quote.get("otherAmountThreshold").unwrap().as_str().unwrap()).unwrap();
        let new_slippage_bps = quote.get("slippageBps").unwrap().as_u64().unwrap();
        assert!(new_threshold < out_amount, "Threshold must be < outAmount");
        let computed_bps = (out_amount - new_threshold) * 10_000 / out_amount;
        assert_eq!(
            computed_bps as u64, new_slippage_bps,
            "slippageBps mismatch"
        );

        // Use modified quote for transaction
        let swap_request = GenericSwapRequest {
            trade_type: TradeType::ExactIn,
            chain_id: ChainId::Solana,
            spender: "7kDXEH3xPS5TvScR1czWvSCJMaeHHB9693mWTrdTRQVB".to_string(),
            dest_address: "G22xmTDQHKnn9TiVbqgLAiBhoVPdhL1A3NqMELWYBGXa".to_string(),
            src_token: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            dest_token: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263".to_string(),
            amount_fixed: 1_000_000,
            slippage: Slippage::Percent(30.0), // 5% original + 25% extra
        };
        let tx =
            get_jupiter_transaction(&client, swap_request, quote, &jupiter_url, None, None, None)
                .await
                .expect("Modified transaction failed");
        println!("Modified Jupiter TX: {:#?}", tx);
    }
}
