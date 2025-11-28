use crate::routers::Slippage;
use crate::routers::estimate::TradeType;
use crate::routers::swap::EvmTxData;
use crate::routers::uniswap::requests::{
    SWAPPER_PLACEHOLDER, UniswapQuoteRequest, UniswapSwapRequest,
};
use crate::routers::uniswap::responses::{
    UniswapQuoteResponse, UniswapQuoteValue, UniswapResponse, UniswapSwapResponse,
};
use crate::utils::json::replace_strings_in_json;
use crate::utils::limit_amount::get_slippage_percentage;
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
use intents_models::network::client_rate_limit::Client;
use intents_models::network::http::{
    HttpMethod, handle_reqwest_response, value_to_sorted_querystring,
};
use serde_json::{Value, json};

pub async fn send_uniswap_request(
    client: &Client,
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

    let request = {
        let client = client.inner_client();
        let mut request = match method {
            HttpMethod::GET => client.get(url),
            HttpMethod::POST => client.post(url),
            _ => return Err(report!(Error::Unknown).attach_printable("Unknown http method")),
        };
        request = match body {
            Some(body) => request.json(&body),
            None => request,
        };
        request = request.header("x-api-key", api_key);
        request
            .build()
            .change_context(Error::ReqwestError)
            .attach_printable("Error building Uniswap request")?
    };

    let response = client
        .execute(request)
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
                serde_json::to_string_pretty(&val).unwrap_or_else(|_| format!("{:?}", val))
            );
            Err(report!(Error::ResponseError).attach_printable("Unknown response from Uniswap"))
        }
        _ => Ok(response),
    }
}

