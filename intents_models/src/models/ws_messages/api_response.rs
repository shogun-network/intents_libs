use crate::{
    error::Error,
    models::ws_messages::auctioneer_message::{WsAuctioneerMessage, WsAuctioneerMessageInner},
};
use error_stack::{Report, report};
use serde::{Deserialize, Serialize};
use serde_json::{Value, from_value};
use std::ops::Deref;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiResponse {
    pub success: bool,
    pub code: u16,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>, // TODO: Maybe use String as data-type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_error_data: Option<Vec<Value>>, // Option is needed for serializing purposes
}

impl ApiResponse {
    pub fn success<T: Into<Value>>(data: T) -> Self {
        Self {
            success: true,
            data: Some(data.into()),
            error: None,
            extra_error_data: None,
            code: 200,
        }
    }

    pub fn internal_server_error<T: Into<Value>>(error: T) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error.into()),
            extra_error_data: None,
            code: 500,
        }
    }

    pub fn unauthorized<T: Into<Value>>(error: T) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error.into()),
            extra_error_data: None,
            code: 401,
        }
    }

    pub fn bad_request<T: Into<Value>>(error: T) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error.into()),
            extra_error_data: None,
            code: 400,
        }
    }

    pub fn payload_too_large<T: Into<Value>>(error: T) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error.into()),
            extra_error_data: None,
            code: 413,
        }
    }

    pub fn extra_err_data<T: Into<Value>>(mut self, data: T) -> Self {
        if let Some(mut err_data) = self.extra_error_data.clone() {
            let value = data.into();
            err_data.push(value);
            self.extra_error_data = Some(err_data);
        } else {
            self.extra_error_data = Some(vec![data.into()]);
        }

        self
    }
}

impl TryFrom<ApiResponse> for WsAuctioneerMessage {
    type Error = Report<Error>;

    fn try_from(api_response: ApiResponse) -> Result<Self, Self::Error> {
        if !api_response.success {
            return Ok(WsAuctioneerMessage::error(api_response));
        }

        match from_value::<WsAuctioneerMessageInner>(api_response.data.unwrap_or(Value::Null)) {
            Ok(inner) => Ok(WsAuctioneerMessage::new(inner)),
            Err(error) => {
                tracing::error!("Failed to deserialize ApiResponse data: {}", error);
                Err(report!(Error::SerdeDeserialize(format!(
                    "Failed to deserialize ApiResponse data: {error}"
                ))))
            }
        }
    }
}

impl From<WsAuctioneerMessage> for ApiResponse {
    fn from(ws_auctioneer_message: WsAuctioneerMessage) -> Self {
        match ws_auctioneer_message.deref() {
            WsAuctioneerMessageInner::RegisterResponse(register_response_data) => {
                match serde_json::to_value(register_response_data) {
                    Ok(value) => ApiResponse::success(value),
                    Err(err) => {
                        tracing::error!("Failed to serialize register response data: {}", err);
                        ApiResponse::bad_request("Invalid register response data".to_string())
                    }
                }
            }
            WsAuctioneerMessageInner::AuctionRequest(auction_request_data) => {
                match serde_json::to_value(auction_request_data) {
                    Ok(value) => ApiResponse::success(value),
                    Err(err) => {
                        tracing::error!("Failed to serialize auction request data: {}", err);
                        ApiResponse::bad_request("Invalid auction request data".to_string())
                    }
                }
            }
            WsAuctioneerMessageInner::AuctionResult(auction_result_data) => {
                match serde_json::to_value(auction_result_data) {
                    Ok(value) => ApiResponse::success(value),
                    Err(err) => {
                        tracing::error!("Failed to serialize auction result data: {}", err);
                        ApiResponse::bad_request("Invalid auction result data".to_string())
                    }
                }
            }
            WsAuctioneerMessageInner::AuctionEnd(auction_end_data) => {
                match serde_json::to_value(auction_end_data) {
                    Ok(value) => ApiResponse::success(value),
                    Err(err) => {
                        tracing::error!("Failed to serialize auction end data: {}", err);
                        ApiResponse::bad_request("Invalid auction end data".to_string())
                    }
                }
            }
            WsAuctioneerMessageInner::ErrorMessage(api_response) => api_response.clone(),
            WsAuctioneerMessageInner::Unknown(unknown_value) => {
                tracing::warn!("Received unknown message: {:?}", unknown_value);
                ApiResponse::bad_request("Unknown message format".to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::models::ws_messages::auctioneer_message::RegisterResponseData;

    use super::*;

    #[test]
    fn test_from_api_response_register_response() {
        let api_response = ApiResponse {
            success: true,
            code: 200,
            data: serde_json::to_value(RegisterResponseData {
                solver_id: "solver_id_mock".to_string(),
                status: "status_mock".to_string(),
                pending_auction_results: vec![],
                unfinished_orders: vec![],
            })
            .ok(),
            error: None,
            extra_error_data: None,
        };
        let message: WsAuctioneerMessage = api_response
            .try_into()
            .expect("Failed to convert ApiResponse to WsAuctioneerMessage");
        assert!(matches!(
            *message,
            WsAuctioneerMessageInner::RegisterResponse(_)
        ));
    }
}
