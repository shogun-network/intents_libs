use crate::error::Error;
use crate::routers::raydium::requests::RaydiumCreateTransaction;
use crate::routers::raydium::responses::{
    PriorityFeeResponse, RaydiumResponse, RaydiumResponseData, SwapResponseData, Transaction,
};
use crate::routers::raydium::{PRIORITY_FEE, SWAP_API_URL};
use crate::{
    error::EstimatorResult,
    routers::{estimate::TradeType, raydium::requests::RaydiumGetQuote},
};
use error_stack::{ResultExt, report};
use intents_models::network::http::{handle_reqwest_response, value_to_sorted_querystring};
use lazy_static::lazy_static;
use reqwest::Client;
use serde_json::Value;
use std::sync::Arc;

lazy_static! {
    static ref HTTP_CLIENT: Arc<Client> = Arc::new(Client::new());
}

pub async fn raydium_get_priority_fee() -> EstimatorResult<PriorityFeeResponse> {
    let response = HTTP_CLIENT
        .get(PRIORITY_FEE)
        .send()
        .await
        .change_context(Error::ReqwestError)
        .attach_printable("Error sending request to Raydium API for priority fee")?;

    let raydium_response = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;

    Ok(raydium_response)
}

pub async fn raydium_get_price_route(
    request: RaydiumGetQuote,
    trade_type: TradeType,
) -> EstimatorResult<RaydiumResponse> {
    let swap_type_uri = match trade_type {
        TradeType::ExactIn => "swap-base-in",
        TradeType::ExactOut => "swap-base-out",
    };
    let query = value_to_sorted_querystring(&serde_json::to_value(request).change_context(
        Error::SerdeSerialize(format!(
            "Error serializing request for raydium get price route"
        )),
    )?)
    .change_context(Error::ModelsError)
    .attach_printable("Error creating query string")?;
    let url = format!("{}/compute/{}?{}", SWAP_API_URL, swap_type_uri, query);

    let response = HTTP_CLIENT
        .get(&url)
        .send()
        .await
        .change_context(Error::ReqwestError)
        .attach_printable("Error sending request to Raydium API")?;

    let raydium_response: Value = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;

    let raydium_response = serde_json::from_value(raydium_response).change_context(
        Error::SerdeDeserialize("Failed to deserialize JSON".to_string()),
    )?;

    Ok(raydium_response)
    // let raydium_response = handle_raydium_response(raydium_response)?;

    // let RaydiumResponseData::GetPriceRoute(get_price_route_response) = raydium_response else {
    //     return Err(report!(Error::ResponseError)
    //         .attach_printable("Unexpected response type from Raydium get price route"));
    // };

    // Ok(get_price_route_response)
}

pub fn raydium_get_price_route_from_swap_response(
    raydium_response: RaydiumResponse,
) -> EstimatorResult<SwapResponseData> {
    let raydium_response = handle_raydium_response(raydium_response)?;

    let RaydiumResponseData::GetPriceRoute(get_price_route_response) = raydium_response else {
        return Err(report!(Error::ResponseError)
            .attach_printable("Unexpected response type from Raydium get price route"));
    };

    Ok(get_price_route_response)
}

pub async fn raydium_create_transaction(
    request: RaydiumCreateTransaction,
    trade_type: TradeType,
) -> EstimatorResult<Vec<Transaction>> {
    let swap_type_uri = match trade_type {
        TradeType::ExactIn => "swap-base-in",
        TradeType::ExactOut => "swap-base-out",
    };
    let url = format!("{}/transaction/{}", SWAP_API_URL, swap_type_uri);

    let response = HTTP_CLIENT
        .post(&url)
        .json(&request)
        .send()
        .await
        .change_context(Error::ReqwestError)
        .attach_printable("Error sending request to Raydium API")?;

    let raydium_response = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;

    // Ok(raydium_response)

    let raydium_response = handle_raydium_response(raydium_response)?;

    let RaydiumResponseData::SwapTransactions(transaction_response) = raydium_response else {
        return Err(report!(Error::ResponseError)
            .attach_printable("Unexpected response type from Raydium create transaction"));
    };

    Ok(transaction_response)
}

fn handle_raydium_response(response: RaydiumResponse) -> EstimatorResult<RaydiumResponseData> {
    match response.success {
        true => {
            if let Some(data) = response.data {
                Ok(data)
            } else {
                Err(report!(Error::ResponseError)
                    .attach_printable("Missing data field in successful Raydium response"))
            }
        }
        false => {
            if let Some(msg) = response.msg {
                Err(report!(Error::AggregatorError(format!(
                    "Raydium API error: {msg}"
                ))))
            } else {
                Err(report!(Error::ResponseError)
                    .attach_printable("Raydium response indicates failure but no message provided"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_raydium_get_priority_fee() {
        let result = raydium_get_priority_fee().await;
        println!("{:?}", result);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_raydium_get_price_route() {
        let request = RaydiumGetQuote {
            input_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(), // USDC
            output_mint: "w6iohhdC6zbq2DP1uwtmvXPJibbFroDnni1A222bonk".to_string(), // BONK
            amount: 1000000,
            slippage_bps: 200,
            tx_version: "V0".to_string(),
        };
        let trade_type = TradeType::ExactIn;

        let result = raydium_get_price_route(request, trade_type).await;

        println!("{:?}", result);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_raydium_create_transaction() {
        // Get quote first
        let request = RaydiumGetQuote {
            input_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(), // USDC
            output_mint: "w6iohhdC6zbq2DP1uwtmvXPJibbFroDnni1A222bonk".to_string(), // BONK
            amount: 1000000,
            slippage_bps: 200,
            tx_version: "V0".to_string(),
        };
        let trade_type = TradeType::ExactIn;

        let result = raydium_get_price_route(request, trade_type).await;

        println!("{:?}", result);
        assert!(result.is_ok());

        let request = RaydiumCreateTransaction {
            swap_response: result.unwrap(),
            compute_unit_price_micro_lamports: "10000".to_string(),
            wrap_sol: false,
            unwrap_sol: false,
            tx_version: "V0".to_string(),
            input_account: "5JzgVH4JD97RT6rG6tRyvh5yaqYthgmKQvzwMKhSvV3E".to_string(),
            output_account: "2BVTs72czvwooFQxvRXoCidh1d6eEZwvVzTtLyUxNbQc".to_string(),
            wallet: "7kDXEH3xPS5TvScR1czWvSCJMaeHHB9693mWTrdTRQVB".to_string(),
        };

        let result = raydium_create_transaction(request, trade_type).await;

        println!("{:?}", result);
        assert!(result.is_ok());
    }
}
