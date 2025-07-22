use std::time::Duration;

use crate::{
    error::{Error, EstimatorResult},
    routers::{
        HTTP_CLIENT,
        constants::LIQUIDSWAP_BASE_API_URL,
        estimate::{GenericEstimateRequest, GenericEstimateResponse, TradeType},
        liquidswap::{
            requests::{GetPriceRouteRequest, GetTokenListRequest, LiquidswapRequest},
            responses::{GetPriceRouteResponse, GetTokenListResponse, LiquidswapResponse},
        },
        swap::{EvmSwapResponse, GenericSwapRequest},
    },
    utils::{
        limit_amount::get_limit_amount,
        number_conversion::{decimal_string_to_u128, u128_to_f64},
    },
};
use error_stack::{ResultExt, report};
use intents_models::{
    constants::chains::{WRAPPED_NATIVE_TOKEN_HYPE_ADDRESS, is_native_token_evm_address},
    network::http::{handle_reqwest_response, value_to_sorted_querystring},
};
use tokio::time::timeout;

pub async fn send_liquidswap_request(
    uri_path: &str,
    query: LiquidswapRequest,
) -> EstimatorResult<LiquidswapResponse> {
    let query = value_to_sorted_querystring(&serde_json::to_value(&query).change_context(
        Error::SerdeSerialize("Error serializing liquidswap request".to_string()),
    )?)
    .change_context(Error::ModelsError)?;
    let url = format!("{}{}?{}", LIQUIDSWAP_BASE_API_URL, uri_path, query);

    let response = HTTP_CLIENT
        .get(url)
        .send()
        .await
        .change_context(Error::ReqwestError)
        .attach_printable("Error in liquidswap request")?;

    let liquidswap_response = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;

    Ok(liquidswap_response)
}

fn handle_liquidswap_response(response: LiquidswapResponse) -> EstimatorResult<LiquidswapResponse> {
    match response {
        LiquidswapResponse::UnknownResponse(val) => {
            tracing::error!(
                "Unknown response from Liquidswap: {}",
                serde_json::to_string_pretty(&val).unwrap()
            );
            // println!(
            //     "Unknown response from Liquidswap: {}",
            //     serde_json::to_string_pretty(&val).unwrap()
            // );
            Err(report!(Error::ResponseError).attach_printable("Unknown response from Liquidswap"))
        }
        _ => Ok(response),
    }
}

pub async fn liquidswap_get_token_list(
    mut request: GetTokenListRequest,
) -> EstimatorResult<GetTokenListResponse> {
    if let Some(address) = request.search.as_ref() {
        if is_native_token_evm_address(&address) {
            request.search = Some(WRAPPED_NATIVE_TOKEN_HYPE_ADDRESS.to_string());
        }
    }

    let uri_path = "/tokens";

    let response =
        send_liquidswap_request(uri_path, LiquidswapRequest::GetTokenList(request)).await?;
    let LiquidswapResponse::GetTokenList(response) = handle_liquidswap_response(response)? else {
        return Err(report!(Error::ResponseError)
            .attach_printable("Unexpected response type from Liquidswap"));
    };
    Ok(response)
}

pub async fn liquidswap_get_price_route(
    request: GetPriceRouteRequest,
) -> EstimatorResult<GetPriceRouteResponse> {
    let uri_path = "/v2/route";

    let response = send_liquidswap_request(uri_path, LiquidswapRequest::GetPriceRoute(request))
        .await
        .change_context(Error::ResponseError)
        .attach_printable("Error getting price route from Liquidswap")?;
    let LiquidswapResponse::GetPriceRoute(response) = handle_liquidswap_response(response)? else {
        return Err(report!(Error::ResponseError)
            .attach_printable("Unexpected response type from Liquidswap"));
    };
    Ok(response)
}

pub fn get_token_decimals(token_info: GetTokenListResponse) -> EstimatorResult<u8> {
    match token_info.data.tokens.get(0) {
        Some(token) => Ok(token.decimals),
        None => {
            return Err(report!(Error::ResponseError)
                .attach_printable("Token not found in Liquidswap token list"));
        }
    }
}

