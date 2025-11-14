use crate::prices::PriceEvent;
use crate::prices::gecko_terminal::GeckoTerminalChain;
use crate::{
    error::{Error, EstimatorResult},
    prices::{
        PriceProvider, TokenId, TokenPrice,
        gecko_terminal::{
            GECKO_TERMINAL_API_URL,
            responses::{
                GeckoTerminalOkResponseType, GeckoTerminalResponse, GeckoTerminalTokensInfo,
            },
        },
    },
};
use dashmap::{DashMap, Entry};
use error_stack::{ResultExt as _, report};
use intents_models::{constants::chains::ChainId, network::http::handle_reqwest_response};
use reqwest::Client;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time;

const PRICE_EVENTS_BUFFER: usize = 32768; // 2^15

#[derive(Debug, Clone)]
struct GtSubscriptionEntry {
    ref_count: usize,
    price: Option<TokenPrice>,
}

#[derive(Debug, Clone)]
pub struct GeckoTerminalProvider {
    client: Client,
    // Event bus for price updates
    event_tx: broadcast::Sender<PriceEvent>,
    subscriptions: Arc<DashMap<TokenId, GtSubscriptionEntry>>,
}

impl GeckoTerminalProvider {
    pub fn new() -> Self {
        let (event_tx, _event_rx) = broadcast::channel(PRICE_EVENTS_BUFFER);

        Self {
            client: Client::new(),
            event_tx,
            subscriptions: Arc::new(DashMap::new()),
        }
    }

    pub fn new_with_subscriptions(refresh_secs: u64) -> Self {
        let (event_tx, _event_rx) = broadcast::channel(PRICE_EVENTS_BUFFER);

        let provider = Self {
            client: Client::new(),
            event_tx,
            subscriptions: Arc::new(DashMap::new()),
        };

        provider.spawn_refresh_task(Duration::from_secs(refresh_secs));
        provider
    }

    // Public method to subscribe to the global price event stream
    pub fn subscribe_events(&self) -> broadcast::Receiver<PriceEvent> {
        self.event_tx.subscribe()
    }

    fn spawn_refresh_task(&self, interval: Duration) {
        let client = self.client.clone();
        let event_tx = self.event_tx.clone();
        let subscriptions = self.subscriptions.clone();

        tokio::spawn(async move {
            let mut ticker = time::interval(interval);
            loop {
                ticker.tick().await;

                // Take a snapshot of current TokenIds
                let snapshot: HashMap<TokenId, Option<TokenPrice>> = subscriptions
                    .iter()
                    .map(|a| (a.key().clone(), a.value().price.clone()))
                    .collect();
                if snapshot.is_empty() {
                    continue;
                }

                // Group by chain to minimize requests
                let mut by_chain: HashMap<ChainId, Vec<String>> = HashMap::new();
                for (token, _) in snapshot.iter() {
                    by_chain
                        .entry(token.chain)
                        .or_default()
                        .push(token.address.clone());
                }

                // Fetch and publish updates per chain
                for (chain, addresses) in by_chain.into_iter() {
                    match gecko_terminal_get_tokens_info(&client, chain, addresses).await {
                        Ok(infos) => {
                            for info in infos {
                                let token_id = TokenId::new(chain, info.attributes.address);

                                let price_f = match info.attributes.price_usd.parse::<f64>() {
                                    Ok(v) => v,
                                    Err(e) => {
                                        tracing::error!(
                                            "Failed to parse GeckoTerminal price for {} on {:?}: {:?}",
                                            token_id.address,
                                            chain,
                                            e
                                        );
                                        continue;
                                    }
                                };
                                let new_price = TokenPrice {
                                    price: price_f,
                                    decimals: info.attributes.decimals,
                                };

                                // Update only if still subscribed.
                                // Emit event if token price changed
                                if let Some(mut entry) = subscriptions.get_mut(&token_id) {
                                    // Check if price changed
                                    if let Some(old_price) = &entry.price {
                                        if (old_price.price - new_price.price).abs() <= 1e-9 {
                                            // No significant change
                                            continue;
                                        }
                                    }
                                    // Price changed
                                    entry.price = Some(new_price.clone());

                                    drop(entry); // Release lock before sending event

                                    tracing::debug!("Sending price event for {:?}", token_id);

                                    match event_tx.send(PriceEvent {
                                        token: token_id.clone(),
                                        price: new_price,
                                    }) {
                                        Ok(_) => {}
                                        Err(err) => {
                                            tracing::error!(
                                                "Failed to send price event for {:?}: {:?}",
                                                token_id,
                                                err
                                            );
                                        }
                                    };
                                } else {
                                    tracing::warn!("Not subscribed anymore: {:?}", token_id);
                                }
                            }
                        }
                        Err(err) => {
                            tracing::error!(
                                "GeckoTerminal refresh error for chain {:?}: {:?}",
                                chain,
                                err
                            );
                        }
                    }
                }
            }
        });
    }
}

