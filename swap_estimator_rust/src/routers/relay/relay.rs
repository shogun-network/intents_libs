use crate::error::{Error, EstimatorResult};
use crate::routers::constants::BASE_RELAY_API_URL;
use crate::routers::estimate::TradeType;
use crate::routers::relay::requests::RelayQuoteRequest;
use crate::routers::relay::responses::{RelayQuoteResponse, RelayResponse};
use error_stack::{ResultExt, report};
use intents_models::network::client_rate_limit::Client;
use intents_models::network::http::{
    HttpMethod, handle_reqwest_response, value_to_sorted_querystring,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::fmt::Debug;

pub async fn send_relay_request<TxData>(
    client: &Client,
    uri_path: &str,
    query: Option<Value>,
    body: Option<Value>,
    method: HttpMethod,
) -> EstimatorResult<RelayResponse<TxData>>
where
    TxData: DeserializeOwned + Debug,
{
    let url = match query {
        Some(query) => {
            let query = value_to_sorted_querystring(&query).change_context(Error::ModelsError)?;
            format!("{BASE_RELAY_API_URL}{uri_path}?{query}")
        }
        None => format!("{BASE_RELAY_API_URL}{uri_path}"),
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
        request
            .build()
            .change_context(Error::ReqwestError)
            .attach_printable("Error building Relay request")?
    };

    let response = client
        .execute(request)
        .await
        .change_context(Error::ReqwestError)
        .attach_printable("Error in Relay request")?;

    let response = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;

    Ok(response)
}

pub fn handle_relay_response<TxData>(
    response: RelayResponse<TxData>,
) -> EstimatorResult<RelayResponse<TxData>> {
    match response {
        RelayResponse::UnknownResponse(val) => {
            tracing::error!(
                "Unknown response from Relay: {}",
                serde_json::to_string_pretty(&val).unwrap_or_else(|_| format!("{:?}", val))
            );
            Err(report!(Error::ResponseError).attach_printable("Unknown response from Relay"))
        }
        _ => Ok(response),
    }
}

pub async fn quote_relay_generic<TxData>(
    client: &Client,
    request: RelayQuoteRequest,
) -> EstimatorResult<RelayQuoteResponse<TxData>>
where
    TxData: DeserializeOwned + Debug,
{
    // Convert the request struct to a serde_json::Value to modify attribute names as specified by serde renames
    let body = serde_json::to_value(request).expect("Can't fail");

    let response = handle_relay_response(
        send_relay_request(client, "quote", None, Some(body), HttpMethod::POST).await?,
    )?;
    if let RelayResponse::Quote(quote_response) = response {
        Ok(quote_response)
    } else {
        tracing::error!(
            "Unexpected response from Relay /quote request, response: {:?}",
            response
        );
        Err(report!(Error::ResponseError).attach_printable("Unexpected response from Relay"))
    }
}

pub fn get_amounts_from_quote<TxData>(
    quote_response: &RelayQuoteResponse<TxData>,
    trade_type: TradeType,
) -> EstimatorResult<(u128, u128)> {
    let (amount_quote, amount_limit) = match trade_type {
        TradeType::ExactIn => (
            quote_response.details.currency_out.amount.clone(),
            quote_response.details.currency_out.minimum_amount.clone(),
        ),
        TradeType::ExactOut => (
            quote_response.details.currency_in.amount.clone(),
            quote_response.details.currency_in.minimum_amount.clone(),
        ),
    };

    let amount_quote = amount_quote
        .parse::<u128>()
        .change_context(Error::AggregatorError(
            "Error deserializing Relay quote output amount".to_string(),
        ))?;
    let amount_limit = amount_limit
        .parse::<u128>()
        .change_context(Error::AggregatorError(
            "Error deserializing Relay limit output amount".to_string(),
        ))?;

    Ok((amount_quote, amount_limit))
}