pub async fn get_in_out_token_decimals(
    token_in: String,
    token_out: String,
) -> EstimatorResult<(u8, u8)> {
    // Get information for the input and output tokens
    let token_in_info = liquidswap_get_token_list(GetTokenListRequest {
        search: Some(token_in),
        limit: Some(1),
        metadata: Some(true),
    });

    let token_out_info = liquidswap_get_token_list(GetTokenListRequest {
        search: Some(token_out),
        limit: Some(1),
        metadata: Some(true),
    });
    let (token_in_info, token_out_info) = tokio::try_join!(token_in_info, token_out_info)?;

    let token_in_decimals = get_token_decimals(token_in_info)?;
    let token_out_decimals = get_token_decimals(token_out_info)?;
    Ok((token_in_decimals, token_out_decimals))
}

fn get_amount_quote_and_fixed(
    route_response: &GetPriceRouteResponse,
    token_in_decimals: u8,
    token_out_decimals: u8,
    trade_type: TradeType,
    slippage: f64,
) -> EstimatorResult<(u128, u128)> {
    let amount_quote = match trade_type {
        TradeType::ExactIn => {
            // For ExactIn, we get the amount out from the route response
            decimal_string_to_u128(&route_response.amount_out, token_out_decimals)?
        }
        TradeType::ExactOut => {
            // For ExactOut, we get the amount in from the route response
            decimal_string_to_u128(&route_response.amount_in, token_in_decimals)?
        }
    };
    let amount_limit = get_limit_amount(trade_type, amount_quote, slippage);
    Ok((amount_quote, amount_limit))
}

pub async fn estimate_swap_liquidswap_generic(
    request: GenericEstimateRequest,
) -> EstimatorResult<GenericEstimateResponse> {
    let (token_in_decimals, token_out_decimals) = get_in_out_token_decimals(
        request.src_token.to_string(),
        request.dest_token.to_string(),
    )
    .await
    .change_context(Error::ResponseError)
    .attach_printable("Error getting token decimals from Liquidswap")?;

    // Calculate the amount as f64 using the token decimals
    let amount_fixed = u128::try_from(request.amount_fixed)
        .change_context(Error::ParseError)
        .attach_printable("Error parsing fixed amount")?;
    let mut liquidswap_route_request = create_route_request_from_generic_estimate(request.clone());
    match request.trade_type {
        TradeType::ExactIn => {
            liquidswap_route_request.amount_in = Some(u128_to_f64(amount_fixed, token_in_decimals));
        }
        TradeType::ExactOut => {
            liquidswap_route_request.amount_out =
                Some(u128_to_f64(amount_fixed, token_out_decimals));
        }
    }

    let route_response = liquidswap_get_price_route(liquidswap_route_request)
        .await
        .change_context(Error::ResponseError)
        .attach_printable("Error getting price route from Liquidswap")?;

    let (amount_quote, amount_limit) = get_amount_quote_and_fixed(
        &route_response,
        token_in_decimals,
        token_out_decimals,
        request.trade_type,
        request.slippage,
    )
    .change_context(Error::ResponseError)
    .attach_printable("Error getting amount quote and limit from route response")?;
    Ok(GenericEstimateResponse {
        amount_quote,
        amount_limit,
    })
}

pub async fn prepare_swap_liquidswap_generic(
    generic_swap_request: GenericSwapRequest,
) -> EstimatorResult<EvmSwapResponse> {
    let (token_in_decimals, token_out_decimals) = get_in_out_token_decimals(
        generic_swap_request.src_token.to_string(),
        generic_swap_request.dest_token.to_string(),
    )
    .await?;

    let mut router_request = create_route_request_from_generic_swap(generic_swap_request.clone());

    let amount_fixed = u128::try_from(generic_swap_request.amount_fixed)
        .change_context(Error::ParseError)
        .attach_printable("Error parsing fixed amount")?;
    match generic_swap_request.trade_type {
        TradeType::ExactIn => {
            router_request.amount_in = Some(u128_to_f64(amount_fixed, token_in_decimals));
        }
        TradeType::ExactOut => {
            router_request.amount_out = Some(u128_to_f64(amount_fixed, token_out_decimals));
        }
    }
    let use_native_hype =
        router_request.use_native_hype.is_some() && router_request.use_native_hype.clone().unwrap();
    let route_response = get_price_route_with_fallback(router_request).await?;

    let (amount_quote, amount_limit) = get_amount_quote_and_fixed(
        &route_response,
        token_in_decimals,
        token_out_decimals,
        generic_swap_request.trade_type,
        generic_swap_request.slippage,
    )
    .change_context(Error::ResponseError)
    .attach_printable("Error getting amount quote and limit from route response")?;

    Ok(EvmSwapResponse {
        amount_quote: amount_quote,
        amount_limit: amount_limit,
        tx_to: route_response.execution.to.clone(),
        tx_data: route_response.execution.calldata,
        tx_value: if use_native_hype { amount_limit } else { 0 },
        approve_address: Some(route_response.execution.to),
        require_transfer: true, // Liquidswap requires transfer for swaps, as it can't be set output address
    })
}

