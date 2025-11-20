use crate::utils::exact_in_reverse_quoter::{ReverseQuoteResult, quote_exact_out_with_exact_in};
use crate::{
    error::{Error, EstimatorResult},
    routers::{
        RouterType, Slippage,
        estimate::{GenericEstimateRequest, GenericEstimateResponse, TradeType},
        swap::{EvmSwapResponse, GenericSwapRequest},
        zero_x::{
            BASE_ZERO_X_API_URL,
            requests::{ZeroXGetPriceRequest, ZeroXGetQuoteRequest},
            responses::{ZeroXApiResponse, ZeroXGetPriceResponse, ZeroXGetQuoteResponse},
        },
    },
    utils::{
        limit_amount::get_slippage_percentage,
        number_conversion::{decimal_string_to_u128, slippage_to_bps},
    },
};
use error_stack::{ResultExt as _, report};
use intents_models::constants::chains::is_native_token_evm_address;
use intents_models::network::client_rate_limit::Client;
use intents_models::network::http::{handle_reqwest_response, value_to_sorted_querystring};
use serde_json::json;

pub fn update_zero_x_native_token(token_address: String) -> String {
    if is_native_token_evm_address(&token_address) {
        "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string()
    } else {
        token_address
    }
}

fn handle_zero_x_response(response: ZeroXApiResponse) -> EstimatorResult<ZeroXApiResponse> {
    match response {
        ZeroXApiResponse::LiquidityResponse(_) => Err(report!(Error::AggregatorError(
            "No liquidity available for Zero X swap".to_string()
        ))),
        _ => Ok(response),
    }
}

pub async fn zero_x_get_price(
    client: &Client,
    api_key: &str,
    request: ZeroXGetPriceRequest,
) -> EstimatorResult<ZeroXGetPriceResponse> {
    let query = json!({
        "chainId": request.chain_id,
        "buyToken": update_zero_x_native_token(request.buy_token),
        "sellToken": update_zero_x_native_token(request.sell_token),
        "sellAmount": request.sell_amount,
        "slippageBps": request.slippage_bps,
    });

    let query_string = value_to_sorted_querystring(&query).change_context(Error::ParseError)?;
    let url = format!("{BASE_ZERO_X_API_URL}/allowance-holder/price?{query_string}",);

    let request = client
        .inner_client()
        .get(&url)
        .header("0x-api-key", api_key)
        .header("0x-version", "v2")
        .build()
        .change_context(Error::ReqwestError)
        .attach_printable("Error building 0x request")?;

    let response = client
        .execute(request)
        .await
        .change_context(Error::ReqwestError)
        .attach_printable("Error in 0x request")?;

    let get_price_response: ZeroXApiResponse = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;

    if let ZeroXApiResponse::GetPriceResponse(res) = handle_zero_x_response(get_price_response)? {
        Ok(res)
    } else {
        Err(report!(Error::AggregatorError(
            "Expected GetPriceResponse variant from ZeroXApiResponse".to_string()
        ))
        .attach_printable("Expected GetPriceResponse variant from ZeroXApiResponse"))
    }
}

pub async fn zero_x_get_quote(
    client: &Client,
    api_key: &str,
    request: ZeroXGetQuoteRequest,
) -> EstimatorResult<ZeroXGetQuoteResponse> {
    let mut query = json!({
        "chainId": request.chain_id,
        "buyToken": update_zero_x_native_token(request.buy_token),
        "sellToken": update_zero_x_native_token(request.sell_token),
        "sellAmount": request.sell_amount,
        "slippageBps": request.slippage_bps,
        "taker": request.taker,
    });

    if let Some(tx_origin) = request.tx_origin {
        query["txOrigin"] = json!(tx_origin);
    }

    if let Some(recipient) = request.recipient {
        query["recipient"] = json!(recipient);
    }

    let query_string = value_to_sorted_querystring(&query).change_context(Error::ParseError)?;
    let url = format!("{BASE_ZERO_X_API_URL}/allowance-holder/quote?{query_string}",);

    let request = client
        .inner_client()
        .get(&url)
        .header("0x-api-key", api_key)
        .header("0x-version", "v2")
        .build()
        .change_context(Error::ReqwestError)
        .attach_printable("Error building 0x request")?;

    let response = client
        .execute(request)
        .await
        .change_context(Error::ReqwestError)
        .attach_printable("Error in 0x request")?;

    let get_quote_response: ZeroXApiResponse = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;

    if let ZeroXApiResponse::GetQuoteResponse(res) = handle_zero_x_response(get_quote_response)? {
        Ok(res)
    } else {
        Err(report!(Error::AggregatorError(
            "Expected GetQuoteResponse variant from ZeroXApiResponse".to_string()
        ))
        .attach_printable("Expected GetQuoteResponse variant from ZeroXApiResponse"))
    }
}

