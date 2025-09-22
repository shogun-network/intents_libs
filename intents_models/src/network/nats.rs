use crate::error::{Error, ModelResult};
use async_nats::{Client, ConnectOptions};
use error_stack::ResultExt;
use futures::stream::StreamExt;
use serde::{Serialize, de::DeserializeOwned};
use std::future::Future;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::network::validate_and_parse_json;

#[derive(Debug, Clone)]
pub struct NatsManager<MsgOut, MsgIn> {
    client: Client,
    max_request_body_size: usize,
    max_json_depth: usize,
    chunk_processing_interval: usize,
    max_concurrency: usize,
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
        max_concurrency: usize,
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
            max_concurrency,
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
        self,
        subject: &'static str,
        processor: F,
    ) -> ModelResult<()>
    where
        F: Fn(MsgIn) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = MsgOut> + Send + 'static,
    {
        let subscriber = self
            .client
            .subscribe(subject)
            .await
            .change_context(Error::NatsError(
                "Failed to subscribe to nats subject".to_string(),
            ))?;

        // Making it capable of processing multiple messages concurrently
        let client = self.client.clone();
        let max_request_body_size = self.max_request_body_size;
        let max_json_depth = self.max_json_depth;
        let chunk_processing_interval = self.chunk_processing_interval;
        let max_concurrency = self.max_concurrency;

        let processor = Arc::new(processor);

        subscriber
            .for_each_concurrent(max_concurrency, |message| {
                let client = client.clone();
                let processor = Arc::clone(&processor);
                async move {
                    // Parse with limits (prevents large/complex JSON abuse)
                    let client_msg: MsgIn = match validate_and_parse_json(
                        &message.payload,
                        max_request_body_size,
                        max_json_depth,
                        chunk_processing_interval,
                    ) {
                        Ok(msg) => msg,
                        Err(e) => {
                            tracing::error!("Failed to parse message: {}", e);
                            return;
                        }
                    };

                    // Process request
                    let response = processor(client_msg).await;

                    // Reply to request
                    if let Some(reply) = message.reply {
                        match serde_json::to_vec(&response) {
                            Ok(bytes) => {
                                if let Err(e) = client.publish(reply, bytes.into()).await {
                                    tracing::error!("Failed to publish nats response: {:?}", e);
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to serialize nats response: {:?}", e);
                            }
                        }
                    } else {
                        tracing::error!("No reply subject found for message. Ignoring");
                    }
                }
            })
            .await;

        Ok(())
    }
}