async fn get_price_route_with_fallback(
    mut router_request: GetPriceRouteRequest,
) -> EstimatorResult<GetPriceRouteResponse> {
    // First attempt with multi_hop enabled
    match timeout(
        Duration::from_secs(10),
        liquidswap_get_price_route(router_request.clone()),
    )
    .await
    {
        Ok(result) => match result {
            Ok(response) => return Ok(response),
            Err(e) => {
                tracing::warn!("Multi-hop route failed: {:?}", e);
            }
        },
        Err(_) => {
            tracing::warn!("Multi-hop route timed out after 5 seconds");
        }
    }

    // Fallback: disable multi_hop and try again
    router_request.multi_hop = Some(false);
    tracing::info!("Retrying price route with multi_hop disabled");

    liquidswap_get_price_route(router_request)
        .await
        .change_context(Error::ResponseError)
        .attach_printable(
            "Error getting price route from Liquidswap (both multi-hop and single-hop failed)",
        )
}

fn create_route_request_from_generic_swap(
    generic_swap_request: GenericSwapRequest,
) -> GetPriceRouteRequest {
    let (token_in, use_native_hype) =
        if is_native_token_evm_address(&generic_swap_request.src_token) {
            (WRAPPED_NATIVE_TOKEN_HYPE_ADDRESS.to_string(), Some(true))
        } else {
            (generic_swap_request.src_token.to_string(), None)
        };
    let (token_out, unwrap_whype) = if is_native_token_evm_address(&generic_swap_request.dest_token)
    {
        (WRAPPED_NATIVE_TOKEN_HYPE_ADDRESS.to_string(), Some(true))
    } else {
        (generic_swap_request.dest_token.to_string(), None)
    };
    GetPriceRouteRequest {
        token_in,
        token_out,
        amount_in: None,
        amount_out: None,
        multi_hop: Some(true),
        exclude_dexes: None,
        unwrap_whype,
        slippage: None,
        use_native_hype,
    }
}

fn create_route_request_from_generic_estimate(
    generic_swap_request: GenericEstimateRequest,
) -> GetPriceRouteRequest {
    let (token_in, use_native_hype) =
        if is_native_token_evm_address(&generic_swap_request.src_token) {
            (WRAPPED_NATIVE_TOKEN_HYPE_ADDRESS.to_string(), Some(true))
        } else {
            (generic_swap_request.src_token.to_string(), None)
        };
    let (token_out, unwrap_whype) = if is_native_token_evm_address(&generic_swap_request.dest_token)
    {
        (WRAPPED_NATIVE_TOKEN_HYPE_ADDRESS.to_string(), Some(true))
    } else {
        (generic_swap_request.dest_token.to_string(), None)
    };
    GetPriceRouteRequest {
        token_in,
        token_out,
        amount_in: None,
        amount_out: None,
        multi_hop: Some(true),
        exclude_dexes: None,
        unwrap_whype,
        slippage: None,
        use_native_hype,
    }
}

#[cfg(test)]
mod tests {
    // use crate::{config::CustomEvmProvider, evm::helpers::provider::IncreasedGasLimitFiller};
    // use alloy::{
    //     network::{EthereumWallet, TransactionBuilder as _},
    //     providers::{Provider as _, ProviderBuilder},
    //     rpc::types::TransactionRequest,
    //     signers::local::PrivateKeySigner,
    // };
    // use reqwest::Url;
    // use std::sync::Arc;

    use intents_models::constants::chains::ChainId;

    use super::*;

    // Helper function to create test request
    fn create_test_request(
        trade_type: TradeType,
        src_token: &str,
        dest_token: &str,
        amount: u128,
    ) -> GenericEstimateRequest {
        GenericEstimateRequest {
            trade_type,
            chain_id: ChainId::HyperEVM, // Or whatever chain Liquidswap supports
            src_token: src_token.to_string(),
            dest_token: dest_token.to_string(),
            amount_fixed: amount,
            slippage: 2.0,
        }
    }

