use error_stack::{ResultExt as _, report};
use intents_models::network::http::{handle_reqwest_response, value_to_sorted_querystring};
use reqwest::Client;
use serde_json::{Value, json};

use crate::{
    error::{Error, EstimatorResult},
    routers::{
        RouterType, Slippage,
        estimate::{GenericEstimateRequest, GenericEstimateResponse, TradeType},
        swap::{EvmSwapResponse, GenericSwapRequest},
        zero_x::{
            BASE_ZERO_X_API_URL,
            requests::{ZeroXGetPriceRequest, ZeroXGetQuoteRequest},
        },
    },
    utils::{limit_amount::get_limit_amount, number_conversion::decimal_string_to_u128},
};

pub async fn zero_x_get_price(
    client: &Client,
    api_key: &str,
    request: ZeroXGetPriceRequest,
) -> EstimatorResult<()> {
    let query = json!({
        "chainId": request.chain_id,
        "buyToken": request.buy_token,
        "sellToken": request.sell_token,
        "sellAmount": request.sell_amount,
        "slippageBps": request.slippage_bps,
    });

    let query_string = value_to_sorted_querystring(&query).change_context(Error::ParseError)?;
    let url = format!("{BASE_ZERO_X_API_URL}/allowance-holder/price?{query_string}",);

    let response = client
        .get(&url)
        .header("0x-api-key", api_key)
        .header("0x-version", "v2")
        .send()
        .await
        .change_context(Error::ReqwestError)
        .attach_printable("Error in 1inch request")?;

    let get_price_response: Value = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;

    println!("ZeroX Get Price Response: {:?}", get_price_response);

    Ok(())
}

pub async fn zero_x_get_quote(
    client: &Client,
    api_key: &str,
    request: ZeroXGetQuoteRequest,
) -> EstimatorResult<()> {
    let mut query = json!({
        "chainId": request.chain_id,
        "buyToken": request.buy_token,
        "sellToken": request.sell_token,
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

    let response = client
        .get(&url)
        .header("0x-api-key", api_key)
        .header("0x-version", "v2")
        .send()
        .await
        .change_context(Error::ReqwestError)
        .attach_printable("Error in 1inch request")?;

    let get_quote_response: Value = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;

    println!("ZeroX Get Quote Response: {:?}", get_quote_response);

    Ok(())
}

#[cfg(test)]
mod tests {
    use intents_models::constants::chains::ChainId;

    use super::*;

    #[tokio::test]
    async fn test_zero_x_get_price() {
        dotenv::dotenv().ok();

        let zero_x_api_key = std::env::var("ZERO_X_API_KEY").expect("ZERO_X_API_KEY must be set");
        let client = Client::new();
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
        let client = Client::new();
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
}