pub async fn estimate_swap_zero_x(
    client: &Client,
    api_key: &str,
    estimator_request: GenericEstimateRequest,
    prev_result: Option<ReverseQuoteResult>,
) -> EstimatorResult<GenericEstimateResponse> {
    match estimator_request.trade_type {
        TradeType::ExactIn => {
            estimate_exact_in_swap_zero_x(client, api_key, estimator_request).await
        }
        TradeType::ExactOut => {
            let (response, _) = quote_exact_out_with_exact_in(
                estimator_request,
                async |generic_estimate_request: GenericEstimateRequest| {
                    let res =
                        estimate_exact_in_swap_zero_x(client, &api_key, generic_estimate_request)
                            .await?;

                    Ok(res)
                },
                prev_result,
            )
            .await?;

            Ok(response)
        }
    }
}

async fn estimate_exact_in_swap_zero_x(
    client: &Client,
    api_key: &str,
    estimator_request: GenericEstimateRequest,
) -> EstimatorResult<GenericEstimateResponse> {
    let slippage_bps = match estimator_request.slippage {
        Slippage::Percent(percent) => {
            let bps = slippage_to_bps(percent)?;
            if bps > 10_000 {
                return Err(report!(Error::AggregatorError(
                    "Slippage percent cannot be more than 100%".to_string()
                )));
            }
            bps
        }
        Slippage::AmountLimit {
            amount_limit: _,
            fallback_slippage,
        } => slippage_to_bps(fallback_slippage)?,
        Slippage::MaxSlippage => 10000, // 100%
    };

    let request = ZeroXGetPriceRequest {
        chain_id: estimator_request.chain_id as u32,
        sell_token: estimator_request.src_token,
        buy_token: estimator_request.dest_token,
        sell_amount: estimator_request.amount_fixed.to_string(),
        slippage_bps,
    };

    let price_response = zero_x_get_price(client, api_key, request).await?;

    let amount_out = decimal_string_to_u128(&price_response.buy_amount, 0)?;

    let amount_limit = decimal_string_to_u128(&price_response.min_buy_amount, 0)?;

    Ok(GenericEstimateResponse {
        amount_quote: amount_out,
        amount_limit,
        router: RouterType::ZeroX,
        router_data: serde_json::Value::Null,
    })
}

pub async fn prepare_swap_zero_x(
    client: &Client,
    api_key: &str,
    swap_request: GenericSwapRequest,
    prev_result: Option<ReverseQuoteResult>,
    amount_estimated: Option<u128>,
    tx_origin: Option<String>,
) -> EstimatorResult<EvmSwapResponse> {
    match swap_request.trade_type {
        TradeType::ExactIn => {
            prepare_exact_in_swap_zero_x(client, api_key, swap_request, amount_estimated, tx_origin)
                .await
        }
        TradeType::ExactOut => {
            let (response, _) = quote_exact_out_with_exact_in(
                swap_request,
                async |swap_request: GenericSwapRequest| {
                    let res = prepare_exact_in_swap_zero_x(
                        client,
                        api_key,
                        swap_request,
                        amount_estimated,
                        tx_origin.clone(),
                    )
                    .await?;

                    Ok(res)
                },
                prev_result,
            )
            .await?;

            Ok(response)
        }
    }
}

async fn prepare_exact_in_swap_zero_x(
    client: &Client,
    api_key: &str,
    swap_request: GenericSwapRequest,
    amount_estimated: Option<u128>,
    tx_origin: Option<String>,
) -> EstimatorResult<EvmSwapResponse> {
    let slippage_bps = match swap_request.slippage {
        Slippage::Percent(percent) => {
            let bps = slippage_to_bps(percent)?;
            if bps > 10_000 {
                return Err(report!(Error::AggregatorError(
                    "Slippage percent cannot be more than 100%".to_string()
                )));
            }
            bps
        }
        Slippage::AmountLimit {
            amount_limit,
            fallback_slippage,
        } => match amount_estimated {
            Some(amount_estimated) => {
                let percent = get_slippage_percentage(
                    amount_estimated,
                    amount_limit,
                    swap_request.trade_type,
                )?;
                slippage_to_bps(percent)?
            }
            None => slippage_to_bps(fallback_slippage)?,
        },
        Slippage::MaxSlippage => 10000, // 100%
    };

    let request = ZeroXGetQuoteRequest {
        chain_id: swap_request.chain_id as u32,
        sell_token: swap_request.src_token,
        buy_token: swap_request.dest_token,
        sell_amount: swap_request.amount_fixed.to_string(),
        slippage_bps,
        recipient: if swap_request.dest_address == swap_request.spender {
            None
        } else {
            Some(swap_request.dest_address)
        },
        taker: swap_request.spender,
        tx_origin,
    };

    let quote_response = zero_x_get_quote(client, api_key, request).await?;

    let amount_out = decimal_string_to_u128(&quote_response.buy_amount, 0)?;

    let amount_limit = decimal_string_to_u128(&quote_response.min_buy_amount, 0)?;

    Ok(EvmSwapResponse {
        amount_quote: amount_out,
        amount_limit,
        tx_to: quote_response.transaction.to.clone(),
        tx_data: quote_response.transaction.data,
        tx_value: decimal_string_to_u128(&quote_response.transaction.value, 0)?,
        approve_address: Some(quote_response.allowance_target),
        require_transfer: false,
    })
}