    fn create_test_swap_request(
        trade_type: TradeType,
        src_token: &str,
        dest_token: &str,
        amount: u128,
    ) -> GenericSwapRequest {
        GenericSwapRequest {
            trade_type,
            chain_id: ChainId::HyperEVM, // Adjust based on actual supported chain
            spender: "0x1111111111111111111111111111111111111111".to_string(),
            dest_address: "0x2222222222222222222222222222222222222222".to_string(),
            src_token: src_token.to_string(),
            dest_token: dest_token.to_string(),
            amount_fixed: amount,
            slippage: 2.0,
        }
    }

    #[tokio::test]
    async fn test_liquidswap_get_token_list() {
        let request = GetTokenListRequest {
            search: None,
            limit: Some(20),
            metadata: None,
        };

        let response = liquidswap_get_token_list(request)
            .await
            .expect("Failed to get token list from Liquidswap");
        assert!(response.success);
        println!(
            "Token List: {}",
            serde_json::to_string_pretty(&response.data.tokens).unwrap()
        );
    }

    #[tokio::test]
    async fn test_liquidswap_get_price_route_exact_in() {
        let request = GetPriceRouteRequest {
            token_in: "0x5555555555555555555555555555555555555555".to_string(), // WHYPE
            token_out: "0xb8ce59fc3717ada4c02eadf9682a9e934f625ebb".to_string(), // USDT0
            amount_in: Some(10.0),
            amount_out: None,
            multi_hop: Some(true),
            exclude_dexes: None,
            slippage: None,
            unwrap_whype: None,
            use_native_hype: None,
        };

        let response = liquidswap_get_price_route(request)
            .await
            .expect("Failed to get price route from Liquidswap");
        assert!(response.success);
        println!("Price Route: {:?}", response);
    }

    #[tokio::test]
    async fn test_liquidswap_get_price_route_exact_out() {
        let request = GetPriceRouteRequest {
            token_in: "0x5555555555555555555555555555555555555555".to_string(), // CATBAL
            token_out: "0x068f321fa8fb9f0d135f290ef6a3e2813e1c8a29".to_string(), // USOL
            amount_in: None,
            amount_out: Some(100.0),
            multi_hop: Some(true),
            exclude_dexes: None,
            slippage: None,
            unwrap_whype: None,
            use_native_hype: None,
        };

        let response = liquidswap_get_price_route(request)
            .await
            .expect("Failed to get price route from Liquidswap");
        assert!(response.success);
        println!(
            "Price Route: {}",
            serde_json::to_string_pretty(&response).unwrap()
        );
    }

    #[tokio::test]
    async fn test_estimate_swap_liquidswap_exact_in_success() {
        let request = create_test_request(
            TradeType::ExactIn,
            "0x5555555555555555555555555555555555555555", // WHYPE
            "0xb8ce59fc3717ada4c02eadf9682a9e934f625ebb", // USDT0
            10_000_000_000_000_000_000,
        );

        let result = estimate_swap_liquidswap_generic(request).await;

        assert!(
            result.is_ok(),
            "Expected successful estimation: {:?}",
            result.err()
        );

        let response = result.unwrap();
        assert!(response.amount_quote > 0);
    }