pub async fn uniswap_quote(
    client: &Client,
    request: UniswapQuoteRequest,
    api_key: &str,
) -> EstimatorResult<UniswapQuoteResponse> {
    // Convert the request struct to a serde_json::Value to modify attribute names as specified by serde renames
    let body = serde_json::to_value(request).expect("Can't fail");

    let response = handle_uniswap_response(
        send_uniswap_request(
            client,
            "/quote/",
            api_key,
            None,
            Some(body),
            HttpMethod::POST,
        )
        .await?,
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
    client: &Client,
    request: UniswapSwapRequest,
    api_key: &str,
) -> EstimatorResult<UniswapSwapResponse> {
    // Convert the request struct to a serde_json::Value to modify attribute names as specified by serde renames
    let body = serde_json::to_value(request).expect("Can't fail");

    let response = handle_uniswap_response(
        send_uniswap_request(
            client,
            "/swap/",
            api_key,
            None,
            Some(body),
            HttpMethod::POST,
        )
        .await?,
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
    client: &Client,
    request: GenericEstimateRequest,
    api_key: &str,
) -> EstimatorResult<GenericEstimateResponse> {
    let trade_type = request.trade_type;
    let slippage = request.slippage;
    let quote_request = UniswapQuoteRequest::from_generic_estimate_request(request, None);

    let quote_response = uniswap_quote(client, quote_request, api_key).await?;
    let quote_data: UniswapQuoteValue = serde_json::from_value(quote_response.quote.clone())
        .change_context(Error::AggregatorError(
            "Error deserializing Uniswap quote response data".to_string(),
        ))?;
    let amount_quote = match trade_type {
        TradeType::ExactIn => quote_data.output.amount,
        TradeType::ExactOut => quote_data.input.amount,
    }
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

pub async fn swap_uniswap_generic(
    client: &Client,
    generic_swap_request: GenericSwapRequest,
    estimate_response: Option<GenericEstimateResponse>,
    api_key: &str,
) -> EstimatorResult<EvmSwapResponse> {
    let mut quote_response = match estimate_response {
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
            let quote_response = uniswap_quote(client, prices_request, api_key).await?;

            quote_response
        }
    };
    let quote_data: UniswapQuoteValue = serde_json::from_value(quote_response.quote.clone())
        .change_context(Error::AggregatorError(
            "Error deserializing Uniswap quote response data".to_string(),
        ))?;
    let amount_quote = match generic_swap_request.trade_type {
        TradeType::ExactIn => quote_data.output.amount,
        TradeType::ExactOut => quote_data.input.amount,
    }
    .parse::<u128>()
    .change_context(Error::AggregatorError(
        "Error deserializing Uniswap quote output amount".to_string(),
    ))?;

    let approve_address = quote_response.permit_transaction.clone().map(|tx| tx.to);

    if let Slippage::AmountLimit { amount_limit, .. } = generic_swap_request.slippage {
        let slippage_percent =
            get_slippage_percentage(amount_quote, amount_limit, generic_swap_request.trade_type)?;
        quote_response.quote["slippage"] = json!(slippage_percent);
    }

    let swap_request = UniswapSwapRequest::from_quote(quote_response.quote);

    let swap_response = uniswap_swap(client, swap_request, api_key).await?;

    let amount_limit = get_limit_amount(
        generic_swap_request.trade_type,
        amount_quote,
        generic_swap_request.slippage,
    )?;

    let pre_transactions = if let Some(permit_tx) = quote_response.permit_transaction {
        Some(vec![EvmTxData {
            tx_to: permit_tx.to,
            tx_data: permit_tx.data,
            tx_value: u128::from_str_radix(permit_tx.value.trim_start_matches("0x"), 16)
                .change_context(Error::AggregatorError(
                    "Parsing Uniswap Permit tx msg.value".to_string(),
                ))?,
        }])
    } else {
        None
    };

    Ok(EvmSwapResponse {
        amount_quote,
        amount_limit,
        pre_transactions,
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

    #[tokio::test]
    async fn test_estimate_swap_uniswap_generic_exact_in() {
        dotenv::dotenv().ok();
        let api_key = dotenv::var("UNISWAP_TRADE_API_KEY").unwrap();
        let client = Client::Unrestricted(reqwest::Client::new());

        let request = GenericEstimateRequest {
            trade_type: TradeType::ExactIn,
            chain_id: ChainId::Base,
            src_token: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string(),
            dest_token: "0x4200000000000000000000000000000000000006".to_string(),
            amount_fixed: 100000000,
            slippage: Slippage::Percent(2.0),
        };
        let result = quote_uniswap_generic(&client, request, &api_key).await;
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
        let client = Client::Unrestricted(reqwest::Client::new());

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

        let swap_result = swap_uniswap_generic(&client, swap_request, None, &api_key).await;
        assert!(swap_result.is_ok());
        let result = swap_result.unwrap();
        assert!(result.approve_address.is_none());
        assert!(result.require_transfer);
        assert!(result.pre_transactions.is_none());
    }

    #[tokio::test]
    async fn test_uniswap_swap_exact_out() {
        dotenv::dotenv().ok();
        let api_key = dotenv::var("UNISWAP_TRADE_API_KEY").unwrap();
        let client = Client::Unrestricted(reqwest::Client::new());

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
        let swap_result = swap_uniswap_generic(&client, request, None, &api_key).await;
        assert!(swap_result.is_ok());
        let swap_result = swap_result.unwrap();
        assert!(swap_result.approve_address.is_some());
        assert!(swap_result.require_transfer);
        assert!(swap_result.pre_transactions.is_some());
        let pre_transactions = swap_result.pre_transactions.unwrap();
        assert_eq!(pre_transactions.len(), 1);
        assert!(swap_result.amount_quote < 1_000_000_000)
    }

    #[tokio::test]
    async fn test_uniswap_swap_exact_in_with_quote() {
        dotenv::dotenv().ok();
        let api_key = dotenv::var("UNISWAP_TRADE_API_KEY").unwrap();
        let client = Client::Unrestricted(reqwest::Client::new());

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
        let quote_result = quote_uniswap_generic(&client, quote_request, &api_key).await;
        assert!(quote_result.is_ok());
        let quote_result = quote_result.unwrap();

        let swap_result =
            swap_uniswap_generic(&client, swap_request, Some(quote_result), &api_key).await;
        assert!(swap_result.is_ok());
        let result = swap_result.unwrap();
        assert!(result.approve_address.is_none());
        assert!(result.require_transfer);
        assert!(result.pre_transactions.is_none());
    }

    #[tokio::test]
    async fn test_uniswap_swap_exact_in_with_quote_amount_limit() {
        dotenv::dotenv().ok();
        let chain_id = ChainId::Base;
        let api_key = dotenv::var("UNISWAP_TRADE_API_KEY").unwrap();
        let client = Client::Unrestricted(reqwest::Client::new());

        let src_token = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string();
        let dest_token = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string();
        let mut swap_request = GenericSwapRequest {
            trade_type: TradeType::ExactIn,
            chain_id,
            spender: "0x9ecDC9aF2a8254DdE8bbce8778eFAe695044cC9F".to_string(),
            dest_address: "0x4E28f22DE1DBDe92310db2779217a74607691038".to_string(),
            src_token,
            dest_token,
            amount_fixed: 1_000_000_000_000_000_000u128,
            slippage: Slippage::AmountLimit {
                amount_limit: 1_000,
                fallback_slippage: 2.0,
            },
        };

        let quote_request: GenericEstimateRequest = swap_request.clone().into();
        let quote_result = quote_uniswap_generic(&client, quote_request, &api_key).await;
        assert!(quote_result.is_ok());
        let quote_result = quote_result.unwrap();

        // Setting to 5%
        let amount_limit = quote_result.amount_quote * 95 / 100;
        swap_request.slippage = Slippage::AmountLimit {
            amount_limit,
            fallback_slippage: 2.0,
        };

        let swap_result =
            swap_uniswap_generic(&client, swap_request, Some(quote_result), &api_key).await;
        assert!(swap_result.is_ok());
        let result = swap_result.unwrap();
        assert!(result.approve_address.is_none());
        assert!(result.require_transfer);
        assert!(result.pre_transactions.is_none());
        assert_eq!(result.amount_limit, amount_limit);
    }
}
