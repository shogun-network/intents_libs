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
use reqwest::{
    Client as HttpClient,
    header::{AUTHORIZATION, HeaderMap, HeaderValue as ReqwestHeaderValue},
};
use tokio::{
    sync::{Mutex, Notify, OnceCell, RwLock, broadcast, watch},
    time,
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
};

use crate::{
    error::{Error, EstimatorResult},
    prices::{
        PriceEvent, PriceProvider, TokenId, TokenMetadata, TokenPrice,
        codex::{
            CODEX_HTTP_URL, CODEX_WS_URL, CodexChain,
            models::{
                CodexGetTrendingTokensData, CodexGraphqlResponse, GraphqlWsMessage, NextPayload,
                TokenSubscription, TrendingTokenData,
            },
            utils::{
                assemble_get_metadata_results, assemble_get_prices_results,
                assemble_price_and_metadata_results, combine_get_metadata_query,
                combine_get_prices_query, combine_price_and_metadata_query, default_decimals,
                subscription_id,
            },
        },
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

const MAX_SUBSCRIPTIONS_PER_CONNECTION: usize = 20;
const MAX_CONNECTIONS: usize = 300;

const TRENDING_TOKENS_QUERY: &str = r#"
query FilterTokens(
    $minLiquidity: Float!,
    $minMarketCap: Float!,
    $network: Int!,
    $minVolume24: Float!
    $limit: Int!
    $offset: Int!
) {
    filterTokens(
        rankings: {attribute: change1, direction: DESC}
        filters: {
            liquidity: { gt: $minLiquidity },
            marketCap: { gt: $minMarketCap },
            network: $network,
            volume24: { gt: $minVolume24 }
        }
        statsType: FILTERED
        limit: $limit
        offset: $offset
    ) {
        results {
            token {
                name
                symbol
                decimals
                address
                networkId
            }
            marketCap
            liquidity
            holders
            volume24
            walletAgeAvg
            buyCount24
        }
    }
}
"#;

#[derive(Debug, Clone)]
pub struct CodexProvider {
    api_key: String,
    pool: Arc<OnceCell<Arc<CodexConnectionPool>>>,
}

impl CodexProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            pool: Arc::new(OnceCell::new()),
        }
    }

    async fn pool(&self) -> EstimatorResult<Arc<CodexConnectionPool>> {
        let api_key = self.api_key.clone();
        let reference = self
            .pool
            .get_or_try_init(|| async move { CodexConnectionPool::new(api_key).map(Arc::new) })
            .await?;
        Ok(reference.clone())
    }

    pub async fn subscribe(&self, token: TokenId) -> EstimatorResult<CodexSubscription> {
        tracing::debug!(
            "Subscribing to Codex price for token {} on chain {:?}",
            token.address,
            token.chain
        );
        let pool = self.pool().await?;
        pool.subscribe(token).await
    }

    pub async fn subscribe_internal(&self, token: TokenId) -> EstimatorResult<()> {
        tracing::debug!(
            "Subscribing to Codex price for token {} on chain {:?}",
            token.address,
            token.chain
        );
        let pool = self.pool().await?;
        pool.subscribe_internal(token).await
    }

    pub async fn unsubscribe(&self, token: &TokenId) -> EstimatorResult<()> {
        tracing::debug!(
            "Unsubscribing from Codex price for token {} on chain {:?}",
            token.address,
            token.chain
        );
        let pool = self.pool().await?;
        pool.unsubscribe(&token).await
    }

    pub async fn unsubscribe_internal(&self, token: &TokenId) -> EstimatorResult<bool> {
        tracing::debug!(
            "Unsubscribing from Codex price for token {} on chain {:?}",
            token.address,
            token.chain
        );
        let pool = self.pool().await?;
        pool.unsubscribe_internal(&token).await
    }

    pub async fn latest_price(&self, token: &TokenId) -> EstimatorResult<Option<TokenPrice>> {
        let pool = self.pool().await?;
        Ok(pool.latest_price(&token).await)
    }

    pub async fn fetch_initial_prices(
        &self,
        tokens: &[TokenId],
    ) -> EstimatorResult<HashMap<TokenId, TokenPrice>> {
        let pool = self.pool().await?;
        pool.fetch_price_and_metadata(&tokens).await
    }

    pub async fn fetch_trending_tokens(
        &self,
        min_liquidity: f64,
        min_market_cap: f64,
        network: ChainId,
        min_volume_24: f64,
        limit: u32,
        offset: u32,
    ) -> EstimatorResult<Vec<TrendingTokenData>> {
        let pool = self.pool().await?;
        pool.fetch_trending_tokens(
            min_liquidity,
            min_market_cap,
            network,
            min_volume_24,
            limit,
            offset,
        )
        .await
    }

    // Public method to subscribe to the global price event stream
    pub async fn subscribe_events(&self) -> EstimatorResult<broadcast::Receiver<PriceEvent>> {
        let pool = self.pool().await?;
        Ok(pool.get_events_subscriber())
    }

    pub async fn fetch_token_metadata(
        &self,
        tokens: &[TokenId],
    ) -> EstimatorResult<HashMap<TokenId, TokenMetadata>> {
        let pool = self.pool().await?;
        pool.fetch_token_metadata(&tokens).await
    }
}

