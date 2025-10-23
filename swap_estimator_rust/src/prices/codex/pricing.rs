use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use error_stack::{ResultExt as _, report};
use futures_util::{SinkExt as _, StreamExt as _};
use intents_models::constants::chains::ChainId;
use serde::Deserialize;
use tokio::{
    sync::{Mutex, Notify, OnceCell, RwLock, watch},
    time,
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, protocol::Message},
};

use crate::{
    error::{Error, EstimatorResult},
    prices::{
        PriceProvider, TokenId, TokenPrice,
        codex::{CODEX_WS_URL, CodexChain},
    },
};

const GRAPHQL_SUBSCRIPTION: &str = r#"
subscription OnPriceUpdated($address: String!, $networkId: Int!) {
  onPriceUpdated(address: $address, networkId: $networkId) {
    address
    priceUsd
    timestamp
    poolAddress
    confidence
  }
}
"#;

#[derive(Debug, Clone)]
pub struct CodexProvider {
    api_key: String,
    client: Arc<OnceCell<Arc<CodexWsClient>>>,
}

impl CodexProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Arc::new(OnceCell::new()),
        }
    }

    async fn client(&self) -> EstimatorResult<Arc<CodexWsClient>> {
        let api_key = self.api_key.clone();
        let reference = self
            .client
            .get_or_try_init(|| async move { CodexWsClient::connect(api_key).await })
            .await?;
        Ok(reference.clone())
    }

    pub async fn subscribe(&self, token: TokenId) -> EstimatorResult<CodexSubscription> {
        let client = self.client().await?;
        client.subscribe(token).await
    }

    pub async fn unsubscribe(&self, token: &TokenId) -> EstimatorResult<()> {
        let client = self.client().await?;
        client.unsubscribe(token).await
    }

    pub async fn latest_price(&self, token: &TokenId) -> EstimatorResult<Option<TokenPrice>> {
        let client = self.client().await?;
        Ok(client.latest_price(token).await)
    }
}

#[async_trait::async_trait]
impl PriceProvider for CodexProvider {
    async fn get_tokens_price(
        &self,
        tokens: HashSet<TokenId>,
    ) -> EstimatorResult<HashMap<TokenId, TokenPrice>> {
        if tokens.is_empty() {
            return Ok(HashMap::new());
        }

        let client = self.client().await?;
        let mut result = HashMap::new();

        for token in tokens.into_iter() {
            let mut subscription = client.subscribe(token.clone()).await?;
            let price = subscription
                .wait_for_price(Duration::from_secs(5))
                .await
                .change_context(Error::ResponseError)
                .attach_printable(format!(
                    "Timed out waiting for Codex price update for {}",
                    token.address
                ))?;

            result.insert(token, price);
            // Drop subscription handle to allow automatic ref-count decrease.
        }

        Ok(result)
    }
}

#[derive(Debug, Clone)]
struct TokenSubscription {
    token: TokenId,
    updates_tx: watch::Sender<Option<TokenPrice>>,
    ref_count: usize,
}

#[derive(Debug)]
struct CodexWsClient {
    sender: tokio::sync::mpsc::UnboundedSender<Message>,
    subscriptions: RwLock<HashMap<String, TokenSubscription>>,
    connected: AtomicBool,
    connected_notify: Notify,
}

