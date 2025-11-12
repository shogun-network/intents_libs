use error_stack::{ResultExt as _, report};
use intents_models::network::http::{handle_reqwest_response, value_to_sorted_querystring};
use reqwest::Client;
use serde_json::json;

use crate::utils::exact_in_reverse_quoter::quote_exact_out_with_exact_in;
use crate::{
    error::{Error, EstimatorResult},
    routers::{
        RouterType, Slippage,
        estimate::{GenericEstimateRequest, GenericEstimateResponse, TradeType},
        one_inch::{
            BASE_1INCH_API_URL,
            requests::{OneInchGetQuoteRequest, OneInchSwapRequest},
            responses::{OneInchApproveResponse, OneInchGetQuoteResponse, OneInchSwapResponse},
        },
        swap::{EvmSwapResponse, GenericSwapRequest},
    },
    utils::{limit_amount::get_limit_amount, number_conversion::decimal_string_to_u128},
};

pub async fn one_inch_get_quote(
    client: Client,
    api_key: &str,
    request: OneInchGetQuoteRequest,
) -> EstimatorResult<u128> {
    let query = json!({
        "src": request.src,
        "dst": request.dst,
        "amount": request.amount,
    });

    let query_string = value_to_sorted_querystring(&query).change_context(Error::ParseError)?;

    let chain = request.chain;

    let url = format!("{BASE_1INCH_API_URL}/{chain}/quote?{query_string}",);

    let response = client
        .get(&url)
        .bearer_auth(api_key)
        .send()
        .await
        .change_context(Error::ReqwestError)
        .attach_printable("Error in 1inch request")?;

    let get_quote_response: OneInchGetQuoteResponse = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;

    decimal_string_to_u128(&get_quote_response.dst_amount, 0)
}

pub async fn one_inch_swap(
    client: Client,
    api_key: &str,
    request: OneInchSwapRequest,
) -> EstimatorResult<OneInchSwapResponse> {
    let mut query = json!({
        "src": request.src,
        "dst": request.dst,
        "amount": request.amount,
        "from": request.from,
        "origin": request.origin,
        "disableEstimate": true,
    });

    if let Some(slippage) = request.slippage {
        query["slippage"] = json!(slippage);
    } else if let Some(min_return) = request.min_return {
        query["minReturn"] = json!(min_return);
    } else {
        return Err(report!(Error::ParseError)
            .attach_printable("Either slippage or minReturn must be provided"));
    }

    let query_string = value_to_sorted_querystring(&query).change_context(Error::ParseError)?;

    let chain = request.chain;

    let url = format!("{BASE_1INCH_API_URL}/{chain}/swap?{query_string}",);

    let response = client
        .get(&url)
        .bearer_auth(api_key)
        .send()
        .await
        .change_context(Error::ReqwestError)
        .attach_printable("Error in 1inch request")?;

    let swap_response: OneInchSwapResponse = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;

    Ok(swap_response)
}

pub async fn one_inch_get_approve_address(
    client: Client,
    api_key: &str,
    chain: u32,
) -> EstimatorResult<String> {
    let url = format!("{BASE_1INCH_API_URL}/{chain}/approve/spender");

    let response = client
        .get(&url)
        .bearer_auth(api_key)
        .send()
        .await
        .change_context(Error::ReqwestError)
        .attach_printable("Error in 1inch request")?;

    let resp_json: OneInchApproveResponse = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;

    Ok(resp_json.address)
}

pub fn estimate_swap_one_inch(
    client: Client,
    api_key: &str,
    estimator_request: GenericEstimateRequest,
) -> impl Future<Output = EstimatorResult<GenericEstimateResponse>> + Send {
    let api_key = api_key.to_owned();
    async {
        match estimator_request.trade_type {
            TradeType::ExactIn => {
                let request = OneInchGetQuoteRequest {
                    chain: estimator_request.chain_id as u32,
                    src: estimator_request.src_token,
                    dst: estimator_request.dest_token,
                    amount: estimator_request.amount_fixed.to_string(),
                };

                let amount_out = one_inch_get_quote(&client, &api_key, request).await?;

                let amount_limit = get_limit_amount(
                    estimator_request.trade_type,
                    amount_out,
                    estimator_request.slippage,
                )?;

                Ok(GenericEstimateResponse {
                    amount_quote: amount_out,
                    amount_limit,
                    router: RouterType::OneInch,
                    router_data: serde_json::Value::Null,
                })
            }
            TradeType::ExactOut => {
                let (response, _) = quote_exact_out_with_exact_in(
                    estimator_request,
                    move |generic_estimate_request: GenericEstimateRequest| {
                        let client = client.clone();
                        let api_key = api_key.clone();
                        async move {
                            Box::pin(estimate_swap_one_inch(
                                client,
                                &api_key,
                                generic_estimate_request,
                            ))
                            .await
                        }
                    },
                )
                .await?;

                Ok(response)
            }
        }
    }
}

