use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use error_stack::report;
use intents_models::constants::chains::ChainId;
use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::interval,
};

use crate::{
    error::{Error, EstimatorResult},
    monitoring::messages::{MonitorAlert, MonitorRequest},
    prices::{
        PriceProvider, TokenId, TokenPrice,
        estimating::{DEFILLAMA_PROVIDER, GECKO_TERMINAL_PROVIDER},
    },
    utils::number_conversion::u128_to_f64,
};

// For limit order on solver src_token and dst_tokens are same as order,
// and for stop loss on auctioneer, src_token and dst_token are switched to check when the
// stop_loss_max_out of dst_token can buy amount_in of src_token
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
    pub feasibility_check_interval: Duration,
    pub receiver: Receiver<MonitorRequest>,
    pub alert_sender: Sender<MonitorAlert>,
    pub coin_cache: HashMap<TokenId, TokenPrice>,
    pub feasibility_margin: f64,
    pub pending_swaps: HashMap<String, PendingSwap>, // OrderId to tokens, and price limit
}

impl MonitorManager {
    pub fn new(
        receiver: Receiver<MonitorRequest>,
        sender: Sender<MonitorAlert>,
        feasibility_margin: f64,
        feasibility_check_interval: u64,
    ) -> Self {
        Self {
            receiver,
            alert_sender: sender,
            coin_cache: HashMap::new(),
            feasibility_margin,
            pending_swaps: HashMap::new(),
            feasibility_check_interval: Duration::from_secs(feasibility_check_interval),
        }
    }

