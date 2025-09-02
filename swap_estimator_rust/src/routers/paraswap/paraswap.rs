use std::sync::Arc;

use super::{
    get_paraswap_format_slippage,
    requests::{
        GetPriceRouteRequest, ParaswapSide, ParaswapSwapCombinedRequest, TransactionsRequest,
    },
    responses::{ParaswapResponse, PriceRoute},
};
use crate::{
    error::{Error, EstimatorResult},
    routers::{
        constants::ETH_TOKEN_DECIMALS,
        estimate::TradeType,
        paraswap::responses::{GetPriceRouteResponse, TransactionsResponse},
    },
    utils::number_conversion::decimal_string_to_u128,
};
use crate::{
    routers::{
        constants::PARASWAP_BASE_API_URL,
        estimate::{GenericEstimateRequest, GenericEstimateResponse},
        paraswap::requests::ParaswapParams,
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

lazy_static! {
    static ref HTTP_CLIENT: Arc<Client> = Arc::new(Client::new());
}

pub async fn send_paraswap_request(
    uri_path: &str,
    query: Option<Value>,
    body: Option<Value>,
    method: HttpMethod,
) -> EstimatorResult<ParaswapResponse> {
    let url = match query {
        Some(query) => {
            let query = value_to_sorted_querystring(&query).change_context(Error::ModelsError)?;
            format!("{PARASWAP_BASE_API_URL}{uri_path}?{query}")
        }
        None => format!("{PARASWAP_BASE_API_URL}{uri_path}"),
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

    let response = request
        .send()
        .await
        .change_context(Error::ReqwestError)
        .attach_printable("Error in paraswap request")?;

    let paraswap_response = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;

    Ok(paraswap_response)
}

fn handle_paraswap_response(response: ParaswapResponse) -> EstimatorResult<ParaswapResponse> {
    match response {
        ParaswapResponse::RequestError { error } => {
            tracing::error!("Request error from Paraswap: {error}");
            Err(report!(Error::ResponseError).attach_printable("Request error from Paraswap"))
        }
        ParaswapResponse::UnknownResponse(val) => {
            tracing::error!(
                "Unknown response from Paraswap: {}",
                serde_json::to_string_pretty(&val).unwrap()
            );
            Err(report!(Error::ResponseError).attach_printable("Unknown response from Paraswap"))
        }
        _ => Ok(response),
    }
}

pub async fn paraswap_prices(
    request: GetPriceRouteRequest,
) -> EstimatorResult<GetPriceRouteResponse> {
    let uri_path = "/prices";

    // Convert the request struct to a serde_json::Value to modify attribute names as specified by serde renames
    let query = serde_json::to_value(request).expect("Can't fail");

    let response = handle_paraswap_response(
        send_paraswap_request(uri_path, Some(query), None, HttpMethod::GET).await?,
    )?;
    if let ParaswapResponse::Prices(prices) = response {
        Ok(prices)
    } else {
        tracing::error!(
            "Unexpected response from Paraswap for prices request, response: {:?}",
            response
        );
        Err(report!(Error::ResponseError).attach_printable("Unexpected response from Paraswap"))
    }
}

pub async fn paraswap_transactions(
    request: TransactionsRequest,
) -> EstimatorResult<TransactionsResponse> {
    let uri_path = format!("/transactions/{}", request.chain_id);

    // Convert the request struct to a serde_json::Value to modify attribute names as specified by serde renames
    let query = serde_json::to_value(request.query_params).expect("Can't fail");

    // Convert the request struct to a serde_json::Value to modify attribute names as specified by serde renames
    let body = serde_json::to_value(request.body_params).expect("Can't fail");

    let response =
        send_paraswap_request(&uri_path, Some(query), Some(body), HttpMethod::POST).await?;
    if let ParaswapResponse::Transactions(transactions) = response {
        Ok(transactions)
    } else {
        tracing::error!(
            "Unexpected response from Paraswap for prices request, response: {:?}",
            response
        );
        Err(report!(Error::ResponseError).attach_printable("Unexpected response from Paraswap"))
    }
}

pub async fn estimate_swap_paraswap_generic(
    request: GenericEstimateRequest,
    src_token_decimals: u8,
    dst_token_decimals: u8,
) -> EstimatorResult<GenericEstimateResponse> {
    let paraswap_params = ParaswapParams {
        side: match request.trade_type {
            TradeType::ExactIn => ParaswapSide::SELL,
            TradeType::ExactOut => ParaswapSide::BUY,
        },
        chain_id: request.chain_id as u32,
        amount: request.amount_fixed,
        token_in: request.src_token.clone(),
        token_out: request.dest_token,
        token0_decimals: src_token_decimals,
        token1_decimals: dst_token_decimals,
        wallet_address: request.src_token.clone(), // This is not needed in this function
        receiver_address: request.src_token,       // This is not needed in this function
        slippage: get_paraswap_format_slippage(request.slippage),
    };
    let price_request =
        ParaswapSwapCombinedRequest::from(paraswap_params).to_get_price_route_request();

    let (amount_quote, _, _) = estimate_amount_paraswap(price_request).await?;

    Ok(GenericEstimateResponse {
        amount_quote,
        amount_limit: get_limit_amount(request.trade_type, amount_quote, request.slippage),
    })
}

/// Estimates amount OUT for exact IN swap and amount IN for exact OUT swap
///
/// ### Arguments
///
/// * `request` - Swap request data
///
/// ### Returns
///
/// * Amount OUT for exact IN swap and amount IN for exact OUT swap
/// * Route
/// * Approval address
pub async fn estimate_amount_paraswap(
    request: GetPriceRouteRequest,
) -> EstimatorResult<(u128, Value, String)> {
    let prices = paraswap_prices(request.clone()).await?;
    let price_route: PriceRoute = serde_json::from_value(prices.price_route.clone())
        .change_context(Error::SerdeSerialize(
            "Failed to deserialize Paraswap quote response".to_string(),
        ))?;
    let amount = match request.side {
        Some(side) => match side {
            ParaswapSide::BUY => price_route.src_amount.clone(),
            ParaswapSide::SELL => price_route.dest_amount.clone(),
        },
        // default SELL
        None => price_route.dest_amount.clone(),
    };

    let amount = amount.parse::<u128>().change_context(Error::ParseError)?;

    let approval_address = price_route.contract_address.clone();
    Ok((amount, prices.price_route, approval_address))
}

pub async fn prepare_swap_paraswap_generic(
    generic_swap_request: GenericSwapRequest,
    src_decimals: u8,
    dest_decimals: u8,
) -> EstimatorResult<EvmSwapResponse> {
    let request = ParaswapSwapCombinedRequest::try_from_generic_parameters(
        generic_swap_request.clone(),
        src_decimals,
        dest_decimals,
    )
    .await?;
    prepare_paraswap_swap(generic_swap_request, request).await
}

pub async fn prepare_paraswap_swap(
    generic_swap_request: GenericSwapRequest,
    mut request: ParaswapSwapCombinedRequest,
) -> EstimatorResult<EvmSwapResponse> {
    let prices_request = request.to_get_price_route_request();
    let (amount_quote, prices_response, approval_address) =
        estimate_amount_paraswap(prices_request).await?;
    let amount_limit = get_limit_amount(
        generic_swap_request.trade_type,
        amount_quote,
        generic_swap_request.slippage,
    );

    request.price_route = Some(prices_response);

    let mut transactions_request = request.to_transactions_request()?;
    match &request.side {
        Some(ParaswapSide::BUY) => {
            transactions_request.body_params.dest_amount =
                Some(generic_swap_request.amount_fixed.to_string());
        }
        _ => {
            // Sell or None, which defaults to sell
            transactions_request.body_params.src_amount =
                Some(generic_swap_request.amount_fixed.to_string());
        }
    };

    let transactions_response = paraswap_transactions(transactions_request).await?;

    Ok(EvmSwapResponse {
        amount_quote,
        amount_limit: amount_limit,
        tx_to: transactions_response.to,
        tx_data: transactions_response.data,
        tx_value: decimal_string_to_u128(&transactions_response.value, ETH_TOKEN_DECIMALS)?,
        approve_address: Some(approval_address),
        require_transfer: false,
    })
}

#[cfg(test)]
mod tests {
    use intents_models::constants::chains::ChainId;

    use super::*;

    #[tokio::test]
    async fn test_estimate_paraswap() {
        let from_token_address = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string();
        let to_token_address = "0x4200000000000000000000000000000000000006".to_string();
        let amount = 100000000;

        let request = GetPriceRouteRequest {
            src_token: from_token_address,
            src_decimals: 6,
            dest_token: to_token_address,
            amount: amount.to_string(),
            side: Some(ParaswapSide::SELL),
            chain_id: ChainId::Base as u32,
            other_exchange_prices: None,
            include_dexs: None,
            exclude_dexs: None,
            exclude_rfq: None,
            include_contract_methods: None,
            exclude_contract_methods: None,
            user_address: Some(
                "0xb5b7FeCdA25d948e62Ce397404Bf765d8b09A4c4"
                    .to_string()
                    .to_lowercase(),
            ),
            route: None,
            partner: None,
            dest_decimals: 18,
            max_impact: None,
            receiver: None,
            src_token_transfer_fee: None,
            dest_token_transfer_fee: None,
            src_token_dex_transfer_fee: None,
            dest_token_dex_transfer_fee: None,
            version: Some(6.2),
            exclude_contract_methods_without_fee_model: None,
            ignore_bad_usd_price: None,
        };

        let amount_out = estimate_amount_paraswap(request)
            .await
            .expect("Failed to estimate amount")
            .0;
        println!("Amount out: {amount_out}");

        assert!(amount_out > 0, "Amount out should be greater than zero");
    }

    #[tokio::test]
    async fn test_estimate_swap_paraswap_generic_exact_in() {
        let src_token_decimals = 6;
        let dst_token_decimals = 18;
        let request = GenericEstimateRequest {
            trade_type: TradeType::ExactIn,
            chain_id: ChainId::Base,
            src_token: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string(),
            dest_token: "0x4200000000000000000000000000000000000006".to_string(),
            amount_fixed: 100000000,
            slippage: 2.0,
        };
        let result =
            estimate_swap_paraswap_generic(request, src_token_decimals, dst_token_decimals).await;
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
    async fn test_paraswap_swap_exact_in() {
        let chain_id = ChainId::Base;
        let src_token = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string();
        let dest_token = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string();
        let src_token_decimals = 18;
        let dst_token_decimals = 6;
        let request = GenericSwapRequest {
            trade_type: TradeType::ExactIn,
            chain_id,
            spender: "0x9ecDC9aF2a8254DdE8bbce8778eFAe695044cC9F".to_string(),
            dest_address: "0x4E28f22DE1DBDe92310db2779217a74607691038".to_string(),
            src_token,
            dest_token,
            amount_fixed: 10_000_000_000u128,
            slippage: 2.0,
        };
        let result =
            prepare_swap_paraswap_generic(request, src_token_decimals, dst_token_decimals).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_paraswap_swap_exact_out() {
        let chain_id = ChainId::Base;
        let src_token = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string();
        let dest_token = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string();
        let src_token_decimals = 18;
        let dst_token_decimals = 6;
        let request = GenericSwapRequest {
            trade_type: TradeType::ExactOut,
            chain_id,
            spender: "0x9ecDC9aF2a8254DdE8bbce8778eFAe695044cC9F".to_string(),
            dest_address: "0x4E28f22DE1DBDe92310db2779217a74607691038".to_string(),
            src_token,
            dest_token,
            amount_fixed: 10_000u128,
            slippage: 2.0,
        };
        let result =
            prepare_swap_paraswap_generic(request, src_token_decimals, dst_token_decimals).await;
        assert!(result.is_ok());
    }
}
