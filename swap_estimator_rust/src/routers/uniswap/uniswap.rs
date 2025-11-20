use crate::routers::uniswap::requests::{
    SWAPPER_PLACEHOLDER, UniswapQuoteRequest, UniswapSwapRequest,
};
use crate::routers::uniswap::responses::{
    UniswapQuoteResponse, UniswapQuoteValue, UniswapResponse, UniswapSwapResponse,
};
use crate::utils::json::replace_strings_in_json;
use crate::{
    error::{Error, EstimatorResult},
    routers::RouterType,
};
use crate::{
    routers::{
        constants::BASE_UNISWAP_API_URL,
        estimate::{GenericEstimateRequest, GenericEstimateResponse},
        swap::{EvmSwapResponse, GenericSwapRequest},
    },
    utils::limit_amount::get_limit_amount,
};
use error_stack::{ResultExt, report};
use intents_models::network::http::{
    HttpMethod, handle_reqwest_response, value_to_sorted_querystring,
};
use lazy_static::lazy_static;
use reqwest::Client;
use serde_json::Value;
use std::sync::Arc;

lazy_static! {
    static ref HTTP_CLIENT: Arc<Client> = Arc::new(Client::new());
}

pub async fn send_uniswap_request(
    uri_path: &str,
    api_key: &str,
    query: Option<Value>,
    body: Option<Value>,
    method: HttpMethod,
) -> EstimatorResult<UniswapResponse> {
    let url = match query {
        Some(query) => {
            let query = value_to_sorted_querystring(&query).change_context(Error::ModelsError)?;
            format!("{BASE_UNISWAP_API_URL}{uri_path}?{query}")
        }
        None => format!("{BASE_UNISWAP_API_URL}{uri_path}"),
    };

    let mut request = match method {
        HttpMethod::GET => HTTP_CLIENT.get(url),
        HttpMethod::POST => HTTP_CLIENT.post(url),
        _ => return Err(report!(Error::Unknown).attach_printable("Unknown http method")),
    };

    request = match body {
        Some(body) => request.json(&body),
        None => request,
    };

    request = request.header("x-api-key", api_key);

    let response = request
        .send()
        .await
        .change_context(Error::ReqwestError)
        .attach_printable("Error in Uniswap request")?;

    let uniswap_response = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;

    Ok(uniswap_response)
}

fn handle_uniswap_response(response: UniswapResponse) -> EstimatorResult<UniswapResponse> {
    match response {
        UniswapResponse::RequestError { error } => {
            tracing::error!("Request error from Uniswap: {error}");
            Err(report!(Error::ResponseError).attach_printable("Request error from Uniswap"))
        }
        UniswapResponse::UnknownResponse(val) => {
            tracing::error!(
                "Unknown response from Uniswap: {}",
                serde_json::to_string_pretty(&val).unwrap()
            );
            Err(report!(Error::ResponseError).attach_printable("Unknown response from Uniswap"))
        }
        _ => Ok(response),
    }
}

pub async fn uniswap_quote(
    request: UniswapQuoteRequest,
    api_key: &str,
) -> EstimatorResult<UniswapQuoteResponse> {
    // Convert the request struct to a serde_json::Value to modify attribute names as specified by serde renames
    let body = serde_json::to_value(request).expect("Can't fail");

    let response = handle_uniswap_response(
        send_uniswap_request("/quote/", api_key, None, Some(body), HttpMethod::POST).await?,
    )?;
    if let UniswapResponse::Quote(quote_response) = response {
        Ok(quote_response)
    } else {
        tracing::error!(
            "Unexpected response from Uniswap /quote request, response: {:?}",
            response
        );
        Err(report!(Error::ResponseError).attach_printable("Unexpected response from Uniswap"))
    }
}