impl CodexWsClient {
    async fn connect(api_key: String) -> EstimatorResult<Arc<Self>> {
        let mut request = CODEX_WS_URL
            .into_client_request()
            .change_context(Error::ResponseError)
            .attach_printable("Failed to construct Codex websocket request")?;

        request.headers_mut().insert(
            "Sec-WebSocket-Protocol",
            HeaderValue::from_static("graphql-transport-ws"),
        );
        request.headers_mut().insert(
            "Authorization",
            HeaderValue::from_str(&api_key)
                .change_context(Error::ResponseError)
                .attach_printable("Invalid characters in CODEX_API_KEY")?,
        );

        let (stream, _response) = connect_async(request)
            .await
            .change_context(Error::ResponseError)
            .attach_printable("Failed to connect to Codex websocket")?;

        let (write, mut read) = stream.split();

        let (send_tx, mut send_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
        let write = Arc::new(Mutex::new(write));
        let write_clone = write.clone();

        tokio::spawn(async move {
            while let Some(message) = send_rx.recv().await {
                if let Err(error) = write_clone.lock().await.send(message).await {
                    tracing::error!("Codex websocket send error: {:?}", error);
                    break;
                }
            }
        });

        let client = Arc::new(Self {
            sender: send_tx,
            subscriptions: RwLock::new(HashMap::new()),
            connected: AtomicBool::new(false),
            connected_notify: Notify::new(),
        });

        client.send_message(Message::Text(
            serde_json::json!({
                "type": "connection_init",
                "payload": { "Authorization": api_key }
            })
            .to_string(),
        ))?;

        let client_clone = client.clone();
        tokio::spawn(async move {
            while let Some(message) = read.next().await {
                match message {
                    Ok(Message::Text(text)) => {
                        if let Err(error) = client_clone.handle_text_message(&text).await {
                            tracing::error!("Codex websocket handler error: {:?}", error);
                        }
                    }
                    Ok(Message::Ping(payload)) => {
                        if let Err(error) = client_clone.send_message(Message::Pong(payload)) {
                            tracing::error!("Codex websocket pong send error: {:?}", error);
                        }
                    }
                    Ok(Message::Close(frame)) => {
                        tracing::warn!("Codex websocket closed by server: {:?}", frame);
                        break;
                    }
                    Ok(_) => {}
                    Err(error) => {
                        tracing::error!("Codex websocket receive error: {:?}", error);
                        break;
                    }
                }
            }
        });

        client.wait_for_connection(Duration::from_secs(5)).await?;

        Ok(client)
    }

    async fn handle_text_message(&self, text: &str) -> EstimatorResult<()> {
        let message: GraphqlWsMessage = serde_json::from_str(text).change_context(
            Error::SerdeDeserialize("Failed to parse Codex websocket message".to_string()),
        )?;

        match message.message_type.as_str() {
            "connection_ack" => {
                self.connected.store(true, Ordering::Release);
                self.connected_notify.notify_waiters();
            }
            "ping" => {
                self.send_message(Message::Text(
                    serde_json::json!({"type": "pong"}).to_string(),
                ))?;
            }
            "next" => {
                if let Some(id) = message.id {
                    if let Some(payload) = message.payload {
                        self.handle_next_message(&id, payload).await?;
                    }
                }
            }
            "error" => {
                tracing::error!("Codex websocket error: {}", text);
            }
            "complete" => {
                if let Some(id) = message.id {
                    self.handle_complete(&id).await;
                }
            }
            _ => {}
        }

        Ok(())
    }

    async fn handle_next_message(
        &self,
        id: &str,
        payload: serde_json::Value,
    ) -> EstimatorResult<()> {
        let subscription = {
            let subscriptions = self.subscriptions.read().await;
            subscriptions.get(id).cloned()
        };

        let Some(subscription) = subscription else {
            return Ok(());
        };

        let next_payload: NextPayload = serde_json::from_value(payload).change_context(
            Error::SerdeDeserialize("Failed to deserialize Codex websocket payload".to_string()),
        )?;

        if let Some(data) = next_payload.data {
            if let Some(update) = data.on_price_updated {
                let decimals = default_decimals(&subscription.token);
                if let Err(error) = subscription.updates_tx.send(Some(TokenPrice {
                    price: update.price_usd,
                    decimals,
                })) {
                    tracing::error!(
                        "Failed to send Codex price update for {}: {:?}",
                        subscription.token.address,
                        error
                    );
                }
            }

            if let Some(errors) = next_payload.errors {
                tracing::error!(
                    "Errors in Codex websocket payload for {}: {:?}",
                    subscription.token.address,
                    errors
                );
            }
        }

        Ok(())
    }

    async fn handle_complete(&self, id: &str) {
        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.remove(id);
    }

    fn send_message(&self, message: Message) -> EstimatorResult<()> {
        self.sender
            .send(message)
            .map_err(|error| report!(Error::ResponseError).attach_printable(format!("{error:?}")))
    }

    async fn wait_for_connection(&self, timeout: Duration) -> EstimatorResult<()> {
        if self.connected.load(Ordering::Acquire) {
            return Ok(());
        }

        time::timeout(timeout, self.connected_notify.notified())
            .await
            .change_context(Error::ResponseError)
            .attach_printable("Timed out waiting for Codex websocket connection_ack")?;

        Ok(())
    }

    async fn subscribe(self: &Arc<Self>, token: TokenId) -> EstimatorResult<CodexSubscription> {
        let key = subscription_id(&token);

        let (receiver, needs_subscribe) = {
            let mut subscriptions = self.subscriptions.write().await;
            if let Some(entry) = subscriptions.get_mut(&key) {
                entry.ref_count += 1;
                (entry.updates_tx.subscribe(), false)
            } else {
                let (tx, rx) = watch::channel(None);
                subscriptions.insert(
                    key.clone(),
                    TokenSubscription {
                        token: token.clone(),
                        updates_tx: tx,
                        ref_count: 1,
                    },
                );
                (rx, true)
            }
        };

        if needs_subscribe {
            let message = serde_json::json!({
                "id": key,
                "type": "subscribe",
                "payload": {
                    "query": GRAPHQL_SUBSCRIPTION,
                    "variables": {
                        "address": token.address,
                        "networkId": token.chain.to_codex_chain_number()
                    }
                }
            });
            self.send_message(Message::Text(message.to_string()))?;
        }

        Ok(CodexSubscription::new(self.clone(), key, receiver))
    }

    async fn unsubscribe(&self, token: &TokenId) -> EstimatorResult<()> {
        let key = subscription_id(token);
        self.release_subscription(key).await
    }

    async fn release_subscription(&self, key: String) -> EstimatorResult<()> {
        let should_unsubscribe = {
            let mut subscriptions = self.subscriptions.write().await;
            if let Some(entry) = subscriptions.get_mut(&key) {
                entry.ref_count = entry.ref_count.saturating_sub(1);
                if entry.ref_count == 0 {
                    subscriptions.remove(&key);
                    true
                } else {
                    false
                }
            } else {
                false
            }
        };

        if should_unsubscribe {
            let message = serde_json::json!({
                "id": key,
                "type": "complete"
            });
            self.send_message(Message::Text(message.to_string()))?;
        }

        Ok(())
    }

    async fn latest_price(&self, token: &TokenId) -> Option<TokenPrice> {
        let key = subscription_id(token);
        let subscriptions = self.subscriptions.read().await;
        subscriptions
            .get(&key)
            .and_then(|entry| entry.updates_tx.borrow().clone())
    }
}

#[derive(Debug)]
pub struct CodexSubscription {
    client: Arc<CodexWsClient>,
    key: String,
    updates_rx: watch::Receiver<Option<TokenPrice>>,
}

impl CodexSubscription {
    fn new(
        client: Arc<CodexWsClient>,
        key: String,
        updates_rx: watch::Receiver<Option<TokenPrice>>,
    ) -> Self {
        Self {
            client,
            key,
            updates_rx,
        }
    }