const PRICE_EVENTS_BUFFER: usize = 32768; // 2^15

#[derive(Debug)]
struct CodexConnectionPool {
    api_key: String,
    http_client: HttpClient,
    clients: RwLock<Vec<Arc<CodexWsClient>>>,
    // Event bus for price updates
    event_tx: broadcast::Sender<PriceEvent>,
    // Anchor subscriptions to keep WS alive until explicit unsubscribe
    held_subscriptions: RwLock<HashMap<TokenId, (usize, CodexSubscription)>>,
}

impl CodexConnectionPool {
    fn new(api_key: String) -> EstimatorResult<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            ReqwestHeaderValue::from_str(&api_key)
                .change_context(Error::ResponseError)
                .attach_printable("Invalid characters in CODEX_API_KEY")?,
        );

        let http_client = HttpClient::builder()
            .default_headers(headers)
            .build()
            .change_context(Error::ResponseError)
            .attach_printable("Failed to build Codex HTTP client")?;

        let (event_tx, _event_rx) = broadcast::channel(PRICE_EVENTS_BUFFER);

        Ok(Self {
            api_key,
            http_client,
            clients: RwLock::new(Vec::new()),
            event_tx,
            held_subscriptions: RwLock::new(HashMap::new()),
        })
    }

    // Allow external components to subscribe to the global stream of events
    fn get_events_subscriber(&self) -> broadcast::Receiver<PriceEvent> {
        self.event_tx.subscribe()
    }

    async fn subscribe(&self, token: TokenId) -> EstimatorResult<CodexSubscription> {
        tracing::debug!(
            "Subscribing in CodexConnectionPool to Codex token: {:?}",
            token
        );
        let key = subscription_id(&token);

        if let Some(client) = self.client_with_subscription(&key).await {
            return client.subscribe(token).await;
        }

        let client = self.client_with_capacity().await?;
        let subscribe_future = client.subscribe(token.clone());
        let tokens_to_search = vec![token.clone()];
        let price_future = self.fetch_price_and_metadata(&tokens_to_search);
        let (subscription_result, price_result) = tokio::join!(subscribe_future, price_future);

        let subscription = subscription_result?;

        match price_result {
            Ok(result) => match result.get(&token) {
                Some(price) => {
                    client
                        .apply_initial_price(&subscription.key, price.clone())
                        .await;
                }
                None => {}
            },
            Err(error) => {
                tracing::warn!(
                    "Failed to fetch initial Codex price for {} on {:?}: {:?}",
                    token.address,
                    token.chain,
                    error
                );
            }
        }

        Ok(subscription)
    }

    async fn subscribe_internal(&self, token: TokenId) -> EstimatorResult<()> {
        tracing::debug!(
            "Subscribing internally in CodexConnectionPool to Codex token: {:?}",
            token
        );
        // Fast path: already anchored
        {
            let mut held = self.held_subscriptions.write().await;
            if let Some((rc, _anchor)) = held.get_mut(&token) {
                *rc = rc.saturating_add(1);
                return Ok(());
            }
        }

        // Slow path: create anchor without holding the lock
        let client = self.client_with_capacity().await?;
        let anchor = client.subscribe(token.clone()).await?;

        // Insert anchor; if a race inserted first, bump and drop our extra handle
        let mut held = self.held_subscriptions.write().await;
        if let std::collections::hash_map::Entry::Occupied(mut occ) = held.entry(token.clone()) {
            // Another task anchored meanwhile; drop our extra anchor to decrement WS refcount
            drop(anchor);
            let (rc, _existing) = occ.get_mut();
            *rc = rc.saturating_add(1);
        } else {
            held.insert(token, (1, anchor));
        }
        Ok(())
    }

    async fn unsubscribe(&self, token: &TokenId) -> EstimatorResult<()> {
        let key = subscription_id(token);
        if let Some(client) = self.client_with_subscription(&key).await {
            client.unsubscribe(token).await?;
        }
        Ok(())
    }

    async fn unsubscribe_internal(&self, token: &TokenId) -> EstimatorResult<bool> {
        let to_drop = {
            let mut held = self.held_subscriptions.write().await;

            let (_rc, anchor_owned) = held.remove(token).expect("entry must exist");
            anchor_owned
        };
        drop(to_drop);
        Ok(true)
    }

    async fn latest_price(&self, token: &TokenId) -> Option<TokenPrice> {
        let key = subscription_id(token);
        if let Some(client) = self.client_with_subscription(&key).await {
            return client.latest_price(token).await;
        }
        None
    }

    async fn fetch_trending_tokens(
        &self,
        min_liquidity: f64,
        min_market_cap: f64,
        network: ChainId,
        min_volume_24: f64,
        limit: u32,
        offset: u32,
    ) -> EstimatorResult<Vec<TrendingTokenData>> {
        let response = self
            .http_client
            .post(CODEX_HTTP_URL)
            .json(&serde_json::json!({
                "query": TRENDING_TOKENS_QUERY,
                "variables": {
                    "minLiquidity": min_liquidity,
                    "minMarketCap": min_market_cap,
                    "network": network.to_codex_chain_number(),
                    "minVolume24": min_volume_24,
                    "limit": limit,
                    "offset": offset
                }
            }))
            .send()
            .await
            .change_context(Error::ResponseError)
            .attach_printable("Failed to send Codex HTTP price request")?;

        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .change_context(Error::ResponseError)
                .attach_printable("Failed to read Codex HTTP error response")?;
            return Err(report!(Error::ResponseError).attach_printable(format!(
                "Codex HTTP price request failed with status {}: {}",
                status.as_u16(),
                body
            )));
        }

        let payload: serde_json::Value = response
            .json()
            .await
            .change_context(Error::ResponseError)
            .attach_printable("Failed to deserialize Codex HTTP price response")?;

        tracing::debug!("Codex HTTP price response payload: {:#?}", payload);

        let payload =
            serde_json::from_value::<CodexGraphqlResponse<CodexGetTrendingTokensData>>(payload)
                .change_context(Error::SerdeDeserialize(
                    "Failed to deserialize Codex HTTP price GraphQL response".to_string(),
                ))?;

        if let Some(errors) = payload.errors.as_ref() {
            if !errors.is_empty() {
                tracing::warn!(
                    "Codex HTTP price batch response contained errors: {:?}",
                    errors
                );
            }
        }

        let Some(data) = payload.data else {
            return Err(report!(Error::ResponseError)
                .attach_printable(format!("No data found in Codex HTTP price response")));
        };

        Ok(data.filter_tokens.results)
    }

    async fn fetch_prices(
        &self,
        tokens: &[TokenId],
    ) -> EstimatorResult<HashMap<TokenId, TokenPrice>> {
        if tokens.is_empty() {
            return Ok(HashMap::new());
        }

        let body = combine_get_prices_query(tokens)?;

        let response = self
            .http_client
            .post(CODEX_HTTP_URL)
            .json(&body)
            .send()
            .await
            .change_context(Error::ResponseError)
            .attach_printable("Failed to send Codex HTTP price request")?;

        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .change_context(Error::ResponseError)
                .attach_printable("Failed to read Codex HTTP error response")?;
            return Err(report!(Error::ResponseError).attach_printable(format!(
                "Codex HTTP price request failed with status {}: {}",
                status.as_u16(),
                body
            )));
        }

        let payload: serde_json::Value = response
            .json()
            .await
            .change_context(Error::ResponseError)
            .attach_printable("Failed to deserialize Codex HTTP price response")?;

        let data = assemble_get_prices_results(tokens.len(), payload)?;

        let mut out = HashMap::new();
        for price in data.prices.into_iter() {
            let Some(price) = price else {
                continue;
            };
            let token_id = TokenId {
                chain: ChainId::from_codex_chain_number(price.network_id)
                    .ok_or(Error::ParseError)?,
                address: price.address,
            };
            let price = TokenPrice {
                price: price.price_usd,
                decimals: default_decimals(token_id.chain),
            };
            out.insert(token_id.clone(), price);
        }

        Ok(out)
    }

    async fn fetch_token_metadata(
        &self,
        tokens: &[TokenId],
    ) -> EstimatorResult<HashMap<TokenId, TokenMetadata>> {
        if tokens.is_empty() {
            return Ok(HashMap::new());
        }

        let body = combine_get_metadata_query(tokens)?;

        let response = self
            .http_client
            .post(CODEX_HTTP_URL)
            .json(&body)
            .send()
            .await
            .change_context(Error::ResponseError)
            .attach_printable("Failed to send Codex HTTP price request")?;

        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .change_context(Error::ResponseError)
                .attach_printable("Failed to read Codex HTTP error response")?;
            return Err(report!(Error::ResponseError).attach_printable(format!(
                "Codex HTTP price request failed with status {}: {}",
                status.as_u16(),
                body
            )));
        }

        let payload: serde_json::Value = response
            .json()
            .await
            .change_context(Error::ResponseError)
            .attach_printable("Failed to deserialize Codex HTTP price response")?;

        let data = assemble_get_metadata_results(tokens.len(), payload)?;

        let mut out = HashMap::new();
        for meta in data.meta.into_iter() {
            let Some(meta) = meta else {
                continue;
            };
            // Map back to the original requested TokenId using (networkId, lowercase address)
            let token_id = TokenId {
                chain: ChainId::from_codex_chain_number(meta.network_id)
                    .ok_or(Error::ParseError)?,
                address: meta.address,
            };
            let token_meta = TokenMetadata {
                name: meta.name,
                symbol: meta.symbol,
                decimals: meta.decimals,
            };
            out.insert(token_id.clone(), token_meta);
        }

        Ok(out)
    }

    async fn fetch_price_and_metadata(
        &self,
        tokens: &[TokenId],
    ) -> EstimatorResult<HashMap<TokenId, TokenPrice>> {
        if tokens.is_empty() {
            return Ok(HashMap::new());
        }

        // Create json body with query and inputs
        let body = combine_price_and_metadata_query(tokens)?;

        let response = self
            .http_client
            .post(CODEX_HTTP_URL)
            .json(&body)
            .send()
            .await
            .change_context(Error::ResponseError)
            .attach_printable("Failed to send Codex HTTP price and metadata request")?;

        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .change_context(Error::ResponseError)
                .attach_printable("Failed to read Codex HTTP error response")?;
            return Err(report!(Error::ResponseError).attach_printable(format!(
                "Codex HTTP price request failed with status {}: {}",
                status.as_u16(),
                body
            )));
        }

        let payload: serde_json::Value = response
            .json()
            .await
            .change_context(Error::ResponseError)
            .attach_printable("Failed to deserialize Codex HTTP price response")?;

        let data = assemble_price_and_metadata_results(tokens.len(), payload)?;

        let mut out = HashMap::new();
        for (price, meta) in data.prices.into_iter().zip(data.meta.into_iter()) {
            let (Some(price), Some(meta)) = (price, meta) else {
                continue;
            };
            let token_id = TokenId {
                chain: ChainId::from_codex_chain_number(meta.network_id)
                    .ok_or(Error::ParseError)?,
                address: meta.address,
            };
            let price = TokenPrice {
                price: price.price_usd,
                decimals: meta.decimals,
            };
            out.insert(token_id.clone(), price);
        }

        Ok(out)
    }

    async fn client_with_subscription(&self, key: &str) -> Option<Arc<CodexWsClient>> {
        for client in self.snapshot_clients().await {
            if client.contains_subscription(key).await {
                return Some(client);
            }
        }
        None
    }

    async fn client_with_capacity(&self) -> EstimatorResult<Arc<CodexWsClient>> {
        for client in self.snapshot_clients().await {
            if client.has_capacity().await {
                return Ok(client);
            }
        }

        {
            let clients = self.clients.read().await;
            if clients.len() >= MAX_CONNECTIONS {
                return Err(report!(Error::ResponseError).attach_printable(format!(
                    "Codex websocket connection limit ({MAX_CONNECTIONS}) reached"
                )));
            }
        }

        let client = CodexWsClient::connect(self.api_key.clone(), self.event_tx.clone()).await?;

        let mut clients = self.clients.write().await;
        if clients.len() >= MAX_CONNECTIONS {
            return Err(report!(Error::ResponseError).attach_printable(format!(
                "Codex websocket connection limit ({MAX_CONNECTIONS}) reached"
            )));
        }
        clients.push(client.clone());

        Ok(client)
    }

    async fn snapshot_clients(&self) -> Vec<Arc<CodexWsClient>> {
        let clients = self.clients.read().await;
        clients.iter().cloned().collect()
    }
}