pub async fn uniswap_swap(
    request: UniswapSwapRequest,
    api_key: &str,
) -> EstimatorResult<UniswapSwapResponse> {
    // Convert the request struct to a serde_json::Value to modify attribute names as specified by serde renames
    let body = serde_json::to_value(request).expect("Can't fail");

    let response = handle_uniswap_response(
        send_uniswap_request("/swap/", api_key, None, Some(body), HttpMethod::POST).await?,
    )?;
    if let UniswapResponse::Swap(swap_response) = response {
        Ok(swap_response)
    } else {
        tracing::error!(
            "Unexpected response from Uniswap /swap request, response: {:?}",
            response
        );
        Err(report!(Error::ResponseError).attach_printable("Unexpected response from Uniswap"))
    }
}

pub async fn quote_uniswap_generic(
    request: GenericEstimateRequest,
    api_key: &str,
) -> EstimatorResult<GenericEstimateResponse> {
    let trade_type = request.trade_type;
    let slippage = request.slippage;
    let quote_request = UniswapQuoteRequest::from_generic_estimate_request(request, None);

    let quote_response = uniswap_quote(quote_request, api_key).await?;
    let quote_data: UniswapQuoteValue = serde_json::from_value(quote_response.quote.clone())
        .change_context(Error::AggregatorError(
            "Error deserializing Uniswap quote response data".to_string(),
        ))?;
    let amount_quote =
        quote_data
            .output
            .amount
            .parse::<u128>()
            .change_context(Error::AggregatorError(
                "Error deserializing Uniswap quote output amount".to_string(),
            ))?;

    let amount_limit = get_limit_amount(trade_type, amount_quote, slippage)?;

    Ok(GenericEstimateResponse {
        amount_quote,
        amount_limit,
        router: RouterType::Uniswap,
        router_data: serde_json::to_value(quote_response).change_context(
            Error::AggregatorError("Error serializing Uniswap quote response".to_string()),
        )?,
    })
}

// todo test::::
// todo uniswap: set swapper
// todo uniswap: set output.recipient
// todo uniswap: set aggregatedOutputs: recipient

// todo uniswap: set limit amount slippage