#[async_trait::async_trait]
impl PriceProvider for GeckoTerminalProvider {
    async fn get_tokens_price(
        &self,
        tokens: HashSet<TokenId>,
        with_subscriptions: bool,
    ) -> EstimatorResult<HashMap<TokenId, TokenPrice>> {
        if tokens.is_empty() {
            return Ok(HashMap::new());
        }

        // Try to reuse prices from in-memory subscriptions
        let mut result = HashMap::new();

        let mut tokens_by_chain: HashMap<ChainId, Vec<String>> = HashMap::new();

        for token in tokens.into_iter() {
            tokens_by_chain
                .entry(token.chain)
                .or_default()
                .push(token.address);
        }

        // For all missing per chain, batch HTTP fetch and fill results
        for (chain, addresses) in tokens_by_chain.into_iter() {
            if addresses.is_empty() {
                continue;
            }

            // Check if we have all tokens in subscriptions, if not, fetch all via HTTP (not just missing ones as it is equivalent cost of rate limit)
            let mut chain_result = HashMap::new();
            let mut were_fetched_on_api = false;
            for token_address in addresses.iter() {
                let key = TokenId::new(chain, token_address.clone());
                match self.subscriptions.get(&key) {
                    Some(entry) => {
                        if let Some(price) = &entry.price {
                            chain_result.insert(key, price.clone());
                        }
                    }
                    None => {
                        // Not found in subscriptions, will need to fetch all via HTTP
                        match gecko_terminal_get_tokens_info(&self.client, chain, addresses.clone())
                            .await
                        {
                            Ok(infos) => {
                                for info in infos {
                                    let token_id = TokenId::new(chain, info.attributes.address);

                                    let price_f = match info.attributes.price_usd.parse::<f64>() {
                                        Ok(v) => v,
                                        Err(e) => {
                                            tracing::error!(
                                                "Failed to parse GeckoTerminal price for {} on {:?}: {:?}",
                                                token_id.address,
                                                chain,
                                                e
                                            );
                                            continue;
                                        }
                                    };
                                    let new_price = TokenPrice {
                                        price: price_f,
                                        decimals: info.attributes.decimals,
                                    };
                                    result.insert(token_id, new_price);
                                    were_fetched_on_api = true;
                                }
                                break;
                            }
                            Err(err) => {
                                tracing::error!(
                                    "GeckoTerminal HTTP error for chain {:?}: {:?}",
                                    chain,
                                    err
                                );
                            }
                        }
                    }
                }
            }
            // If we found all, extend result
            if !were_fetched_on_api {
                result.extend(chain_result);
            }
        }

        Ok(result)
    }

    async fn get_tokens_prices_events(
        &self,
    ) -> EstimatorResult<tokio::sync::broadcast::Receiver<PriceEvent>> {
        Ok(self.subscribe_events())
    }

    async fn subscribe_to_token(&self, token: TokenId) -> EstimatorResult<()> {
        tracing::debug!("Subscribing to token: {:?}", token);
        self.subscriptions
            .entry(token)
            .and_modify(|entry| {
                entry.ref_count += 1;
            })
            .or_insert(GtSubscriptionEntry {
                ref_count: 1,
                price: None,
            });
        Ok(())
    }

    async fn unsubscribe_from_token(&self, token: TokenId) -> EstimatorResult<bool> {
        tracing::debug!("Unsubscribing from token: {:?}", token);

        let mut dropped = false;
        match self.subscriptions.entry(token.clone()) {
            Entry::Occupied(mut occ) => {
                let entry = occ.get_mut();
                if entry.ref_count > 1 {
                    // Decrement ref count
                    entry.ref_count -= 1;
                } else {
                    // Safe remove
                    dropped = true;
                    occ.remove();
                }
            }
            Entry::Vacant(_) => {
                // Nothing to do.
                tracing::debug!(
                    "Unsubscribe called for non-existent subscription: {:?}",
                    token
                );
            }
        }

        Ok(dropped)
    }
}

pub async fn gecko_terminal_get_tokens_info(
    client: &Client,
    chain_id: ChainId,
    tokens_address: Vec<String>,
) -> EstimatorResult<Vec<GeckoTerminalTokensInfo>> {
    let url = format!(
        "{}/networks/{}/tokens/multi/{}",
        GECKO_TERMINAL_API_URL,
        chain_id.to_gecko_terminal_chain_name(),
        tokens_address.join(",")
    );

    let response = client
        .get(&url)
        .send()
        .await
        .change_context(Error::ReqwestError)
        .attach_printable("Error in gecko terminal request")?;

    let tokens_response: GeckoTerminalResponse = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;

    if let GeckoTerminalOkResponseType::TokensInfo(tokens_info) =
        handle_gecko_terminal_response(tokens_response)?
    {
        Ok(tokens_info
            .into_iter()
            .filter_map(
                |v| match serde_json::from_value::<GeckoTerminalTokensInfo>(v) {
                    Ok(info) => Some(info),
                    Err(e) => {
                        tracing::error!("Failed to parse gecko terminal token info: {:?}", e);
                        None
                    }
                },
            )
            .collect())
    } else {
        tracing::error!("Unexpected response in gecko terminal request");
        Err(report!(Error::ResponseError)
            .attach_printable("Unexpected response in gecko terminal request"))
    }
}

