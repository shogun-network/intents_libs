use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use intents_models::constants::chains::ChainId;
use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::interval,
};

use crate::{
    error::{Error, EstimatorResult},
    monitoring::messages::{MonitorAlert, MonitorRequest},
    prices::defillama::pricing::{DefiLlamaCoinData, DefiLlamaCoinHashMap, get_tokens_data},
};

#[derive(Debug)]
pub struct MonitorManager {
    pub receiver: Receiver<MonitorRequest>,
    pub sender: Sender<MonitorAlert>,
    pub cache: HashMap<(ChainId, String), DefiLlamaCoinData>,
}

impl MonitorManager {
    pub fn new(receiver: Receiver<MonitorRequest>, sender: Sender<MonitorAlert>) -> Self {
        Self {
            receiver,
            sender,
            cache: HashMap::new(),
        }
    }

    pub async fn run(&mut self) {
        let mut cache_update_interval = interval(Duration::from_secs(15));
        loop {
            tokio::select! {
                    // Periodic cache update every 15 seconds
                    _ = cache_update_interval.tick() => {
                        if !self.cache.is_empty() {
                            tracing::debug!("Performing periodic cache update");
                            if let Err(error) = self.update_cache().await {
                                tracing::error!("Periodic cache update failed: {:?}", error);
                            } else {
                                tracing::debug!("Periodic cache update completed successfully");
                            }
                        } else {
                            tracing::debug!("Cache is empty, skipping periodic update");
                        }
                    }
                    request = self.receiver.recv() => {
                        match request {
                            Some(request) => {
                        tracing::debug!("Received monitor request: {:?}", request);

                        match request {
                            MonitorRequest::GetCoinData {
                                chain,
                                address,
                                resp,
                            } => {
                                // If we have it in our cache, return it
                                if let Some(data) = self.cache.get(&(chain, address.clone())) {
                                    tracing::debug!("Cache hit for {:?}: {:?}", (chain, address), data);
                                    // TODO: Check timestamp, maybe data is old and we no longer get updates
                                    // if data.timestamp ...
                                    match resp.send(Ok(data.clone())) {
                                        Ok(_) => tracing::debug!("Response sent successfully"),
                                        Err(_) => tracing::error!("Failed to send GetCoinData response"),
                                    }
                                } else {
                                    tracing::debug!(
                                        "Cache miss for chain: {}, token {}, fetching...",
                                        chain,
                                        address
                                    );
                                    let mut token_request = HashSet::new();
                                    token_request.insert((chain, address.clone()));
                                    let response = match get_tokens_data(token_request).await {
                                        Ok(data) => {
                                            // Check if we got the data for the requested token
                                            if let Some(coin_data) = data.get((chain, &address)) {
                                                // Update cache and send response
                                                self.cache
                                                    .insert((chain, address.clone()), coin_data.clone());
                                                tracing::debug!(
                                                    "Fetched data for chain: {}, token: {}",
                                                    chain,
                                                    address
                                                );
                                                Ok(coin_data.clone())
                                            } else {
                                                tracing::error!(
                                                    "No data found for chain: {}, token: {}",
                                                    chain,
                                                    address
                                                );
                                                Err(Error::TokenNotFound(
                                                    "No data found for requested token".to_string(),
                                                ))
                                            }
                                        }
                                        Err(error) => {
                                            tracing::error!("Failed to fetch coin data: {:?}", error);
                                            Err(error.current_context().to_owned())
                                        }
                                    };
                                    match resp.send(response) {
                                        Ok(_) => tracing::debug!("Error response sent successfully"),
                                        Err(_) => tracing::error!("Failed to send error response"),
                                    }
                                }
                            }
                        }
                    }
                            None => {
                                tracing::warn!("Monitor request channel closed, exiting...");
                                break;
                            }
                        }
                }
            }
        }
    }

    pub async fn update_cache(&mut self) -> EstimatorResult<()> {
        let request = self.cache.keys().cloned().collect::<HashSet<_>>();
        match get_tokens_data(request.clone()).await {
            Ok(data) => {
                for (chain, address) in request {
                    if let Some(coin_data) = data.get((chain, &address)) {
                        self.cache.insert((chain, address), coin_data.clone());
                    } else {
                        tracing::warn!(
                            "No data found for chain: {}, token: {} during cache update",
                            chain,
                            address
                        );
                    }
                }
                Ok(())
            }
            Err(error) => {
                tracing::error!("Failed to update cache: {:?}", error);
                Err(error)
            }
        }
    }
}
