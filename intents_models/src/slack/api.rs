use crate::{
    error::{Error, ModelResult},
    network::http::{HttpMethod, handle_reqwest_response, value_to_sorted_querystring},
};
use error_stack::{ResultExt, report};
use once_cell::sync::Lazy;
use reqwest::{Client, header::CONTENT_TYPE};
use serde_json::Value;
use std::sync::Arc;

use super::{
    constants::SLACK_API_URL,
    responses::{PostMessageResponse, SlackResponse},
};

pub static HTTP_CLIENT: Lazy<Arc<Client>> = Lazy::new(|| Arc::new(Client::new()));

/// Sends a request to the Slack API with the provided parameters.
///
/// # Arguments
///
/// * `token` - The Slack authentication token
/// * `uri_path` - The API endpoint path (e.g., "/chat.postMessage")
/// * `query` - Optional query parameters as a JSON Value
/// * `body` - Optional request body as a JSON Value
/// * `method` - HTTP method to use (GET, POST, etc.)
///
/// # Returns
///
/// A Result containing a `SlackResponse` or an error
///
/// # Errors
///
/// Will return an error if:
/// - Query parsing fails
/// - Request sending fails
/// - Invalid HTTP method is provided
/// - Slack API returns an error response
async fn send_slack_api_request(
    token: &str,
    uri_path: &str,
    query: Option<Value>,
    body: Option<Value>,
    method: HttpMethod,
) -> ModelResult<SlackResponse> {
    let url = format!("{SLACK_API_URL}{uri_path}");
    let client = HTTP_CLIENT.clone();
    let url_and_query = match query {
        Some(q) => {
            let query_string = value_to_sorted_querystring(&q)
                .change_context(Error::ParseError)
                .attach_printable("Failed to parse query string".to_string())?;
            format!("{url}?{query_string}")
        }
        None => url,
    };
    let request = match method {
        HttpMethod::GET => client.get(url_and_query),
        HttpMethod::POST => match body {
            Some(body) => client
                .post(url_and_query)
                .header(CONTENT_TYPE, "application/json")
                .json(&body),
            None => client.post(url_and_query),
        },
        _ => {
            return Err(report!(Error::Unknown)
                .attach_printable(format!("Invalid http method: {method:?}")));
        }
    };
    let response = request
        .bearer_auth(token)
        // .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .change_context(Error::ReqwestError("Failed to send request".to_string()))?;

    match handle_reqwest_response(response).await {
        Ok(val) => Ok(val),
        Err(e) => {
            Err(e.change_context(Error::ReqwestError("Failed to handle response".to_string())))
        }
    }
}

/// Processes a Slack API response and validates it for errors.
///
/// This function examines the Slack API response and handles different response types.
/// If the response indicates an error from the Slack API or is of an unrecognized format,
/// it converts these to appropriate application errors. Otherwise, it passes through
/// successful responses.
///
/// # Arguments
///
/// * `response` - The Slack API response to process
///
/// # Returns
///
/// * `Ok(SlackResponse)` - If the response is valid and not an error
/// * `Err` - If the response contains a Slack API error or is of an unknown format
///
fn handle_slack_response(response: SlackResponse) -> ModelResult<SlackResponse> {
    match response {
        SlackResponse::Error(slack_error) => {
            tracing::error!("Error in slack api response: {}", slack_error.error);
            Err(report!(Error::ReqwestError(format!(
                "Slack API error: {}",
                slack_error.error
            ))))
        }
        SlackResponse::UnknownResponse(value) => {
            tracing::error!("Unknown response: {value:?}");
            Err(report!(Error::Unknown)
                .attach_printable(format!("Unknown response from Slack API: {value:?}")))
        }
        _ => Ok(response),
    }
}

/// Sends a message to a Slack channel.
///
/// This function composes and sends a message to the specified Slack channel using
/// the Slack Chat API. It allows for optional customization of the sender's username
/// that will be displayed in the Slack message.
///
/// # Arguments
///
/// * `token` - The Slack authentication token for API access
/// * `channel` - The target channel ID or name (e.g., "#general" or "C012AB3CD")
/// * `text` - The message content to send
/// * `username` - Optional username to display as the message sender
///
/// # Returns
///
/// * `ModelResult<PostMessageResponse>` - A result containing the Slack API response with
///   message details on success
///
/// # Errors
///
/// Will return an error if:
/// - The Slack API request fails to send
/// - The authentication token is invalid or lacks necessary permissions
/// - The channel doesn't exist or the bot isn't a member
/// - The response from Slack contains an error
/// - The response is not of the expected type
///
pub async fn post_msg(token: &str, channel: &str, text: &str) -> ModelResult<PostMessageResponse> {
    let uri_path = "/chat.postMessage";
    let body = serde_json::json!({
        "channel": channel,
        "text": text,
    });
    let response: SlackResponse =
        send_slack_api_request(token, uri_path, None, Some(body), HttpMethod::POST).await?;
    match handle_slack_response(response)? {
        SlackResponse::PostMessage(post_message_response) => Ok(post_message_response),
        response => Err(report!(Error::Unknown)
            .attach_printable(format!("Unexpected response from Slack API: {response:?}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slack::responses::SlackError;

    #[tokio::test]
    async fn test_send_msg() {
        dotenv::dotenv().ok();
        let token = match std::env::var("SLACK_BOT_TOKEN") {
            Ok(token) => token,
            Err(_) => return,
        };

        let channel = match std::env::var("SLACK_REBALANCE_CHANNEL") {
            Ok(channel) => channel,
            Err(_) => return,
        };
        let text = "Testing message";

        let result = post_msg(&token, &channel, text).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_slack_response() {
        let response = SlackResponse::PostMessage(PostMessageResponse {
            ok: true,
            channel: "mock".to_string(),
            ts: "mock".to_string(),
            message: Default::default(),
        });
        assert!(handle_slack_response(response).is_ok());
        let response = SlackResponse::Error(SlackError {
            ok: false,
            error: "mock".to_string(),
        });
        assert!(handle_slack_response(response).is_err());
        let response = SlackResponse::UnknownResponse(Default::default());
        assert!(handle_slack_response(response).is_err());
    }
}