#[async_trait::async_trait]
impl PriceProvider for CodexProvider {
    async fn get_tokens_price(
        &self,
        tokens: &[TokenId],
        with_subscriptions: bool,
    ) -> EstimatorResult<HashMap<TokenId, TokenPrice>> {
        if tokens.is_empty() {
            return Ok(HashMap::new());
        }

        let pool = self.pool().await?;
        let mut result: HashMap<TokenId, TokenPrice> = HashMap::new();
        let mut missing: HashSet<TokenId> = tokens.into_iter().cloned().collect();

        // Try to use the latest price already available from the watch channel
        if with_subscriptions {
            for token in tokens.iter() {
                match pool.latest_price(&token).await {
                    Some(price) => {
                        missing.remove(&token);
                        result.insert(token.clone(), price);
                    }
                    None => {}
                }
            }
        }

        // For missing tokens, fetch price via HTTP
        let missing = missing.into_iter().collect::<Vec<_>>();
        match pool.fetch_prices(&missing).await {
            Ok(prices) => {
                for (token, price) in prices.into_iter() {
                    result.insert(token, price);
                }
            }
            Err(err) => {
                tracing::error!(
                    "Failed to fetch initial Codex prices for missing tokens: {:?}",
                    err
                );
            }
        }

        Ok(result)
    }

