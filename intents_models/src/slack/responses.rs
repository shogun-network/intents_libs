use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SlackResponse {
    #[serde(untagged)]
    PostMessage(PostMessageResponse),
    #[serde(untagged)]
    Error(SlackError),
    #[serde(untagged)]
    UnknownResponse(Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostMessageResponse {
    pub ok: bool,
    pub channel: String,
    pub ts: String,
    pub message: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackError {
    pub ok: bool,
    pub error: String,
}