pub async fn swap_uniswap_generic(
    generic_swap_request: GenericSwapRequest,
    estimate_response: Option<GenericEstimateResponse>,
    api_key: &str,
) -> EstimatorResult<EvmSwapResponse> {
    let quote_response = match estimate_response {
        Some(estimate_response) => {
            let mut quote_response: UniswapQuoteResponse = serde_json::from_value(
                estimate_response.router_data,
            )
            .change_context(Error::AggregatorError(
                "Error deserializing Uniswap quote response data".to_string(),
            ))?;

            quote_response.quote = replace_strings_in_json(
                quote_response.quote,
                SWAPPER_PLACEHOLDER,
                &generic_swap_request.spender,
            );

            quote_response
        }
        None => {
            let generic_estimate_request =
                GenericEstimateRequest::from(generic_swap_request.clone());

            let prices_request = UniswapQuoteRequest::from_generic_estimate_request(
                generic_estimate_request,
                Some(generic_swap_request.spender.clone()),
            );
            let quote_response = uniswap_quote(prices_request, api_key).await?;

            quote_response
        }
    };
    let quote_data: UniswapQuoteValue = serde_json::from_value(quote_response.quote.clone())
        .change_context(Error::AggregatorError(
            "Error deserializing Uniswap quote response data".to_string(),
        ))?;
    let amount_quote =
        quote_data
            .output
            .amount
            .parse::<u128>()
            .change_context(Error::AggregatorError(
                "Error deserializing Uniswap quote output amount".to_string(),
            ))?;

    let approve_address = quote_response.permit_transaction.clone().map(|tx| tx.to);

    // todo permitApprovalTx

    // todo slippage
    // todo uniswap: set limit amount slippage

    let swap_request = UniswapSwapRequest::from_quote(quote_response.quote);

    let swap_response = uniswap_swap(swap_request, api_key).await?;

    let amount_limit = get_limit_amount(
        generic_swap_request.trade_type,
        amount_quote,
        generic_swap_request.slippage,
    )?;

    Ok(EvmSwapResponse {
        amount_quote,
        amount_limit,
        tx_to: swap_response.swap.to,
        tx_data: swap_response.swap.data,
        tx_value: u128::from_str_radix(swap_response.swap.value.trim_start_matches("0x"), 16)
            .change_context(Error::AggregatorError(
                "Parsing Uniswap msg.value".to_string(),
            ))?,
        approve_address,
        // Uniswap API sends tokens to msg.sender
        require_transfer: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routers::{Slippage, estimate::TradeType};
    use intents_models::constants::chains::ChainId;

    //     #[tokio::test]
    //     async fn test_estimate_uniswap() {
    //         let from_token_address = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string();
    //         let to_token_address = "0x4200000000000000000000000000000000000006".to_string();
    //         let amount = 100000000;
    //
    //         let request = GetPriceRouteRequest {
    //             src_token: from_token_address,
    //             src_decimals: 6,
    //             dest_token: to_token_address,
    //             amount: amount.to_string(),
    //             side: Some(uniswapSide::SELL),
    //             chain_id: (ChainId::Base as u32).to_string(),
    //             user_address: Some(
    //                 "0xb5b7FeCdA25d948e62Ce397404Bf765d8b09A4c4"
    //                     .to_string()
    //                     .to_lowercase(),
    //             ),
    //             dest_decimals: 18,
    //             max_impact: None,
    //             receiver: None,
    //             version: Some(6.2),
    //             exclude_dexs: Some("uniswapPool,uniswapLimitOrders".to_string()), // Had to add this to set ignoreChecks as true on transaction request
    //         };
    //
    //         let amount_out = estimate_amount_uniswap(request)
    //             .await
    //             .expect("Failed to estimate amount")
    //             .0;
    //         println!("Amount out: {amount_out}");
    //
    //         assert!(amount_out > 0, "Amount out should be greater than zero");
    //     }
    //
    #[tokio::test]
    async fn test_estimate_swap_uniswap_generic_exact_in() {
        dotenv::dotenv().ok();
        let api_key = dotenv::var("UNISWAP_TRADE_API_KEY").unwrap();

        let request = GenericEstimateRequest {
            trade_type: TradeType::ExactIn,
            chain_id: ChainId::Base,
            src_token: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string(),
            dest_token: "0x4200000000000000000000000000000000000006".to_string(),
            amount_fixed: 100000000,
            slippage: Slippage::Percent(2.0),
        };
        let result = quote_uniswap_generic(request, &api_key).await;
        assert!(
            result.is_ok(),
            "Expected a successful estimate swap response"
        );
        let response = result.unwrap();
        println!("Response: {response:?}");
        assert!(
            response.amount_quote > 0,
            "Expected a non-zero amount quote"
        );
    }

    #[tokio::test]
    async fn test_uniswap_swap_exact_in() {
        dotenv::dotenv().ok();
        let api_key = dotenv::var("UNISWAP_TRADE_API_KEY").unwrap();

        let chain_id = ChainId::Base;
        let src_token = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string();
        let dest_token = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string();
        let swap_request = GenericSwapRequest {
            trade_type: TradeType::ExactIn,
            chain_id,
            spender: "0x9ecDC9aF2a8254DdE8bbce8778eFAe695044cC9F".to_string(),
            dest_address: "0x4E28f22DE1DBDe92310db2779217a74607691038".to_string(),
            src_token,
            dest_token,
            amount_fixed: 10_000_000_000u128,
            slippage: Slippage::Percent(2.0),
        };

        let swap_result = swap_uniswap_generic(swap_request, None, &api_key).await;
        assert!(swap_result.is_ok());
        let result = swap_result.unwrap();
        assert!(result.approve_address.is_none());
        assert!(result.require_transfer);
        // todo uniswap: test pre_transactions
    }

    #[tokio::test]
    async fn test_uniswap_swap_exact_out() {
        dotenv::dotenv().ok();
        let api_key = dotenv::var("UNISWAP_TRADE_API_KEY").unwrap();

        let chain_id = ChainId::Base;
        let src_token = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string();
        let dest_token = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string();
        let request = GenericSwapRequest {
            trade_type: TradeType::ExactOut,
            chain_id,
            spender: "0x9ecDC9aF2a8254DdE8bbce8778eFAe695044cC9F".to_string(),
            dest_address: "0x4E28f22DE1DBDe92310db2779217a74607691038".to_string(),
            src_token,
            dest_token,
            amount_fixed: 10_000_000_000_000_000u128,
            slippage: Slippage::Percent(2.0),
        };
        let swap_result = swap_uniswap_generic(request, None, &api_key).await;
        assert!(swap_result.is_ok());
        let swap_result = swap_result.unwrap();
        assert!(swap_result.approve_address.is_some());
        assert!(swap_result.require_transfer);
        // todo uniswap: test pre_transactions
    }

    #[tokio::test]
    async fn test_uniswap_swap_exact_in_with_quote() {
        dotenv::dotenv().ok();
        let api_key = dotenv::var("UNISWAP_TRADE_API_KEY").unwrap();

        let chain_id = ChainId::Base;
        let src_token = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string();
        let dest_token = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string();
        let swap_request = GenericSwapRequest {
            trade_type: TradeType::ExactIn,
            chain_id,
            spender: "0x9ecDC9aF2a8254DdE8bbce8778eFAe695044cC9F".to_string(),
            dest_address: "0x4E28f22DE1DBDe92310db2779217a74607691038".to_string(),
            src_token,
            dest_token,
            amount_fixed: 10_000_000_000u128,
            slippage: Slippage::Percent(2.0),
        };

        let quote_request: GenericEstimateRequest = swap_request.clone().into();
        let quote_result = quote_uniswap_generic(quote_request, &api_key).await;
        assert!(quote_result.is_ok());
        let quote_result = quote_result.unwrap();

        let swap_result = swap_uniswap_generic(swap_request, Some(quote_result), &api_key).await;
        assert!(swap_result.is_ok());
        let result = swap_result.unwrap();
        assert!(result.approve_address.is_none());
        assert!(result.require_transfer);
        // todo uniswap: test pre_transactions
    }

    //     #[tokio::test]
    //     async fn test_uniswap_swap_exact_in_with_quote_amount_limit() {
    //         let chain_id = ChainId::Base;
    //         let src_token = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string();
    //         let dest_token = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string();
    //         let src_token_decimals = 18;
    //         let dst_token_decimals = 6;
    //         let request = GenericSwapRequest {
    //             trade_type: TradeType::ExactIn,
    //             chain_id,
    //             spender: "0x9ecDC9aF2a8254DdE8bbce8778eFAe695044cC9F".to_string(),
    //             dest_address: "0x4E28f22DE1DBDe92310db2779217a74607691038".to_string(),
    //             src_token,
    //             dest_token,
    //             amount_fixed: 10_000_000_000u128,
    //             slippage: Slippage::AmountLimit {
    //                 amount_limit: 20,
    //                 fallback_slippage: 2.0,
    //             },
    //         };
    //
    //         let generic_estimate_request = GenericEstimateRequest::from(request.clone());
    //         let result = estimate_swap_uniswap_generic(
    //             generic_estimate_request,
    //             src_token_decimals,
    //             dst_token_decimals,
    //         )
    //             .await;
    //         assert!(
    //             result.is_ok(),
    //             "Expected a successful estimate swap response"
    //         );
    //         let response = result.unwrap();
    //
    //         let result = prepare_swap_uniswap_generic(
    //             request,
    //             src_token_decimals,
    //             dst_token_decimals,
    //             Some(response),
    //         )
    //             .await;
    //         println!("Result: {:#?}", result);
    //         assert!(result.is_ok());
    //     }
}