pub fn prepare_swap_one_inch(
    client: Client,
    api_key: &str,
    swap_request: GenericSwapRequest,
) -> impl Future<Output = EstimatorResult<EvmSwapResponse>> + Send {
    let api_key = api_key.to_owned();
    async {
        match swap_request.trade_type {
            TradeType::ExactIn => {
                let mut request = OneInchSwapRequest {
                    chain: swap_request.chain_id as u32,
                    src: swap_request.src_token,
                    dst: swap_request.dest_token,
                    amount: swap_request.amount_fixed.to_string(),
                    from: swap_request.spender,
                    min_return: None,
                    origin: swap_request.dest_address,
                    slippage: None,
                };

                match swap_request.slippage {
                    Slippage::Percent(slippage) => {
                        if slippage > 50.0 {
                            request.slippage = Some(50.0);
                        } else {
                            request.slippage = Some(slippage);
                        }
                    }
                    Slippage::AmountLimit {
                        amount_limit,
                        fallback_slippage: _,
                    } => {
                        request.min_return = Some(amount_limit.to_string());
                    }
                    Slippage::MaxSlippage => {
                        request.slippage = Some(50.0); // 50%
                    }
                }

                let swap_response = one_inch_swap(&client, &api_key, request).await?;

                let amount_out = decimal_string_to_u128(&swap_response.dst_amount, 0)?;

                let amount_limit =
                    get_limit_amount(swap_request.trade_type, amount_out, swap_request.slippage)?;

                Ok(EvmSwapResponse {
                    amount_quote: amount_out,
                    amount_limit,
                    tx_to: swap_response.tx.to.clone(),
                    tx_data: swap_response.tx.data,
                    tx_value: decimal_string_to_u128(&swap_response.tx.value, 0)?,
                    approve_address: Some(swap_response.tx.to),
                    require_transfer: false,
                })
            }
            TradeType::ExactOut => {
                let (response, _) = quote_exact_out_with_exact_in(
                    swap_request,
                    move |swap_request: GenericSwapRequest| {
                        let client = client.clone();
                        let api_key = api_key.clone();
                        async move {
                            Box::pin(prepare_swap_one_inch(client, &api_key, swap_request)).await
                        }
                    },
                )
                .await?;

                Ok(response)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use intents_models::constants::chains::ChainId;

    use super::*;

    #[tokio::test]
    async fn test_one_inch_get_quote() {
        dotenv::dotenv().ok();

        let one_inch_api_key =
            std::env::var("ONE_INCH_API_KEY").expect("ONE_INCH_API_KEY must be set");

        let client = Client::new();
        let request = OneInchGetQuoteRequest {
            chain: ChainId::Base as u32,
            src: "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string(),
            dst: "0x0555E30da8f98308EdB960aa94C0Db47230d2B9c".to_string(),
            amount: "1000000".to_string(), // 1 USDC
        };

        let result = one_inch_get_quote(client, &one_inch_api_key, request).await;
        println!("Result: {:#?}", result);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_one_inch_swap() {
        dotenv::dotenv().ok();

        let one_inch_api_key =
            std::env::var("ONE_INCH_API_KEY").expect("ONE_INCH_API_KEY must be set");

        let client = Client::new();
        let request = OneInchSwapRequest {
            chain: ChainId::Base as u32,
            src: "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string(),
            dst: "0x0555E30da8f98308EdB960aa94C0Db47230d2B9c".to_string(),
            amount: "1000000".to_string(), // 1 USDC
            from: "0x9ecdc9af2a8254dde8bbce8778efae695044cc9f".to_string(),
            min_return: None,
            origin: "0x9ecdc9af2a8254dde8bbce8778efae695044cc9f".to_string(),
            slippage: Some(0.5), // 0.5%
        };

        let result = one_inch_swap(client, &one_inch_api_key, request).await;
        println!("Result: {:#?}", result);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_one_inch_get_approve_address() {
        dotenv::dotenv().ok();

        let one_inch_api_key =
            std::env::var("ONE_INCH_API_KEY").expect("ONE_INCH_API_KEY must be set");
        let client = Client::new();

        let result =
            one_inch_get_approve_address(client, &one_inch_api_key, ChainId::Base as u32).await;
        println!("Result: {:#?}", result);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_one_inch_swap_exact_in() {
        dotenv::dotenv().ok();

        let one_inch_api_key =
            std::env::var("ONE_INCH_API_KEY").expect("ONE_INCH_API_KEY must be set");

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

        let client = Client::new();

        let generic_estimate_request = GenericEstimateRequest::from(request.clone());
        let result =
            estimate_swap_one_inch(client.clone(), &one_inch_api_key, generic_estimate_request)
                .await;
        assert!(
            result.is_ok(),
            "Expected a successful estimate swap response"
        );

        let result = prepare_swap_one_inch(client, &one_inch_api_key, request).await;
        println!("Result: {:#?}", result);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_one_inch_swap_exact_out() {
        dotenv::dotenv().ok();

        let one_inch_api_key =
            std::env::var("ONE_INCH_API_KEY").expect("ONE_INCH_API_KEY must be set");

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

        let client = Client::new();

        let generic_estimate_request = GenericEstimateRequest::from(request.clone());
        let result =
            estimate_swap_one_inch(client.clone(), &one_inch_api_key, generic_estimate_request)
                .await;
        assert!(
            result.is_ok(),
            "Expected a successful estimate swap response"
        );

        let result = prepare_swap_one_inch(client, &one_inch_api_key, request).await;
        println!("Result: {:#?}", result);
        assert!(result.is_ok());
    }
}
