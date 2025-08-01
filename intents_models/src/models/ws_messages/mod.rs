pub mod api_response;
pub mod auctioneer_message;
pub mod helpers;
pub mod solver_message;

use crate::{
    error::{Error, ModelResult},
    models::ws_messages::{api_response::ApiResponse, solver_message::WsSolverMessage},
};
use auctioneer_message::WsAuctioneerMessage;
use error_stack::{ResultExt, report};
use serde_json::{from_slice, to_vec};

pub fn handle_ws_auctioneer_request_msg(bytes: &[u8]) -> ModelResult<WsAuctioneerMessage> {
    match from_slice::<ApiResponse>(bytes) {
        Ok(msg) => msg.try_into(),
        Err(err) => Err(report!(Error::SerdeDeserialize(format!(
            "Failed to deserialize into WsAuctioneerMessage: {err}"
        )))),
    }
}
pub fn serialize_solver_response_message(msg: WsSolverMessage) -> ModelResult<Vec<u8>> {
    to_vec(&msg)
        .change_context(Error::SerdeSerialize(
            "Failed to serialize WsSolverMessage:".to_string(),
        ))
        .attach_printable_lazy(|| format!("Failed to serialize message: {msg:?}"))
}

pub fn handle_ws_solver_request_msg(bytes: &[u8]) -> ModelResult<WsSolverMessage> {
    match from_slice::<WsSolverMessage>(bytes) {
        Ok(msg) => Ok(msg),
        Err(err) => Err(report!(Error::SerdeDeserialize(
            "Failed to deserialize into WsSolverMessage".to_string()
        ))
        .attach_printable(format!("Failed to deserialize message: {err}"))),
    }
}

pub fn serialize_auctioneer_response_message(msg: WsAuctioneerMessage) -> ModelResult<Vec<u8>> {
    let api_response = ApiResponse::from(msg.clone());
    to_vec(&api_response).change_context(Error::SerdeSerialize(
        "Failed to serialize WsAuctioneerMessage".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use crate::models::ws_messages::auctioneer_message::{
        RegisterResponseData, WsAuctioneerMessageInner,
    };

    use super::*;

    #[test]
    fn test_handle_ws_request_msg_success() {
        let api_response = ApiResponse {
            success: true,
            code: 200,
            data: Some(
                serde_json::to_value(RegisterResponseData {
                    solver_id: "solver_id_mock".to_string(),
                    status: "status_mock".to_string(),
                    pending_auction_results: vec![],
                    unfinished_orders: vec![],
                })
                .unwrap(),
            ),
            error: None,
            extra_error_data: None,
        };

        let bytes = to_vec(&api_response).unwrap();

        let result = handle_ws_auctioneer_request_msg(&bytes);
        assert!(
            result.is_ok(),
            "Error handling request message: {:?}",
            result.err()
        );

        let msg = result.unwrap();
        assert!(matches!(
            *msg,
            WsAuctioneerMessageInner::RegisterResponse(RegisterResponseData { .. })
        ));
    }

    #[test]
    fn test_handle_ws_request_msg_failure() {
        let bytes = b"{ not valid json }";

        let result = handle_ws_auctioneer_request_msg(bytes);
        assert!(result.is_err());
    }
}