    pub fn latest(&self) -> Option<TokenPrice> {
        self.updates_rx.borrow().clone()
    }

    pub async fn wait_for_price(&mut self, timeout: Duration) -> EstimatorResult<TokenPrice> {
        if let Some(price) = self.updates_rx.borrow().clone() {
            return Ok(price);
        }

        time::timeout(timeout, async {
            loop {
                if self.updates_rx.changed().await.is_err() {
                    return Err(report!(Error::ResponseError)
                        .attach_printable("Codex subscription closed before receiving price"));
                }
                if let Some(price) = self.updates_rx.borrow().clone() {
                    return Ok(price);
                }
            }
        })
        .await
        .change_context(Error::ResponseError)
        .attach_printable("Timed out waiting for Codex price update")
        .and_then(|result| result)
    }

    pub async fn next_update(&mut self) -> EstimatorResult<TokenPrice> {
        loop {
            if self.updates_rx.changed().await.is_err() {
                return Err(
                    report!(Error::ResponseError).attach_printable("Codex subscription closed")
                );
            }
            if let Some(price) = self.updates_rx.borrow().clone() {
                return Ok(price);
            }
        }
    }
}

impl Drop for CodexSubscription {
    fn drop(&mut self) {
        let client = self.client.clone();
        let key = self.key.clone();
        tokio::spawn(async move {
            if let Err(error) = client.release_subscription(key).await {
                tracing::error!("Failed to release Codex subscription: {:?}", error);
            }
        });
    }
}

#[derive(Debug, Deserialize)]
struct GraphqlWsMessage {
    #[serde(rename = "type")]
    message_type: String,
    id: Option<String>,
    payload: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct NextPayload {
    data: Option<NextData>,
    errors: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
struct NextData {
    #[serde(rename = "onPriceUpdated")]
    on_price_updated: Option<OnPriceUpdated>,
}

#[derive(Debug, Deserialize)]
struct OnPriceUpdated {
    #[serde(rename = "priceUsd")]
    price_usd: f64,
}

fn subscription_id(token: &TokenId) -> String {
    format!(
        "{}:{}",
        token.chain.to_codex_chain_number(),
        token.address.to_lowercase()
    )
}

fn default_decimals(token: &TokenId) -> u8 {
    match token.chain {
        ChainId::Solana | ChainId::Sui => 9,
        _ => 18,
    }
}
