use crate::{
    error::{Error, ModelResult},
    network::{
        client_rate_limit::Client,
        http::{HttpMethod, handle_reqwest_response, value_to_sorted_querystring},
    },
};
use error_stack::{ResultExt, report};
use reqwest::header::CONTENT_TYPE;
use serde_json::Value;

use super::{
    constants::SLACK_API_URL,
    responses::{PostMessageResponse, SlackResponse},
};

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
    client: &Client,
    token: &str,
    uri_path: &str,
    query: Option<Value>,
    body: Option<Value>,
    method: HttpMethod,
) -> ModelResult<SlackResponse> {
    let url = format!("{SLACK_API_URL}{uri_path}");
    let url_and_query = match query {
        Some(q) => {
            let query_string = value_to_sorted_querystring(&q)
                .change_context(Error::ParseError)
                .attach_printable("Failed to parse query string".to_string())?;
            format!("{url}?{query_string}")
        }
        None => url,
    };
    let request = {
        let client = client.inner_client();
        let mut request = match method {
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
        request = request.bearer_auth(token);
        request.build().change_context(Error::ReqwestError(
            "Failed to build Slack request".to_string(),
        ))?
    };

    let response = client
        .execute(request)
        .await
        .change_context(Error::ReqwestError("Failed to send request".to_string()))?;

    match handle_reqwest_response(response).await {
        Ok(val) => Ok(val),
        Err(e) => Err(e.attach_printable("Error handling Slack response")),
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

/// Sends a message to a Slack channel using `chat.postMessage`.
///
/// # Arguments
///
/// * `client` - HTTP client used to execute the request.
/// * `token` - Slack Bot/User OAuth token used for authentication (sent as `Authorization: Bearer ...`).
/// * `channel` - Target channel identifier. Typically a channel ID like `C012AB3CD`
/// * `text` - Message content to send.
///
/// # Returns
///
/// On success, returns a [`PostMessageResponse`](intents_models/src/slack/responses.rs) containing
/// Slack's response payload for the posted message.
///
/// # Errors
///
/// Returns an error if:
/// - The request cannot be built or sent.
/// - Slack returns a non-success HTTP status (including rate limiting via `429 Retry-After`).
/// - The response cannot be deserialized into the expected Slack response type.
/// - Slack returns an application-level error (`ok: false`) in the JSON body.
/// - The response is not the expected variant for this endpoint.
pub async fn post_msg(
    client: &Client,
    token: &str,
    channel: &str,
    text: &str,
) -> ModelResult<PostMessageResponse> {
    let uri_path = "/chat.postMessage";
    let body = serde_json::json!({
        "channel": channel,
        "text": text,
    });
    let response: SlackResponse =
        send_slack_api_request(client, token, uri_path, None, Some(body), HttpMethod::POST).await?;
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

        let client = Client::Unrestricted(reqwest::Client::new());
        let result = post_msg(&client, &token, &channel, text).await;
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
