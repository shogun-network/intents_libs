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
    utils::number_conversion::u128_to_f64,
};

#[derive(Debug, Clone)]
pub struct PendingSwap {
    pub src_chain: ChainId,
    pub dst_chain: ChainId,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: u128,
    pub amount_out: u128,
    pub price_limit: Option<f64>,
}

#[derive(Debug)]
pub struct MonitorManager {
    pub receiver: Receiver<MonitorRequest>,
    pub alert_sender: Sender<MonitorAlert>,
    pub coin_cache: HashMap<(ChainId, String), DefiLlamaCoinData>,
    pub feasibility_margin: f64,
    pub pending_swaps: HashMap<String, PendingSwap>, // OrderId to tokens, and price limit
}

impl MonitorManager {
    pub fn new(
        receiver: Receiver<MonitorRequest>,
        sender: Sender<MonitorAlert>,
        feasibility_margin: f64,
    ) -> Self {
        Self {
            receiver,
            alert_sender: sender,
            coin_cache: HashMap::new(),
            feasibility_margin,
            pending_swaps: HashMap::new(),
        }
    }

    // TODO: Do async updates in cache update and check swaps feasibility
    pub async fn run(&mut self) {
        let mut cache_update_interval = interval(Duration::from_secs(15));
        loop {
            tokio::select! {
                    // Periodic cache update every 15 seconds
                    _ = cache_update_interval.tick() => {
                        if !self.coin_cache.is_empty() {
                            tracing::debug!("Performing periodic cache update");
                            if let Err(error) = self.update_cache().await {
                                tracing::error!("Periodic cache update failed: {:?}", error);
                            } else {
                                tracing::debug!("Periodic cache update completed successfully");
                            }
                        } else {
                            tracing::debug!("Cache is empty, skipping periodic update");
                        }
                        // Check swaps feasibility
                        self.pending_swaps = check_swaps_feasibility(
                            self.feasibility_margin,
                            self.coin_cache.clone(),
                            self.pending_swaps.clone(),
                            self.alert_sender.clone(),
                        ).await;
                    }
                    request = self.receiver.recv() => {
                        match request {
                            Some(request) => {
                        tracing::debug!("Received monitor request: {:?}", request);

                        match request {
                            MonitorRequest::CheckSwapFeasibility {
                                order_id,
                                src_chain,
                                dst_chain,
                                token_in,
                                token_out,
                                amount_in,
                                amount_out,
                            } => {
                                tracing::debug!(
                                    "Checking swap feasibility for order_id: {}, token_in: {}, token_out: {}, amount_in: {}, amount_out: {}",
                                    order_id, token_in, token_out, amount_in, amount_out
                                );
                                // Add tokens to coin cache
                                self.coin_cache.insert(
                                    (src_chain, token_in.clone()),
                                    DefiLlamaCoinData::default(), // Placeholder for actual data
                                );
                                // Add the swap to pending swaps
                                self.pending_swaps.insert(
                                    order_id.clone(),
                                    PendingSwap {
                                        src_chain,
                                        dst_chain,
                                        token_in,
                                        token_out,
                                        amount_in,
                                        amount_out,
                                        price_limit: None, // No price limit set initially
                                    },
                                );
                            }
                            MonitorRequest::GetCoinData {
                                chain,
                                address,
                                resp,
                            } => {
                                // If we have it in our cache, return it
                                if let Some(data) = self.coin_cache.get(&(chain, address.clone())) && data.timestamp != 0 { // If timestamp is 0, it means we don't have data
                                    tracing::debug!("Cache hit for {:?}: {:?}", (chain, address), data);
                                    // TODO: Check timestamp, maybe data is old and we no longer get updates
                                    // if data.timestamp < ...
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
                                                self.coin_cache
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
        let request = self.coin_cache.keys().cloned().collect::<HashSet<_>>();
        match get_tokens_data(request.clone()).await {
            Ok(data) => {
                for (chain, address) in request {
                    if let Some(coin_data) = data.get((chain, &address)) {
                        self.coin_cache.insert((chain, address), coin_data.clone());
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

async fn check_swaps_feasibility(
    feasibility_margin: f64,
    coin_cache: HashMap<(ChainId, String), DefiLlamaCoinData>,
    pending_swaps: HashMap<String, PendingSwap>,
    alert_sender: Sender<MonitorAlert>,
) -> HashMap<String, PendingSwap> {
    let mut unfinished_swaps = HashMap::new();
    for (order_id, mut pending_swap) in pending_swaps.into_iter() {
        let src_chain_data =
            coin_cache.get(&(pending_swap.src_chain, pending_swap.token_in.clone()));
        let dst_chain_data =
            coin_cache.get(&(pending_swap.dst_chain, pending_swap.token_out.clone()));
        if let (Some(src_data), Some(dst_data)) = (src_chain_data, dst_chain_data)
            && src_data.timestamp != 0
            && dst_data.timestamp != 0
        {
            let price_limit = if let Some(price_limit) = pending_swap.price_limit {
                price_limit
            } else {
                // Calculate price limit based on amounts and token decimals
                let amount_in_dec = u128_to_f64(pending_swap.amount_in, src_data.decimals);
                let amount_out_dec = u128_to_f64(pending_swap.amount_out, dst_data.decimals);
                let price_limit = amount_out_dec / amount_in_dec;
                pending_swap.price_limit = Some(price_limit);
                price_limit
            };

            // Check real price_limit
            let real_price_limit = dst_data.price / src_data.price;
            if real_price_limit >= price_limit * (1.0 - feasibility_margin) {
                tracing::debug!("Swap feasibility check passed for order_id: {}", order_id);
                // Send alert
                if let Err(e) = alert_sender
                    .send(MonitorAlert::SwapIsFeasible {
                        order_id: order_id.clone(),
                    })
                    .await
                {
                    tracing::error!("Failed to send alert for order_id {}: {:?}", order_id, e);
                }
            } else {
                tracing::debug!(
                    "Swap feasibility check failed for order_id: {}, real_price_limit: {}, price_limit: {}",
                    order_id,
                    real_price_limit,
                    price_limit
                );
                unfinished_swaps.insert(order_id, pending_swap);
            }
        }
    }
    unfinished_swaps
}