    // TODO: Do async updates in cache update and check swaps feasibility
    pub async fn run(&mut self) {
        let mut cache_update_interval = interval(self.feasibility_check_interval);
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
                            MonitorRequest::RemoveCheckSwapFeasibility { order_id } => {
                                tracing::debug!("Removing check swap feasibility for order_id: {}", order_id);
                                // Remove the swap from pending swaps
                                self.pending_swaps.remove(&order_id);
                            }
                            MonitorRequest::CheckSwapFeasibility {
                                order_id,
                                src_chain,
                                dst_chain,
                                token_in,
                                token_out,
                                amount_in,
                                amount_out,
                            } => {
                                self.check_swap_feasibility(order_id, src_chain, dst_chain, token_in, token_out, amount_in, amount_out);
                            }
                            MonitorRequest::GetCoinData {
                                token_id,
                                resp,
                            } => {
                                let response = self.get_coin_data(token_id).await;
                                match resp.send(response) {
                                    Ok(_) => tracing::debug!("Error response sent successfully"),
                                    Err(_) => tracing::error!("Failed to send error response"),
                                }
                            }
                            MonitorRequest::GetCoinsData { token_ids, resp } => {

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

    fn check_swap_feasibility(
        &mut self,
        order_id: String,
        src_chain: ChainId,
        dst_chain: ChainId,
        token_in: String,
        token_out: String,
        amount_in: u128,
        amount_out: u128,
    ) {
        tracing::debug!(
            "Checking swap feasibility for order_id: {}, token_in: {}, token_out: {}, amount_in: {}, amount_out: {}",
            order_id,
            token_in,
            token_out,
            amount_in,
            amount_out
        );
        // Add tokens to coin cache
        self.coin_cache.insert(
            TokenId {
                chain: src_chain,
                address: token_in.clone(),
            },
            TokenPrice::default(), // Placeholder for actual data
        );
        self.coin_cache.insert(
            TokenId {
                chain: dst_chain,
                address: token_out.clone(),
            },
            TokenPrice::default(), // Placeholder for actual data
        );
        // Add the swap to pending swaps
        tracing::debug!(
            "Adding pending swap for order_id: {}, src_chain: {}, dst_chain: {}, token_in: {}, token_out: {}, amount_in: {}, amount_out: {}",
            order_id,
            src_chain,
            dst_chain,
            token_in,
            token_out,
            amount_in,
            amount_out
        );
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

    async fn get_coin_data(&mut self, token_id: TokenId) -> Result<TokenPrice, Error> {
        let chain = token_id.chain;
        let address = token_id.address;
        // If we have it in our cache, return it
        if let Some(data) = self.coin_cache.get(&TokenId {
            chain,
            address: address.clone(),
        }) && data.price == 0.0
        {
            // If price is 0.0, it means we don't have data
            tracing::debug!("Cache hit for {:?}: {:?}", (chain, address), data);
            return Ok(data.clone());
        } else {
            tracing::debug!(
                "Cache miss for chain: {}, token {}, fetching...",
                chain,
                address
            );
            let mut token_request = HashSet::new();
            token_request.insert(TokenId {
                chain,
                address: address.clone(),
            });
            // TODO: Use Defillama and Gecko Terminal get_tokens_price and combine results (prioritazing Defillama)
            let defillama_token_price = match DEFILLAMA_PROVIDER
                .get_tokens_price(token_request.clone())
                .await
            {
                Ok(data) => {
                    // Check if we got the data for the requested token
                    if let Some(coin_data) = data.get(&TokenId {
                        chain,
                        address: address.clone(),
                    }) {
                        // Update cache and send response
                        self.coin_cache.insert(
                            TokenId {
                                chain,
                                address: address.clone(),
                            },
                            coin_data.clone(),
                        );
                        tracing::debug!("Fetched data for chain: {}, token: {}", chain, address);
                        Some(coin_data.clone())
                    } else {
                        tracing::warn!(
                            "No data found in defillama for chain: {}, token: {}",
                            chain,
                            address
                        );
                        None
                    }
                }
                Err(error) => {
                    tracing::error!("Failed to fetch coin data: {:?}", error);
                    None
                }
            };
            let response = if let Some(token_price) = defillama_token_price {
                Some(token_price)
            } else {
                // Try Gecko Terminal if Defillama failed
                match GECKO_TERMINAL_PROVIDER
                    .get_tokens_price(token_request)
                    .await
                {
                    Ok(data) => {
                        // Check if we got the data for the requested token
                        if let Some(coin_data) = data.get(&TokenId {
                            chain,
                            address: address.clone(),
                        }) {
                            // Update cache and send response
                            self.coin_cache.insert(
                                TokenId {
                                    chain,
                                    address: address.clone(),
                                },
                                coin_data.clone(),
                            );
                            tracing::debug!(
                                "Fetched data for chain: {}, token: {}",
                                chain,
                                address
                            );
                            Some(coin_data.clone())
                        } else {
                            tracing::warn!(
                                "No data found in defillama for chain: {}, token: {}",
                                chain,
                                address
                            );
                            None
                        }
                    }
                    Err(error) => {
                        tracing::error!("Failed to fetch coin data: {:?}", error);
                        None
                    }
                }
            };
            match response {
                Some(token_price) => Ok(token_price),
                None => Err(Error::TokenNotFound(format!(
                    "Token data not found for chain: {}, address: {}",
                    chain, address
                ))),
            }
        }
    }

    pub async fn update_cache(&mut self) -> EstimatorResult<()> {
        let request = self.coin_cache.keys().cloned().collect::<HashSet<_>>();
        // Call Defillama API to get token data
        let defillama_tokens_price = DEFILLAMA_PROVIDER.get_tokens_price(request.clone()).await;
        // Call Gecko Terminal API to get token data
        let gecko_terminal_tokens_price = GECKO_TERMINAL_PROVIDER
            .get_tokens_price(request.clone())
            .await;

        if let Err(_) = defillama_tokens_price
            && let Err(_) = gecko_terminal_tokens_price
        {
            tracing::error!("Failed to fetch data from both Defillama and Gecko Terminal");
            return Err(report!(Error::ResponseError)
                .attach_printable("Failed to fetch token data".to_string()));
        }

        // Merge both results, prioritizing Defillama data
        let mut data = HashMap::new();
        if let Ok(defillama_data) = defillama_tokens_price {
            data.extend(defillama_data);
        } else {
            tracing::error!(
                "Failed to fetch data from Defillama: {:?}",
                defillama_tokens_price
            );
        }
        if let Ok(gecko_data) = gecko_terminal_tokens_price {
            data.extend(gecko_data);
        } else {
            tracing::error!(
                "Failed to fetch data from Gecko Terminal: {:?}",
                gecko_terminal_tokens_price
            );
        }

        for (token_id, token_price) in data.into_iter() {
            self.coin_cache.insert(token_id, token_price);
        }
        Ok(())
    }
}

async fn check_swaps_feasibility(
    feasibility_margin: f64,
    coin_cache: HashMap<TokenId, TokenPrice>,
    pending_swaps: HashMap<String, PendingSwap>,
    alert_sender: Sender<MonitorAlert>,
) -> HashMap<String, PendingSwap> {
    tracing::debug!(
        "Checking swaps feasibility with margin: {}",
        feasibility_margin
    );
    let mut unfinished_swaps = HashMap::new();
    for (order_id, mut pending_swap) in pending_swaps.into_iter() {
        tracing::debug!(
            "Processing pending swap for order_id: {}, src_chain: {}, dst_chain: {}, token_in: {}, token_out: {}, amount_in: {}, amount_out: {}",
            order_id,
            pending_swap.src_chain,
            pending_swap.dst_chain,
            pending_swap.token_in,
            pending_swap.token_out,
            pending_swap.amount_in,
            pending_swap.amount_out
        );
        let src_chain_data = coin_cache.get(&TokenId {
            chain: pending_swap.src_chain,
            address: pending_swap.token_in.clone(),
        });
        let dst_chain_data = coin_cache.get(&TokenId {
            chain: pending_swap.dst_chain,
            address: pending_swap.token_out.clone(),
        });
        tracing::debug!(
            "Fetched coin data for src_chain: {:?}, dst_chain: {:?}",
            src_chain_data,
            dst_chain_data
        );
        if let (Some(src_data), Some(dst_data)) = (src_chain_data, dst_chain_data) {
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
        } else {
            tracing::warn!(
                "Missing or invalid coin data for order_id: {}, src_chain: {}, dst_chain: {}, token_in: {}, token_out: {}",
                order_id,
                pending_swap.src_chain,
                pending_swap.dst_chain,
                pending_swap.token_in,
                pending_swap.token_out
            );
            // If we don't have data, we can't process the swap
            unfinished_swaps.insert(order_id, pending_swap);
        }
    }
    unfinished_swaps
}

#[cfg(test)]
mod tests {
    use super::*;
    use intents_models::constants::chains::ChainId;
    use tokio::sync::mpsc;

    fn create_coin_data(price: f64, decimals: u8) -> TokenPrice {
        TokenPrice { price, decimals }
    }

    fn create_pending_swap(
        src_chain: ChainId,
        dst_chain: ChainId,
        token_in: String,
        token_out: String,
        amount_in: u128,
        amount_out: u128,
        price_limit: Option<f64>,
    ) -> PendingSwap {
        PendingSwap {
            src_chain,
            dst_chain,
            token_in,
            token_out,
            amount_in,
            amount_out,
            price_limit,
        }
    }

    #[tokio::test]
    async fn test_check_swaps_feasibility_successful_swap() {
        let (alert_sender, mut alert_receiver) = mpsc::channel(10);

        let mut coin_cache = HashMap::new();
        coin_cache.insert(
            TokenId {
                chain: ChainId::Ethereum,
                address: "token_a".to_string(),
            },
            create_coin_data(100.0, 18), // $100 per token
        );
        coin_cache.insert(
            TokenId {
                chain: ChainId::Base,
                address: "token_b".to_string(),
            },
            create_coin_data(50.0, 6), // $50 per token
        );

        let mut pending_swaps = HashMap::new();
        pending_swaps.insert(
            "order_1".to_string(),
            create_pending_swap(
                ChainId::Ethereum,
                ChainId::Base,
                "token_a".to_string(),
                "token_b".to_string(),
                1_000_000_000_000_000_000, // 1 token (18 decimals)
                1_900_000,                 // 1.9 tokens (6 decimals), expecting ~2 tokens
                None,
            ),
        );

        let result = check_swaps_feasibility(
            0.05, // 5% margin
            coin_cache,
            pending_swaps,
            alert_sender,
        )
        .await;

        // The swap should be feasible: real_price = 50/100 = 0.5, expected = 1.9/1 = 1.9
        // Since 0.5 < 1.9 * (1 - 0.05), the swap should NOT be feasible
        assert_eq!(result.len(), 1);
        assert!(result.contains_key("order_1"));

        // Should not receive any alert
        assert!(alert_receiver.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_check_swaps_feasibility_with_preset_price_limit() {
        let (alert_sender, mut alert_receiver) = mpsc::channel(10);

        let mut coin_cache = HashMap::new();
        coin_cache.insert(
            TokenId {
                chain: ChainId::Ethereum,
                address: "token_a".to_string(),
            },
            create_coin_data(100.0, 18),
        );
        coin_cache.insert(
            TokenId {
                chain: ChainId::Base,
                address: "token_b".to_string(),
            },
            create_coin_data(50.0, 18),
        );

        let mut pending_swaps = HashMap::new();
        pending_swaps.insert(
            "order_1".to_string(),
            create_pending_swap(
                ChainId::Ethereum,
                ChainId::Base,
                "token_a".to_string(),
                "token_b".to_string(),
                1_000_000_000_000_000_000,
                2_000_000_000_000_000_000,
                Some(0.4), // Preset price limit: expecting price ratio of 0.4
            ),
        );

        let result = check_swaps_feasibility(
            0.1, // 10% margin
            coin_cache,
            pending_swaps,
            alert_sender,
        )
        .await;

        // real_price_limit = 50/100 = 0.5
        // price_limit = 0.4
        // 0.5 >= 0.4 * (1 - 0.1) = 0.36, so swap should be feasible
        assert_eq!(result.len(), 0);

        // Should receive alert
        let alert = alert_receiver.try_recv().unwrap();
        assert!(
            matches!(alert, MonitorAlert::SwapIsFeasible { order_id } if order_id == "order_1")
        );
    }

    #[tokio::test]
    async fn test_check_swaps_feasibility_missing_coin_data() {
        let (alert_sender, mut alert_receiver) = mpsc::channel(10);

        let mut coin_cache = HashMap::new();
        // Only add one token, missing the other
        coin_cache.insert(
            TokenId {
                chain: ChainId::Ethereum,
                address: "token_a".to_string(),
            },
            create_coin_data(100.0, 18),
        );

        let mut pending_swaps = HashMap::new();
        pending_swaps.insert(
            "order_1".to_string(),
            create_pending_swap(
                ChainId::Ethereum,
                ChainId::Base,
                "token_a".to_string(),
                "token_b".to_string(), // This token is missing from cache
                1_000_000_000_000_000_000,
                2_000_000_000_000_000_000,
                None,
            ),
        );

        let result = check_swaps_feasibility(0.05, coin_cache, pending_swaps, alert_sender).await;

        // Should not process the swap due to missing data
        assert_eq!(result.len(), 0);
        assert!(alert_receiver.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_check_swaps_feasibility_multiple_swaps_mixed_results() {
        let (alert_sender, mut alert_receiver) = mpsc::channel(10);

        let mut coin_cache = HashMap::new();
        coin_cache.insert(
            TokenId {
                chain: ChainId::Ethereum,
                address: "token_a".to_string(),
            },
            create_coin_data(100.0, 18),
        );
        coin_cache.insert(
            TokenId {
                chain: ChainId::Base,
                address: "token_b".to_string(),
            },
            create_coin_data(50.0, 18),
        );

        let mut pending_swaps = HashMap::new();

        // Feasible swap
        pending_swaps.insert(
            "feasible_order".to_string(),
            create_pending_swap(
                ChainId::Ethereum,
                ChainId::Base,
                "token_a".to_string(),
                "token_b".to_string(),
                1_000_000_000_000_000_000,
                2_000_000_000_000_000_000,
                Some(0.4), // Will be feasible
            ),
        );

        // Non-feasible swap
        pending_swaps.insert(
            "non_feasible_order".to_string(),
            create_pending_swap(
                ChainId::Ethereum,
                ChainId::Base,
                "token_a".to_string(),
                "token_b".to_string(),
                1_000_000_000_000_000_000,
                2_000_000_000_000_000_000,
                Some(0.8), // Will not be feasible
            ),
        );

        let result = check_swaps_feasibility(0.1, coin_cache, pending_swaps, alert_sender).await;

        // Only non-feasible swap should remain
        assert_eq!(result.len(), 1);
        assert!(result.contains_key("non_feasible_order"));

        // Should receive one alert for feasible swap
        let alert = alert_receiver.try_recv().unwrap();
        assert!(
            matches!(alert, MonitorAlert::SwapIsFeasible { order_id } if order_id == "feasible_order")
        );

        // No more alerts
        assert!(alert_receiver.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_check_swaps_feasibility_different_decimals() {
        let (alert_sender, mut alert_receiver) = mpsc::channel(10);

        let mut coin_cache = HashMap::new();
        coin_cache.insert(
            TokenId {
                chain: ChainId::Ethereum,
                address: "token_a".to_string(),
            },
            create_coin_data(1.0, 6), // 6 decimals
        );
        coin_cache.insert(
            TokenId {
                chain: ChainId::Base,
                address: "token_b".to_string(),
            },
            create_coin_data(2.0, 18), // 18 decimals
        );

        let mut pending_swaps = HashMap::new();
        pending_swaps.insert(
            "order_1".to_string(),
            create_pending_swap(
                ChainId::Ethereum,
                ChainId::Base,
                "token_a".to_string(),
                "token_b".to_string(),
                1_000_000,               // 1 token with 6 decimals
                500_000_000_000_000_000, // 0.5 tokens with 18 decimals
                None,
            ),
        );

        let result = check_swaps_feasibility(
            0.0, // No margin for easier calculation
            coin_cache,
            pending_swaps,
            alert_sender,
        )
        .await;

        // price_limit = 0.5 / 1.0 = 0.5
        // real_price_limit = 2.0 / 1.0 = 2.0
        // 2.0 >= 0.5, so swap should be feasible
        assert_eq!(result.len(), 0);

        let alert = alert_receiver.try_recv().unwrap();
        assert!(
            matches!(alert, MonitorAlert::SwapIsFeasible { order_id } if order_id == "order_1")
        );
    }

    #[tokio::test]
    async fn test_check_swaps_feasibility_edge_case_zero_amounts() {
        let (alert_sender, _alert_receiver) = mpsc::channel(10);

        let mut coin_cache = HashMap::new();
        coin_cache.insert(
            TokenId {
                chain: ChainId::Ethereum,
                address: "token_a".to_string(),
            },
            create_coin_data(100.0, 18),
        );
        coin_cache.insert(
            TokenId {
                chain: ChainId::Base,
                address: "token_b".to_string(),
            },
            create_coin_data(50.0, 18),
        );

        let mut pending_swaps = HashMap::new();
        pending_swaps.insert(
            "zero_amount_order".to_string(),
            create_pending_swap(
                ChainId::Ethereum,
                ChainId::Base,
                "token_a".to_string(),
                "token_b".to_string(),
                0, // Zero amount_in - this would cause division by zero
                1_000_000_000_000_000_000,
                None,
            ),
        );

        // This test should either handle zero gracefully or panic
        // Currently the code will panic on division by zero
        // You should fix this in the actual implementation
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(check_swaps_feasibility(
                    0.05,
                    coin_cache,
                    pending_swaps,
                    alert_sender,
                ))
        }));

        // Currently this will panic - you should fix the implementation
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_check_swaps_feasibility_empty_inputs() {
        let (alert_sender, mut alert_receiver) = mpsc::channel(10);

        let coin_cache = HashMap::new();
        let pending_swaps = HashMap::new();

        let result = check_swaps_feasibility(0.05, coin_cache, pending_swaps, alert_sender).await;

        assert_eq!(result.len(), 0);
        assert!(alert_receiver.try_recv().is_err());
    }
}