#[cfg(test)]
mod tests {
    use intents_models::constants::chains::ChainId;

    use super::*;

    #[tokio::test]
    async fn test_zero_x_get_price() {
        dotenv::dotenv().ok();

        let zero_x_api_key = std::env::var("ZERO_X_API_KEY").expect("ZERO_X_API_KEY must be set");
        let client = Client::Unrestricted(reqwest::Client::new());
        let request = ZeroXGetPriceRequest {
            chain_id: ChainId::Base as u32,
            sell_token: "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string(),
            buy_token: "0x0555E30da8f98308EdB960aa94C0Db47230d2B9c".to_string(),
            sell_amount: "1000000".to_string(), // 1 USDC
            slippage_bps: 100,                  // 1%
        };

        let result = zero_x_get_price(&client, &zero_x_api_key, request).await;
        println!("Result: {:#?}", result);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_zero_x_get_quote() {
        dotenv::dotenv().ok();

        let zero_x_api_key = std::env::var("ZERO_X_API_KEY").expect("ZERO_X_API_KEY must be set");
        let client = Client::Unrestricted(reqwest::Client::new());
        let request = ZeroXGetQuoteRequest {
            chain_id: ChainId::Base as u32,
            sell_token: "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string(),
            buy_token: "0x0555E30da8f98308EdB960aa94C0Db47230d2B9c".to_string(),
            sell_amount: "1000000".to_string(), // 1 USDC
            taker: "0x9ecdc9af2a8254dde8bbce8778efae695044cc9f".to_string(),
            slippage_bps: 100, // 1%
            recipient: None,
            tx_origin: None,
        };

        let result = zero_x_get_quote(&client, &zero_x_api_key, request).await;
        println!("Result: {:#?}", result);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_zero_x_swap_exact_in() {
        dotenv::dotenv().ok();

        let zero_x_api_key = std::env::var("ZERO_X_API_KEY").expect("ZERO_X_API_KEY must be set");
        let chain_id = ChainId::Bsc;
        let src_token = "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c".to_string();
        let dest_token = "0x55d398326f99059ff775485246999027b3197955".to_string();
        let request = GenericSwapRequest {
            trade_type: TradeType::ExactIn,
            chain_id,
            spender: "0x9ecDC9aF2a8254DdE8bbce8778eFAe695044cC9F".to_string(),
            dest_address: "0x4E28f22DE1DBDe92310db2779217a74607691038".to_string(),
            src_token,
            dest_token,
            amount_fixed: 10_000_000_000u128,
            slippage: Slippage::Percent(2.0),
        };

        let client = Client::Unrestricted(reqwest::Client::new());

        let generic_estimate_request = GenericEstimateRequest::from(request.clone());
        let result =
            estimate_swap_zero_x(&client, &zero_x_api_key, generic_estimate_request, None).await;
        assert!(
            result.is_ok(),
            "Expected a successful estimate swap response"
        );
        println!("Result: {:#?}", result);
        let prev_res: Option<ReverseQuoteResult> =
            serde_json::from_value(result.unwrap().router_data).unwrap();
        assert!(prev_res.is_none());

        let result =
            prepare_swap_zero_x(&client, &zero_x_api_key, request, prev_res, None, None).await;
        println!("Result: {:#?}", result);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_zero_x_swap_exact_out() {
        dotenv::dotenv().ok();

        let zero_x_api_key = std::env::var("ZERO_X_API_KEY").expect("ZERO_X_API_KEY must be set");
        let chain_id = ChainId::Bsc;
        let src_token = "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c".to_string();
        let dest_token = "0x55d398326f99059ff775485246999027b3197955".to_string();
        let request = GenericSwapRequest {
            trade_type: TradeType::ExactOut,
            chain_id,
            spender: "0x9ecDC9aF2a8254DdE8bbce8778eFAe695044cC9F".to_string(),
            dest_address: "0x4E28f22DE1DBDe92310db2779217a74607691038".to_string(),
            src_token,
            dest_token,
            // 10 Million USDT
            amount_fixed: 10_000_000_000_000_000_000_000_000u128,
            slippage: Slippage::Percent(2.0),
        };

        let client = Client::Unrestricted(reqwest::Client::new());

        let generic_estimate_request = GenericEstimateRequest::from(request.clone());
        let result =
            estimate_swap_zero_x(&client, &zero_x_api_key, generic_estimate_request, None).await;
        assert!(
            result.is_ok(),
            "Expected a successful estimate swap response"
        );
        println!("Result: {:#?}", result);
        let prev_res = serde_json::from_value(result.unwrap().router_data).unwrap();

        let result =
            prepare_swap_zero_x(&client, &zero_x_api_key, request, prev_res, None, None).await;
        println!("Result: {:#?}", result);
        assert!(result.is_ok());
    }
}
