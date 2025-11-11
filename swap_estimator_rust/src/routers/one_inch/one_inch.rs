use error_stack::{ResultExt as _, report};
use intents_models::network::http::{handle_reqwest_response, value_to_sorted_querystring};
use reqwest::Client;
use serde_json::json;

use crate::{
    error::{Error, EstimatorResult},
    routers::one_inch::{
        BASE_1INCH_API_URL,
        requests::{OneInchGetQuoteRequest, OneInchSwapRequest},
        responses::{OneInchGetQuoteResponse, OneInchSwapResponse},
    },
    utils::number_conversion::decimal_string_to_u128,
};

pub async fn one_inch_get_quote(
    client: &Client,
    api_key: String,
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
    client: &Client,
    api_key: String,
    request: OneInchSwapRequest,
) -> EstimatorResult<OneInchSwapResponse> {
    let mut query = json!({
        "src": request.src,
        "dst": request.dst,
        "amount": request.amount,
        "from": request.from,
        "origin": request.origin,
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

        let result = one_inch_get_quote(&client, one_inch_api_key, request).await;
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
            slippage: Some(1), // 1%
        };

        let result = one_inch_swap(&client, one_inch_api_key, request).await;
        println!("Result: {:#?}", result);
        assert!(result.is_ok());
    }
}