    #[tokio::test]
    async fn test_estimate_swap_liquidswap_exact_out_success() {
        let request = create_test_request(
            TradeType::ExactOut,
            "0x5555555555555555555555555555555555555555", // WHYPE
            "0xb8ce59fc3717ada4c02eadf9682a9e934f625ebb", // USDT0
            380_000_000,                                  // 380 USDT0 (6 decimals)
        );

        let result = estimate_swap_liquidswap_generic(request).await;

        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.amount_quote > 0);
    }

    #[tokio::test]
    async fn test_prepare_swap_liquidswap_generic_exact_in() {
        let request = create_test_swap_request(
            TradeType::ExactIn,
            "0x5555555555555555555555555555555555555555", // WHYPE
            "0xb8ce59fc3717ada4c02eadf9682a9e934f625ebb", // USDT0
            10_000_000_000_000_000_000,                   // 10 WHYPE (18 decimals)
        );

        let result = prepare_swap_liquidswap_generic(request).await;

        // This will likely fail due to the smart contract integration issues
        assert!(
            result.is_ok(),
            "Expected successful swap preparation: {:?}",
            result.err()
        );

        let response = result.unwrap();

        // Basic validations
        assert!(response.amount_quote > 0, "Expected non-zero quote amount");
        assert!(response.amount_limit > 0, "Expected non-zero limit amount");
        assert!(
            !response.tx_data.is_empty(),
            "Expected non-empty transaction data"
        );
        assert!(
            response.approve_address.is_some(),
            "Expected approval address"
        );

        println!("Swap Response: {:?}", response);
    }

    #[tokio::test]
    async fn test_prepare_swap_liquidswap_generic_exact_out() {
        let request = create_test_swap_request(
            TradeType::ExactOut,
            "0x5555555555555555555555555555555555555555", // WHYPE
            "0xb8ce59fc3717ada4c02eadf9682a9e934f625ebb", // USDT0
            10_000_000,                                   // 10 USDT0 (6 decimals)
        );

        let result = prepare_swap_liquidswap_generic(request).await;

        // This will likely fail due to the smart contract integration issues
        assert!(
            result.is_ok(),
            "Expected successful swap preparation: {:?}",
            result.err()
        );

        let response = result.unwrap();

        // Basic validations
        assert!(response.amount_quote > 0, "Expected non-zero quote amount");
        assert!(response.amount_limit > 0, "Expected non-zero limit amount");
        assert!(
            !response.tx_data.is_empty(),
            "Expected non-empty transaction data"
        );
        assert!(
            response.approve_address.is_some(),
            "Expected approval address"
        );

        println!("Swap Response: {:?}", response);
    }

    #[tokio::test]
    async fn test_liquidswap_swap_test() {
        // Get tx info
        let request = create_test_swap_request(
            TradeType::ExactOut,
            "0xb8ce59fc3717ada4c02eadf9682a9e934f625ebb", // USDT0
            "0x5555555555555555555555555555555555555555", // WHYPE
            4674186744772283,                             // 1 USDT0 (6 decimals)
        );

        let result = prepare_swap_liquidswap_generic(request).await;

        // This will likely fail due to the smart contract integration issues
        assert!(
            result.is_ok(),
            "Expected successful swap preparation: {:?}",
            result.err()
        );

        let response = result.unwrap();
        println!("Swap Response: {:?}", response);

        // // Create RPC of hyperEVM
        // let evm_private_key = match std::env::var("TEST_EVM_PKEY") {
        //     Ok(key) => key,
        //     Err(_) => {
        //         println!("TEST_EVM_PKEY environment variable not set");
        //         return;
        //     }
        // };
        // let evm_private_key_signer = PrivateKeySigner::from_str(&evm_private_key).unwrap();
        // let evm_wallet = EthereumWallet::from(evm_private_key_signer.clone());
        // let provider: Arc<CustomEvmProvider> = Arc::new(
        //     ProviderBuilder::new()
        //         .filler(IncreasedGasLimitFiller)
        //         .wallet(evm_wallet)
        //         .on_http(
        //             "https://rpc.hyperliquid.xyz/evm"
        //                 .parse::<Url>()
        //                 .expect("Failed to parse provider URL"),
        //         ),
        // );

        // // Create tx
        // let tx = TransactionRequest::default()
        //     .with_to(response.tx_to)
        //     .with_input(response.tx_data)
        //     .with_value(response.tx_value);

        // // Send tx ? Or make a call, to check if works
        // let estimation = provider.send_transaction(tx).await;
        // println!("Transaction Estimation: {:?}", estimation);
        // assert!(estimation.is_ok());
        // let estimation = estimation.unwrap();
        // println!("Gas Estimation: {:?}", estimation);
    }

    #[tokio::test]
    async fn test_liquidswap_swap_native_hype_in() {
        // Get tx info
        let request = create_test_swap_request(
            TradeType::ExactOut,
            "0x0000000000000000000000000000000000000000", // Hype
            "0x5555555555555555555555555555555555555555", // WHYPE
            4674186744772283,
        );

        let result = prepare_swap_liquidswap_generic(request).await;

        // This will likely fail due to the smart contract integration issues
        println!("Result: {:?}", result);
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.tx_value > 0,);
        println!("Swap Response: {:?}", response);
    }
}