fn handle_gecko_terminal_response(
    response: GeckoTerminalResponse,
) -> EstimatorResult<GeckoTerminalOkResponseType> {
    match response {
        GeckoTerminalResponse::Ok(gecko_terminal_ok_response) => {
            Ok(gecko_terminal_ok_response.data)
        }
        GeckoTerminalResponse::Error(gecko_terminal_error_response) => {
            tracing::error!(
                "Error in gecko terminal request: {:?}",
                gecko_terminal_error_response.errors
            );
            Err(report!(Error::ResponseError).attach_printable("Error"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::init_tracing_in_tests;

    #[tokio::test]
    async fn test_gecko_terminal_get_tokens_price() {
        dotenv::dotenv().ok();
        init_tracing_in_tests();

        let gt_provider: GeckoTerminalProvider = GeckoTerminalProvider::new();

        let tokens = HashSet::from([
            TokenId {
                chain: ChainId::Solana,
                address: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263".to_string(),
            },
            TokenId {
                chain: ChainId::Base,
                address: "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string(),
            },
        ]);

        let tokens_info = gt_provider
            .get_tokens_price(tokens, false)
            .await
            .expect("Failed to get tokens price");
        println!("Tokens Info: {:?}", tokens_info);
        // Check that we got data for both tokens
        assert_eq!(tokens_info.len(), 2);
        assert!(tokens_info.contains_key(&TokenId {
            chain: ChainId::Solana,
            address: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263".to_string(),
        }));
        assert!(tokens_info.contains_key(&TokenId {
            chain: ChainId::Base,
            address: "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string(),
        }));
        // Check that the prices are valid
        let sol_token_price = tokens_info
            .get(&TokenId {
                chain: ChainId::Solana,
                address: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263".to_string(),
            })
            .unwrap();
        assert!(sol_token_price.price > 0.0);
        let base_token_price = tokens_info
            .get(&TokenId {
                chain: ChainId::Base,
                address: "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string(),
            })
            .unwrap();
        assert!(base_token_price.price > 0.0);
    }

    #[tokio::test]
    async fn test_gecko_terminal_subscription_broadcast_event() {
        dotenv::dotenv().ok();
        init_tracing_in_tests();

        // Use a short refresh interval to speed up the test
        let gt_provider: GeckoTerminalProvider = GeckoTerminalProvider::new_with_subscriptions(3);

        // Popular token (Solana Bonk)
        let token = TokenId {
            chain: ChainId::Solana,
            address: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263".to_string(),
        };

        // Subscribe to token so the background refresher includes it in the snapshot
        gt_provider
            .subscribe_to_token(token.clone())
            .await
            .expect("subscribe_to_token failed");

        // Subscribe to the broadcast of price events
        let mut rx = gt_provider.subscribe_events();

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

        assert!(
            evt.price.price > 0.0,
            "Expected positive price from GeckoTerminal"
        );

        // Unsubscribe and ensure the entry is removed when ref_count reaches zero
        gt_provider
            .unsubscribe_from_token(token.clone())
            .await
            .expect("unsubscribe_from_token failed");

        assert!(
            gt_provider.subscriptions.get(&token).is_none(),
            "Subscription entry should be removed on unsubscribe"
        );
    }

    #[tokio::test]
    async fn test_gecko_terminal_subscription_and_unsuscription() {
        dotenv::dotenv().ok();
        init_tracing_in_tests();

        // Use a short refresh interval to speed up the test
        let gt_provider: GeckoTerminalProvider = GeckoTerminalProvider::new_with_subscriptions(3);

        // Popular token (Solana Bonk)
        let token = TokenId {
            chain: ChainId::Solana,
            address: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263".to_string(),
        };

        // Subscribe to token so the background refresher includes it in the snapshot
        gt_provider
            .subscribe_to_token(token.clone())
            .await
            .expect("subscribe_to_token failed");

        gt_provider
            .subscribe_to_token(token.clone())
            .await
            .expect("subscribe_to_token failed");

        // Subscribe to the broadcast of price events
        let mut rx = gt_provider.subscribe_events();

        // Unsubscribe once
        gt_provider
            .unsubscribe_from_token(token.clone())
            .await
            .expect("unsubscribe_from_token failed");

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
        gt_provider
            .unsubscribe_from_token(token.clone())
            .await
            .expect("unsubscribe_from_token failed");
    }
}
