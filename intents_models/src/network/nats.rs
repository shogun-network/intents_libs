use crate::error::{Error, ModelResult};
use async_nats::{Client, ConnectOptions};
use error_stack::ResultExt;
use futures::stream::StreamExt;
use serde::{Serialize, de::DeserializeOwned};
use std::future::Future;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::time::Duration;

use crate::network::validate_and_parse_json;

#[derive(Debug, Clone)]
pub struct NatsManager<MsgOut, MsgIn> {
    client: Client,
    max_request_body_size: usize,
    max_json_depth: usize,
    chunk_processing_interval: usize,
    _marker: PhantomData<(MsgOut, MsgIn)>,
}

const REQUEST_TIMEOUT_SECONDS: u64 = 30;

impl<MsgOut, MsgIn> NatsManager<MsgOut, MsgIn>
where
    MsgOut: Serialize,
    MsgIn: DeserializeOwned,
{
    pub async fn new(
        nats_url: String,
        user: String,
        passwd: String,
        tls_cert_path: Option<String>,
        max_request_body_size: usize,
        max_json_depth: usize,
        chunk_processing_interval: usize,
    ) -> ModelResult<Self> {
        let mut conn_opt = ConnectOptions::new()
            .user_and_password(user, passwd)
            .request_timeout(Some(Duration::from_secs(REQUEST_TIMEOUT_SECONDS)));

        if let Some(tls_cert_path) = tls_cert_path {
            rustls::crypto::ring::default_provider()
                .install_default()
                .expect("Failed to install rustls crypto provider");

            let cert_path: PathBuf = tls_cert_path.into();
            conn_opt = conn_opt.add_root_certificates(cert_path).require_tls(true)
        }
        let client = async_nats::connect_with_options(nats_url, conn_opt)
            .await
            .change_context(Error::NatsError(
                "Failed to connect to NATS server".to_string(),
            ))?;

        Ok(NatsManager {
            client,
            max_request_body_size,
            max_json_depth,
            chunk_processing_interval,
            _marker: PhantomData,
        })
    }

    pub async fn request(&self, subject: &'static str, msg: MsgOut) -> ModelResult<MsgIn> {
        let data = serde_json::to_vec(&msg).change_context(Error::SerdeSerialize(
            "Failed to serialize nats msg".to_string(),
        ))?;

        let response_msg = self
            .client
            .request(subject, data.into())
            .await
            .change_context(Error::NatsError("Failed to send nats request".to_string()))?;
        serde_json::from_slice(&response_msg.payload).change_context(Error::SerdeDeserialize(
            "Failed to deserialize nats response".to_string(),
        ))
    }

    pub async fn subscribe_and_process<F, Fut>(
        &self,
        subject: &'static str,
        processor: F,
    ) -> ModelResult<()>
    where
        F: Fn(MsgIn) -> Fut,
        Fut: Future<Output = MsgOut>,
    {
        let mut subscriber =
            self.client
                .subscribe(subject)
                .await
                .change_context(Error::NatsError(
                    "Failed to subscribe to nats subject".to_string(),
                ))?;

        while let Some(message) = subscriber.next().await {
            let client_msg: MsgIn = match validate_and_parse_json(
                &message.payload,
                self.max_request_body_size,
                self.max_json_depth,
                self.chunk_processing_interval,
            ) {
                Ok(msg) => msg,
                Err(e) => {
                    tracing::error!("Failed to parse message: {}", e);
                    continue; // Skip this message if parsing fails
                }
            };

            let response = processor(client_msg).await;
            // Reply
            if let Some(reply) = message.reply {
                let response_bytes = serde_json::to_vec(&response).change_context(
                    Error::SerdeSerialize("Failed to serialize nats response".to_string()),
                )?;
                self.client
                    .publish(reply, response_bytes.into())
                    .await
                    .change_context(Error::NatsError(
                        "Failed to publish nats response".to_string(),
                    ))?;
            } else {
                tracing::error!("No reply subject found for message. Ignoring");
            }
        }
        Ok(())
    }
}
