use crate::config::GLOBAL_CONFIG;
use crate::solana::cache::token_data::SOLANA_TOKEN_DATA_CACHE;
use crate::solana::helpers::constants::CU_BUDGET;
use crate::solana::helpers::get_native_transfer_ix;
use crate::solana::helpers::transactions::{
    append_instruction_and_increase_budget, decode_versioned_tx_from_str,
    prepend_create_ata_instruction_if_required,
};
use crate::solana::routers::estimate::GenericSolanaEstimateRequest;
use crate::solana::routers::swap::{
    GenericSwapRequest, GenericSwapResponse, SolanaPriorityFeeType,
};
use anchor_client::solana_sdk::signature::Signer;
use anchor_lang::prelude::Pubkey;
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use error_stack::{ResultExt, report};
use lib::{AppResult, Error};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use solver::constants::chains::{
    WRAPPED_NATIVE_TOKEN_SOLANA_ADDRESS, is_native_token_solana_address,
};
use solver::models::types::estimate::{GenericEstimateResponse, TradeType};
use solver::network::http::{handle_reqwest_response, value_to_sorted_querystring};
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
pub fn get_jupiter_token_mint(token_mint: &Pubkey) -> String {
    if is_native_token_solana_address(&token_mint.to_string()) {
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
    generic_solana_estimate_request: &GenericSolanaEstimateRequest,
) -> AppResult<(GenericEstimateResponse, QuoteResponse)> {
    let query_value = json!({
        "amount": generic_solana_estimate_request.amount_fixed,
        "inputMint": get_jupiter_token_mint(&generic_solana_estimate_request.src_token_mint),
        "outputMint": get_jupiter_token_mint(&generic_solana_estimate_request.dest_token_mint),
        "swapMode": match generic_solana_estimate_request.trade_type {
            TradeType::ExactOut => SwapMode::ExactOut.as_str(),
            TradeType::ExactIn => SwapMode::ExactIn.as_str(),
        },
        "slippageBps": (generic_solana_estimate_request.slippage * 100.0) as u16,
    });
    let query_string = value_to_sorted_querystring(&query_value)?;
    let client = Client::new();
    let jupiter_url = &GLOBAL_CONFIG.env_config.jupiter_url;
    let url = format!("{jupiter_url}quote?{query_string}");

    let response = client
        .get(&url)
        .send()
        .await
        .change_context(Error::ReqwestError)?
        .text()
        .await
        .change_context(Error::Unknown)
        .attach_printable("Failed to get text from Jupiter quote response")?;

    let quote: QuoteResponse =
        serde_json::from_str(&response).change_context(Error::SerdeDeserialize)?;

    let generic_response = GenericEstimateResponse {
        amount_quote: u128::from_str(match generic_solana_estimate_request.trade_type {
            TradeType::ExactIn => &quote.outAmount,
            TradeType::ExactOut => &quote.inAmount,
        })
        .change_context(Error::SerdeSerialize)?,
        amount_limit: u128::from_str(&quote.otherAmountThreshold)
            .change_context(Error::SerdeSerialize)?,
    };

    Ok((generic_response, quote))
}

/// Prepares Jupiter swap transaction.
/// DOES NOT initialize `destinationTokenAccount` if it doesn't exist
///
/// # Arguments
///
/// * `generic_swap_request` - Generic Solana swap request.
pub async fn build_jupiter_swap_transaction(
    generic_swap_request: &GenericSwapRequest,
) -> AppResult<GenericSwapResponse> {
    let solver_keypair = &GLOBAL_CONFIG.env_config.solana_keypair;

    let generic_estimate_request = GenericSolanaEstimateRequest::try_from(generic_swap_request)?;
    let (generic_estimate_response, quote) = get_jupiter_quote(&generic_estimate_request).await?;
    let token_out_is_native =
        is_native_token_solana_address(&generic_swap_request.dest_token.to_string());
    let destination_wallet = generic_swap_request
        .destination_wallet_address
        .unwrap_or(solver_keypair.pubkey());
    let sending_to_custom_destination = generic_swap_request.destination_wallet_address.is_some()
        && destination_wallet.ne(&solver_keypair.pubkey());

    let mut swap_request_body = json!({
        "quoteResponse": quote,
        "userPublicKey": solver_keypair.pubkey().to_string(),
        "dynamicComputeUnitLimit": true,
        "dynamicSlippage": true,
        // if destination token mint is requested as wSol, we don't want to unwrap it
        "wrapAndUnwrapSol": generic_swap_request.dest_token.to_string()
            .ne(&WRAPPED_NATIVE_TOKEN_SOLANA_ADDRESS)
    });

    if sending_to_custom_destination
        // If token OUT is native SOL, we swap to source wallet as destination address
        // and then add transfer instruction to transaction
        && !token_out_is_native
    {
        let destination_ata = get_associated_token_address_with_program_id(
            &destination_wallet,
            &generic_swap_request.dest_token,
            &SOLANA_TOKEN_DATA_CACHE
                .get_token_data(&generic_swap_request.dest_token)
                .await?
                .program_id,
        );
        swap_request_body["destinationTokenAccount"] = json!(destination_ata.to_string());
    };

    if let Some(priority_fee) = generic_swap_request.priority_fee {
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

    let client = Client::new();
    let jupiter_url = &GLOBAL_CONFIG.env_config.jupiter_url;
    let response = client
        .post(format!("{jupiter_url}swap"))
        .json(&swap_request_body)
        .send()
        .await
        .change_context(Error::ReqwestError)?;

    let raw_response: Value = handle_reqwest_response(response).await?;

    let tx = match serde_json::from_value::<SwapResponse>(raw_response) {
        Ok(swap_response) => {
            let mut tx = decode_versioned_tx_from_str(&swap_response.swapTransaction)?;

            // Since Jupiter doesn't add create_ata instruction for custom destination wallet
            // we need to do it ourselves
            if sending_to_custom_destination {
                if token_out_is_native {
                    // If token OUT is native SOL, we swapped to source wallet as destination address
                    // and now we need to add transfer instruction to transaction
                    append_instruction_and_increase_budget(
                        &mut tx,
                        get_native_transfer_ix(
                            &destination_wallet,
                            generic_swap_request.amount_fixed,
                        )
                        .await?,
                        CU_BUDGET.send_native,
                    )
                    .await?;
                } else {
                    // If token OUT is not native, we need to create destination token account
                    prepend_create_ata_instruction_if_required(
                        &mut tx,
                        &generic_swap_request.dest_token,
                        &destination_wallet,
                    )
                    .await?;
                }
            };

            Ok(tx)
        }
        Err(e) => {
            log::error!("Error decoding response: {e:?}");
            Err(report!(Error::DecodeError)
                .attach_printable("Failed to decode Jupiter swap response"))
        }
    }?;

    Ok(GenericSwapResponse {
        amount_quote: generic_estimate_response.amount_quote,
        amount_limit: generic_estimate_response.amount_limit,
        tx,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solana::helpers::transactions::{
        send_versioned_transaction_with_rpc, sign_versioned_transaction,
        simulate_versioned_transaction_with_rpc,
    };
    use dotenv::dotenv;
    use solver::constants::chains::NATIVE_TOKEN_SOLANA_ADDRESS;
    // We don't test `get_jupiter_quote` separately since it's called in `build_jupiter_swap_transaction`

    #[test]
    fn test_get_jupiter_token_mint() {
        let mint = get_jupiter_token_mint(
            &Pubkey::from_str("So11111111111111111111111111111111111111111").unwrap(),
        );
        assert_eq!(&mint, "So11111111111111111111111111111111111111112");

        let mint = get_jupiter_token_mint(
            &Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
        );
        assert_eq!(&mint, "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
    }

    #[tokio::test]
    async fn test_swap_to_custom_destination_exact_out_from_native() {
        dotenv().ok();
        let solver_keypair = &GLOBAL_CONFIG.env_config.solana_keypair;

        let destination_wallet =
            Pubkey::from_str("12hb5a1D27FJqCMwdvBr5dvWwKjz9M7CQPWjRXoqJhKo").unwrap();
        let usdc_token_mint =
            Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
        let native_token_mint = Pubkey::from_str(NATIVE_TOKEN_SOLANA_ADDRESS).unwrap();

        let swap_request = GenericSwapRequest {
            trade_type: TradeType::ExactOut,
            destination_wallet_address: Some(destination_wallet),
            priority_fee: Some(SolanaPriorityFeeType::JitoTip(1_000_000)),
            src_token: native_token_mint,
            dest_token: usdc_token_mint,
            amount_fixed: 10_000,
            slippage: 2.0,
        };

        let res = build_jupiter_swap_transaction(&swap_request).await.unwrap();

        let signed_swap_tx = sign_versioned_transaction(solver_keypair, res.tx)
            .await
            .unwrap();

        if std::env::var("TEST_SEND_TRANSACTION") == Ok("true".to_string()) {
            let tx_hash = send_versioned_transaction_with_rpc(&signed_swap_tx)
                .await
                .unwrap();
            println!("https://solscan.io/tx/{tx_hash}");
        } else {
            let simulation = simulate_versioned_transaction_with_rpc(&signed_swap_tx, "test").await;
            assert!(simulation.is_ok());
        }
    }

    #[tokio::test]
    async fn test_swap_to_custom_destination_exact_in_to_native() {
        dotenv().ok();
        let solver_keypair = &GLOBAL_CONFIG.env_config.solana_keypair;

        let destination_wallet =
            Pubkey::from_str("12hb5a1D27FJqCMwdvBr5dvWwKjz9M7CQPWjRXoqJhKo").unwrap();
        let usdc_token_mint =
            Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
        let native_token_mint = Pubkey::from_str(NATIVE_TOKEN_SOLANA_ADDRESS).unwrap();

        let swap_request = GenericSwapRequest {
            trade_type: TradeType::ExactIn,
            destination_wallet_address: Some(destination_wallet),
            priority_fee: Some(SolanaPriorityFeeType::JitoTip(1_000_000)),
            src_token: usdc_token_mint,
            dest_token: native_token_mint,
            amount_fixed: 10_000,
            slippage: 2.0,
        };

        let res = build_jupiter_swap_transaction(&swap_request).await.unwrap();

        let signed_swap_tx = sign_versioned_transaction(solver_keypair, res.tx)
            .await
            .unwrap();

        if std::env::var("TEST_SEND_TRANSACTION") == Ok("true".to_string()) {
            let tx_hash = send_versioned_transaction_with_rpc(&signed_swap_tx)
                .await
                .unwrap();
            println!("https://solscan.io/tx/{tx_hash}");
        } else {
            let simulation = simulate_versioned_transaction_with_rpc(&signed_swap_tx, "test").await;
            assert!(simulation.is_ok());
        }
    }
    #[tokio::test]
    async fn test_swap_to_custom_destination_exact_in_to_bonk_token() {
        dotenv().ok();
        let solver_keypair = &GLOBAL_CONFIG.env_config.solana_keypair;

        let destination_wallet =
            Pubkey::from_str("12hb5a1D27FJqCMwdvBr5dvWwKjz9M7CQPWjRXoqJhKo").unwrap();
        let usdc_token_mint =
            Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
        let bonk_token_mint =
            Pubkey::from_str("DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263").unwrap();

        let swap_request = GenericSwapRequest {
            trade_type: TradeType::ExactIn,
            destination_wallet_address: Some(destination_wallet),
            priority_fee: Some(SolanaPriorityFeeType::PriorityFee(1_000_000)),
            src_token: usdc_token_mint,
            dest_token: bonk_token_mint,
            amount_fixed: 10_000,
            slippage: 2.0,
        };

        let res = build_jupiter_swap_transaction(&swap_request).await.unwrap();

        let signed_swap_tx = sign_versioned_transaction(solver_keypair, res.tx)
            .await
            .unwrap();

        if std::env::var("TEST_SEND_TRANSACTION") == Ok("true".to_string()) {
            let tx_hash = send_versioned_transaction_with_rpc(&signed_swap_tx)
                .await
                .unwrap();
            println!("https://solscan.io/tx/{tx_hash}");
        } else {
            let simulation = simulate_versioned_transaction_with_rpc(&signed_swap_tx, "test").await;
            assert!(simulation.is_ok());
        }
    }
}