    async fn get_tokens_prices_events(
        &self,
    ) -> EstimatorResult<tokio::sync::broadcast::Receiver<PriceEvent>> {
        self.subscribe_events().await
    }

    async fn subscribe_to_token(&self, token: TokenId) -> EstimatorResult<()> {
        self.subscribe_internal(token).await.map(|_| ())
    }

    async fn unsubscribe_from_token(&self, token: TokenId) -> EstimatorResult<bool> {
        self.unsubscribe_internal(&token).await
    }
}

#[derive(Debug)]
struct CodexWsClient {
    sender: tokio::sync::mpsc::UnboundedSender<Message>,
    subscriptions: RwLock<HashMap<String, TokenSubscription>>,
    connected: AtomicBool,
    connected_notify: Notify,
    // Event bus for price updates
    event_tx: broadcast::Sender<PriceEvent>,
}

impl CodexWsClient {
    async fn connect(
        api_key: String,
        event_tx: broadcast::Sender<PriceEvent>,
    ) -> EstimatorResult<Arc<Self>> {
        let mut request = CODEX_WS_URL
            .into_client_request()
            .change_context(Error::ResponseError)
            .attach_printable("Failed to construct Codex websocket request")?;

        request.headers_mut().insert(
            "Sec-WebSocket-Protocol",
            ReqwestHeaderValue::from_static("graphql-transport-ws"),
        );
        request.headers_mut().insert(
            "Authorization",
            ReqwestHeaderValue::from_str(&api_key)
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
            event_tx,
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
                let decimals = default_decimals(subscription.token.chain);
                let new_price = TokenPrice {
                    price: update.price_usd,
                    decimals,
                };

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

                // Emit global event
                if let Err(err) = self.event_tx.send(PriceEvent {
                    token: subscription.token.clone(),
                    price: new_price,
                }) {
                    // If there are no subscribers or receivers lagged, just log and continue
                    tracing::trace!(
                        "No listeners for price event or lagging receivers: {:?}",
                        err
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
        tracing::debug!("Subscribing in CodexWsClient to Codex token: {:?}", token);
        let key = subscription_id(&token);

        let (receiver, needs_subscribe) = {
            let mut subscriptions = self.subscriptions.write().await;
            if let Some(entry) = subscriptions.get_mut(&key) {
                entry.ref_count += 1;
                (entry.updates_tx.subscribe(), false)
            } else {
                if subscriptions.len() >= MAX_SUBSCRIPTIONS_PER_CONNECTION {
                    return Err(
                        report!(Error::ResponseError).attach_printable(format!(
                            "Codex websocket subscription limit per connection ({MAX_SUBSCRIPTIONS_PER_CONNECTION}) exceeded"
                        )),
                    );
                }
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

    async fn has_capacity(&self) -> bool {
        let subscriptions = self.subscriptions.read().await;
        subscriptions.len() < MAX_SUBSCRIPTIONS_PER_CONNECTION
    }

    async fn contains_subscription(&self, key: &str) -> bool {
        let subscriptions = self.subscriptions.read().await;
        subscriptions.contains_key(key)
    }

    async fn apply_initial_price(&self, key: &str, price: TokenPrice) {
        let subscriptions = self.subscriptions.read().await;
        if let Some(entry) = subscriptions.get(key) {
            if let Err(error) = entry.updates_tx.send(Some(price)) {
                tracing::warn!(
                    "Failed to seed initial Codex price for {}: {:?}",
                    entry.token.address,
                    error
                );
            }
        }
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
        tracing::debug!("Unsubscribing from Codex token: {:?}", self.key);
        let client = self.client.clone();
        let key = self.key.clone();
        tokio::spawn(async move {
            if let Err(error) = client.release_subscription(key).await {
                tracing::error!("Failed to release Codex subscription: {:?}", error);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::init_tracing_in_tests;
    use intents_models::constants::chains::NATIVE_TOKEN_SUI_ADDRESS;

    #[tokio::test]
    async fn test_trending_tokens_fetch() {
        dotenv::dotenv().ok();
        init_tracing_in_tests();

        let codex_api_key = match std::env::var("CODEX_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                eprintln!("Skipping CodexProvider test: CODEX_API_KEY not set");
                return;
            }
        };
        let codex_provider = CodexProvider::new(codex_api_key);

        let trending_tokens = codex_provider
            .fetch_trending_tokens(10_000.0, 100_000.0, ChainId::Solana, 5_000.0, 2, 0)
            .await
            .expect("Failed to fetch Codex trending tokens");
        println!("Codex trending tokens: {:#?}", trending_tokens);
        assert!(!trending_tokens.is_empty());
        let trending_tokens = codex_provider
            .fetch_trending_tokens(10_000.0, 100_000.0, ChainId::Solana, 5_000.0, 2, 2)
            .await
            .expect("Failed to fetch Codex trending tokens");
        println!("Codex trending tokens: {:#?}", trending_tokens);
        assert!(!trending_tokens.is_empty());
    }

    #[tokio::test]
    async fn test_codex_get_tokens_price_success() {
        dotenv::dotenv().ok();
        init_tracing_in_tests();

        let codex_api_key = match std::env::var("CODEX_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                eprintln!("Skipping CodexProvider test: CODEX_API_KEY not set");
                return;
            }
        };
        let codex_provider = CodexProvider::new(codex_api_key);

        let tokens = HashSet::from([
            TokenId {
                chain: ChainId::Solana,
                address: "So11111111111111111111111111111111111111112".to_string(),
            },
            TokenId::new_for_codex(ChainId::Sui, NATIVE_TOKEN_SUI_ADDRESS),
            TokenId {
                chain: ChainId::Solana,
                address: "G6jmigL9nkgYrT9MFP5fvrgrztDhtdVZkrmQz5Q5bonk".to_string(),
            },
            TokenId {
                chain: ChainId::Solana,
                address: "3sNToh4Z3WJyqzMMDP34Jjiw9PLcW8KabuewS1EB8ray".to_string(),
            },
            TokenId {
                chain: ChainId::Solana,
                address: "55E5Bn6n3L44tjfUBc18turPsdSBvs8MVb22oeM9robo".to_string(),
            },
            TokenId {
                chain: ChainId::Solana,
                address: "GTEPYkUDfArmcijxE2Z4g54TuNHECzMnrntYkyPapump".to_string(),
            },
            TokenId {
                chain: ChainId::Solana,
                address: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            },
            TokenId {
                chain: ChainId::Base,
                address: "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string(),
            },
            TokenId {
                chain: ChainId::Ethereum,
                address: "0x3fc29836e84e471a053d2d9e80494a867d670ead".to_string(),
            },
        ]);

        let tokens_price = codex_provider
            .get_tokens_price(&tokens.iter().cloned().collect::<Vec<_>>(), false)
            .await
            .expect("Failed to get Codex tokens price");
        println!("Codex tokens price: {:#?}", tokens_price);
        for token in tokens.into_iter() {
            assert!(tokens_price.contains_key(&token));
        }
    }

    #[tokio::test]
    async fn test_codex_get_tokens_metadata_success() {
        dotenv::dotenv().ok();
        init_tracing(false);

        let codex_api_key = match std::env::var("CODEX_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                eprintln!("Skipping CodexProvider test: CODEX_API_KEY not set");
                return;
            }
        };
        let codex_provider = CodexProvider::new(codex_api_key);

        let tokens = HashSet::from([
            TokenId {
                chain: ChainId::Solana,
                address: "So11111111111111111111111111111111111111112".to_string(),
            },
            TokenId::new_for_codex(ChainId::Sui, NATIVE_TOKEN_SUI_ADDRESS),
            TokenId {
                chain: ChainId::Solana,
                address: "G6jmigL9nkgYrT9MFP5fvrgrztDhtdVZkrmQz5Q5bonk".to_string(),
            },
            TokenId {
                chain: ChainId::Solana,
                address: "3sNToh4Z3WJyqzMMDP34Jjiw9PLcW8KabuewS1EB8ray".to_string(),
            },
            TokenId {
                chain: ChainId::Solana,
                address: "55E5Bn6n3L44tjfUBc18turPsdSBvs8MVb22oeM9robo".to_string(),
            },
            TokenId {
                chain: ChainId::Solana,
                address: "GTEPYkUDfArmcijxE2Z4g54TuNHECzMnrntYkyPapump".to_string(),
            },
            TokenId {
                chain: ChainId::Solana,
                address: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            },
            TokenId {
                chain: ChainId::Base,
                address: "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string(),
            },
            TokenId {
                chain: ChainId::Ethereum,
                address: "0x3fc29836e84e471a053d2d9e80494a867d670ead".to_string(),
            },
        ]);

        let tokens_metadata = codex_provider
            .fetch_token_metadata(&tokens.iter().cloned().collect::<Vec<_>>())
            .await
            .expect("Failed to get Codex tokens price");
        println!("Codex tokens metadata: {:#?}", tokens_metadata);
        for token in tokens.into_iter() {
            assert!(tokens_metadata.contains_key(&token));
        }
    }

    #[tokio::test]
    async fn test_codex_get_tokens_price_and_metadata_success() {
        dotenv::dotenv().ok();
        init_tracing_in_tests();

        let codex_api_key = match std::env::var("CODEX_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                eprintln!("Skipping CodexProvider test: CODEX_API_KEY not set");
                return;
            }
        };
        let codex_provider = CodexProvider::new(codex_api_key);

        let tokens = Vec::from([
            TokenId {
                chain: ChainId::Solana,
                address: "So11111111111111111111111111111111111111112".to_string(),
            },
            TokenId::new_for_codex(ChainId::Sui, NATIVE_TOKEN_SUI_ADDRESS),
            TokenId {
                chain: ChainId::Solana,
                address: "G6jmigL9nkgYrT9MFP5fvrgrztDhtdVZkrmQz5Q5bonk".to_string(),
            },
            TokenId {
                chain: ChainId::Solana,
                address: "3sNToh4Z3WJyqzMMDP34Jjiw9PLcW8KabuewS1EB8ray".to_string(),
            },
            TokenId {
                chain: ChainId::Solana,
                address: "55E5Bn6n3L44tjfUBc18turPsdSBvs8MVb22oeM9robo".to_string(),
            },
            TokenId {
                chain: ChainId::Solana,
                address: "GTEPYkUDfArmcijxE2Z4g54TuNHECzMnrntYkyPapump".to_string(),
            },
            TokenId {
                chain: ChainId::Solana,
                address: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            },
            TokenId {
                chain: ChainId::Base,
                address: "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string(),
            },
            TokenId {
                chain: ChainId::Ethereum,
                address: "0x3fc29836e84e471a053d2d9e80494a867d670ead".to_string(),
            },
            TokenId {
                chain: ChainId::Bsc,
                address: "0x5449ecb1da825204003abba9b4cb9f139417aa89".to_string(),
            },
        ]);

        let tokens_price_and_meta = codex_provider.fetch_initial_prices(&tokens).await;
        println!(
            "Codex tokens price and meta fetch_initial_prices result: {:#?}",
            tokens_price_and_meta
        );
        let tokens_price_and_meta =
            tokens_price_and_meta.expect("Failed to get Codex tokens price and metadata");
        println!("Codex tokens price and meta: {:#?}", tokens_price_and_meta);
        for token in tokens.into_iter() {
            assert!(tokens_price_and_meta.contains_key(&token));
        }
    }

    #[tokio::test]
    async fn test_codex_get_tokens_price_unexisting_token() {
        dotenv::dotenv().ok();
        init_tracing_in_tests();

        let codex_api_key = match std::env::var("CODEX_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                eprintln!("Skipping CodexProvider test: CODEX_API_KEY not set");
                return;
            }
        };
        let codex_provider = CodexProvider::new(codex_api_key);

        let tokens = HashSet::from([
            TokenId {
                chain: ChainId::Solana,
                address: "So11111111111111111111111111111111111111112".to_string(),
            },
            TokenId {
                chain: ChainId::Base,
                address: "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string(),
            },
            // Non-existing token
            TokenId {
                chain: ChainId::Base,
                address: "0x103589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string(),
            },
        ]);

        let tokens_price = codex_provider
            .get_tokens_price(&tokens.iter().cloned().collect::<Vec<_>>(), false)
            .await
            .expect("Failed to get Codex tokens price");
        println!("Codex tokens price: {:#?}", tokens_price);
        let mut count = 0;
        for token in tokens.into_iter() {
            if tokens_price.contains_key(&token) {
                count += 1;
            }
        }
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_codex_subscription_broadcast_event() {
        dotenv::dotenv().ok();
        init_tracing_in_tests();

        let codex_api_key = match std::env::var("CODEX_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                eprintln!("Skipping CodexProvider test: CODEX_API_KEY not set");
                return;
            }
        };

        // Use a short refresh interval to speed up the test
        let codex_provider: CodexProvider = CodexProvider::new(codex_api_key);

        // Popular token (Solana Bonk)
        let token = TokenId {
            chain: ChainId::Solana,
            address: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263".to_string(),
        };

        // Subscribe to token so the background refresher includes it in the snapshot
        codex_provider
            .subscribe_to_token(token.clone())
            .await
            .expect("subscribe_to_token failed");

        // Subscribe to the broadcast of price events
        let mut rx = codex_provider
            .subscribe_events()
            .await
            .expect("subscribe_events failed");

        // Wait for a matching event with a timeout
        let evt = tokio::time::timeout(Duration::from_secs(120), async {
            loop {
                match rx.recv().await {
                    Ok(event) if event.token == token => {
                        tracing::info!("Received price event for {:?}", token);
                        break event;
                    }
                    Ok(_) => {
                        tracing::info!("Received price event for different token");
                        continue;
                    } // Different token update; keep waiting
                    Err(e) => panic!("broadcast receiver error: {:?}", e),
                }
            }
        })
        .await
        .expect("Timed out waiting for GeckoTerminal price event");

        println!("Received Codex price event: {:?}", evt);
        assert!(
            evt.price.price > 0.0,
            "Expected positive price from GeckoTerminal"
        );

        // Unsubscribe and ensure the entry is removed when ref_count reaches zero
        codex_provider
            .unsubscribe_from_token(token.clone())
            .await
            .expect("unsubscribe_from_token failed");
    }

    #[tokio::test]
    async fn test_codex_subscription_and_unsuscription() {
        dotenv::dotenv().ok();
        init_tracing_in_tests();

        let codex_api_key = match std::env::var("CODEX_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                eprintln!("Skipping CodexProvider test: CODEX_API_KEY not set");
                return;
            }
        };

        // Use a short refresh interval to speed up the test
        let codex_provider: CodexProvider = CodexProvider::new(codex_api_key);

        // Popular token (Solana Bonk)
        let token = TokenId {
            chain: ChainId::Solana,
            address: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263".to_string(),
        };

        // Subscribe to token so the background refresher includes it in the snapshot
        codex_provider
            .subscribe_to_token(token.clone())
            .await
            .expect("subscribe_to_token failed");

        codex_provider
            .subscribe_to_token(token.clone())
            .await
            .expect("subscribe_to_token failed");

        // Subscribe to the broadcast of price events
        let mut rx = codex_provider
            .subscribe_events()
            .await
            .expect("subscribe_events failed");

        // Unsubscribe once
        codex_provider
            .unsubscribe_from_token(token.clone())
            .await
            .expect("unsubscribe_from_token failed");

        // Wait for a matching event with a timeout
        let evt = tokio::time::timeout(Duration::from_secs(120), async {
            loop {
                match rx.recv().await {
                    Ok(event) if event.token == token => {
                        tracing::info!("Received price event for {:?}", token);
                        println!("Received price event: {:#?}", event);
                        break event;
                    }
                    Ok(_) => {
                        tracing::info!("Received price event for different token");
                        continue;
                    } // Different token update; keep waiting
                    Err(e) => panic!("broadcast receiver error: {:?}", e),
                }
            }
        })
        .await
        .expect("Timed out waiting for GeckoTerminal price event");

        println!("Received Codex price event: {:?}", evt);
        assert!(
            evt.price.price > 0.0,
            "Expected positive price from GeckoTerminal"
        );

        // Try to get the latest price from the pool
        let latest_price = codex_provider.latest_price(&token).await;
        println!("Latest price from pool: {:#?}", latest_price);

        // Unsubscribe and ensure the entry is removed when ref_count reaches zero
        codex_provider
            .unsubscribe_from_token(token.clone())
            .await
            .expect("unsubscribe_from_token failed");
    }

    // #[tokio::test]
    // async fn test_codex_fake_token_subscription() {
    //     dotenv::dotenv().ok();
    //     init_tracing_in_tests();

    //     let codex_api_key = match std::env::var("CODEX_API_KEY") {
    //         Ok(key) => key,
    //         Err(_) => {
    //             eprintln!("Skipping CodexProvider test: CODEX_API_KEY not set");
    //             return;
    //         }
    //     };

    //     // Use a short refresh interval to speed up the test
    //     let codex_provider: CodexProvider = CodexProvider::new(codex_api_key);

    //     // Popular token (Solana Bonk)
    //     let token = TokenId {
    //         chain: ChainId::Solana,
    //         address: "DezXAZ8z7PnrnRJjz3wXaoRgixCa6xjnB7YaB1pPB263".to_string(),
    //     };

    //     // Subscribe to token so the background refresher includes it in the snapshot
    //     codex_provider
    //         .subscribe_to_token(token.clone())
    //         .await
    //         .expect("subscribe_to_token failed");
    // }
}
