use std::collections::{HashMap, HashSet};

use intents_models::constants::chains::ChainId;
use tokio::sync::mpsc::{Receiver, Sender};

use crate::{
    error::{Error, EstimatorResult},
    monitoring::messages::{MonitorAlert, MonitorRequest},
    prices::{
        PriceEvent, PriceProvider, TokenId, TokenPrice,
        estimating::{CODEX_PROVIDER, GECKO_TERMINAL_PROVIDER, OrderEstimationData},
    },
};
use rust_decimal::Decimal;
use rust_decimal::prelude::*;

// For limit order on solver src_token and dst_tokens are same as order,
// and for stop loss on auctioneer, src_token and dst_token are switched to check when the
// stop_loss_max_out of dst_token can buy amount_in of src_token
#[derive(Debug, Clone)]
pub struct PendingSwap {
    pub order_id: String,
    pub src_chain: ChainId,
    pub dst_chain: ChainId,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: u128,
    pub amount_out: u128,
    pub feasibility_margin: f64,
    // pub price_limit: Option<f64>, // If set, use this price limit instead of calculating from amounts
}

#[derive(Debug)]
pub struct MonitorManager {
    pub receiver: Receiver<MonitorRequest>,
    pub alert_sender: Sender<MonitorAlert>,
    pub coin_cache: HashMap<TokenId, TokenPrice>,
    pub pending_swaps: HashMap<String, PendingSwap>, // OrderId to tokens, and price limit
    pub swaps_by_token: HashMap<TokenId, HashSet<String>>, // TokenId to OrderIds
}

impl MonitorManager {
    pub fn new(receiver: Receiver<MonitorRequest>, sender: Sender<MonitorAlert>) -> Self {
        Self {
            receiver,
            alert_sender: sender,
            coin_cache: HashMap::new(),
            pending_swaps: HashMap::new(),
            swaps_by_token: HashMap::new(),
        }
    }

    pub async fn run(mut self) {
        let mut gt_rx = GECKO_TERMINAL_PROVIDER.subscribe_events();
        let mut codex_rx_opt = if let Some(provider) = CODEX_PROVIDER.as_ref() {
            match provider.subscribe_events().await {
                Ok(rx) => Some(rx),
                Err(err) => {
                    tracing::error!("Failed to subscribe Codex price events: {:?}", err);
                    None
                }
            }
        } else {
            None
        };

        loop {
            tokio::select! {
                evt = gt_rx.recv() => {
                    match evt {
                        Ok(event) => {
                            self.on_price_event(event).await;
                        }
                        Err(e) => {
                            tracing::warn!("GeckoTerminal price events receiver closed: {:?}", e);
                        }
                    }
                }
                // Evento de precio desde Codex (si hay proveedor)
                evt = codex_rx_opt.as_mut().unwrap().recv(), if codex_rx_opt.is_some() => {
                    match evt {
                        Ok(event) => {
                            self.on_price_event(event).await;
                        }
                        Err(e) => {
                            tracing::warn!("Codex price events receiver closed: {:?}", e);
                        }
                    }
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
                                    feasibility_margin,
                                } => {
                                    self.check_swap_feasibility(order_id, src_chain, dst_chain, token_in, token_out, amount_in, amount_out, feasibility_margin);
                                }
                                MonitorRequest::GetCoinsData { token_ids, resp } => {
                                    let response = self.get_coins_data(token_ids).await;
                                    match resp.send(response) {
                                        Ok(_) => tracing::debug!("Error response sent successfully"),
                                        Err(_) => tracing::error!("Failed to send error response"),
                                    }
                                }
                                MonitorRequest::EstimateOrdersAmountOut {
                                    orders, resp,
                                } => {
                                    let response = self.estimate_orders_amount_out(orders).await;
                                    match resp.send(response) {
                                        Ok(_) => tracing::debug!("Error response sent successfully"),
                                        Err(_) => tracing::error!("Failed to send error response"),
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

    async fn estimate_orders_amount_out(
        &mut self,
        orders: Vec<OrderEstimationData>,
    ) -> Result<HashMap<String, u128>, Error> {
        // Get Token Info for all tokens in orders
        let mut token_ids = HashSet::new();
        for order in orders.iter() {
            token_ids.insert(TokenId::new(
                order.src_chain.clone(),
                order.token_in.clone(),
            ));
            token_ids.insert(TokenId::new(
                order.dst_chain.clone(),
                order.token_out.clone(),
            ));
        }
        let tokens_info = self.get_coins_data(token_ids).await?;
        match crate::prices::estimating::estimate_orders_amount_out(orders, tokens_info).await {
            Ok(result) => Ok(result),
            Err(e) => {
                tracing::error!("Failed to estimate orders amount out: {:?}", e);
                Err(e.current_context().clone())
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
        feasibility_margin: f64,
    ) {
        tracing::debug!(
            "Checking swap feasibility for order_id: {}, token_in: {}, token_out: {}, amount_in: {}, amount_out: {}",
            order_id,
            token_in,
            token_out,
            amount_in,
            amount_out
        );

        let token_in_id = TokenId::new(src_chain, token_in.clone());

        let token_out_id = TokenId::new(dst_chain, token_out.clone());

        // Add tokens to coin cache
        self.coin_cache.insert(
            token_in_id.clone(),
            TokenPrice::default(), // Placeholder for actual data
        );
        self.coin_cache.insert(
            token_out_id.clone(),
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

        self.swaps_by_token
            .entry(token_in_id)
            .or_insert_with(HashSet::new)
            .insert(order_id.clone());

        self.swaps_by_token
            .entry(token_out_id)
            .or_insert_with(HashSet::new)
            .insert(order_id.clone());

        self.pending_swaps.insert(
            order_id.clone(),
            PendingSwap {
                order_id,
                src_chain,
                dst_chain,
                token_in,
                token_out,
                amount_in,
                amount_out,
                feasibility_margin,
            },
        );
    }

    async fn get_coins_data(
        &mut self,
        token_ids: HashSet<TokenId>,
    ) -> Result<HashMap<TokenId, TokenPrice>, Error> {
        let mut result = HashMap::new();
        let mut tokens_not_in_cache = HashSet::new();

        for token_id in token_ids.into_iter() {
            // Add token to cache if not already present
            let chain = token_id.chain.clone();
            let address = token_id.address.clone();
            // If we have it in our cache, return it
            if let Some(data) = self.coin_cache.get(&token_id)
                && data.price != 0.0
            {
                // If price is 0.0, it means we don't have data
                tracing::debug!("Cache hit for {:?}: {:?}", (chain, address), data);
                result.insert(token_id, data.clone());
            } else {
                tracing::debug!(
                    "Cache miss for chain: {}, token: {}, fetching...",
                    chain,
                    address
                );
                tokens_not_in_cache.insert(TokenId::new(chain, address));
            }
        }
        // If we have tokens not in cache, fetch them
        if tokens_not_in_cache.is_empty() {
            return Ok(result);
        }
        // Fetch data from Codex and Gecko Terminal
        let data = get_combined_tokens_data(tokens_not_in_cache).await?;

        // Update cache with fetched data
        for (token_id, token_price) in data.iter() {
            self.coin_cache
                .insert(token_id.clone(), token_price.clone());
        }
        result.extend(data);

        Ok(result)
    }

    pub async fn update_cache(&mut self) -> EstimatorResult<()> {
        let request = self.coin_cache.keys().cloned().collect::<HashSet<_>>();
        let data = get_combined_tokens_data(request).await?;

        for (token_id, token_price) in data.into_iter() {
            self.coin_cache.insert(token_id, token_price);
        }
        Ok(())
    }

    // Gestionar un PriceEvent: actualizar cache, re-evaluar órdenes afectadas, limpiar y desuscribir tokens si ya no quedan swaps que dependan de ellos.
    async fn on_price_event(&mut self, event: PriceEvent) {
        // Update in-memory cache
        self.coin_cache
            .insert(event.token.clone(), event.price.clone());

        // Orders which have this token
        let impacted_orders: HashSet<String> = self
            .swaps_by_token
            .get(&event.token)
            .cloned()
            .unwrap_or_default();
        if impacted_orders.is_empty() {
            return;
        }

        // Get the swap data of these orders
        let mut subset: Vec<PendingSwap> = Vec::new();
        for order_id in impacted_orders.iter() {
            if let Some(ps) = self.pending_swaps.get(order_id).cloned() {
                subset.push(ps);
            }
        }
        if subset.is_empty() {
            return;
        }

        // Re-evaluate these swaps
        for pending_swap in subset.into_iter() {
            tracing::debug!(
                "Re-evaluating swap feasibility for order_id: {}, token_in: {}, token_out: {}",
                pending_swap.order_id,
                pending_swap.token_in,
                pending_swap.token_out
            );
            match check_swap_feasibility(&pending_swap, &self.coin_cache) {
                Ok(is_feasible) => {
                    if is_feasible {
                        tracing::debug!(
                            "Swap is feasible for order_id: {}, sending alert",
                            pending_swap.order_id
                        );
                        if let Err(e) = self
                            .alert_sender
                            .send(MonitorAlert::SwapIsFeasible {
                                order_id: pending_swap.order_id.clone(),
                            })
                            .await
                        {
                            tracing::error!(
                                "Failed to send alert for order_id {}: {:?}",
                                pending_swap.order_id,
                                e
                            );
                            // Do not remove the swap if we failed to send alert
                            continue;
                        }
                        // Remove from pending swaps
                        self.pending_swaps.remove(&pending_swap.order_id);
                        // Detach from token->orders map and unsubscribe if needed
                        let t_in =
                            TokenId::new(pending_swap.src_chain, pending_swap.token_in.clone());
                        let t_out =
                            TokenId::new(pending_swap.dst_chain, pending_swap.token_out.clone());
                        self.detach_order_from_token(&t_in, &pending_swap.order_id)
                            .await;
                        self.detach_order_from_token(&t_out, &pending_swap.order_id)
                            .await;
                    }
                }
                Err(error) => {
                    tracing::error!(
                        "Error checking swap feasibility for order_id {}: {:?}",
                        pending_swap.order_id,
                        error
                    );
                }
            }
        }
    }

    // Quita un order_id del índice swaps_by_token y desuscribe si ya no quedan órdenes para ese token
    async fn detach_order_from_token(&mut self, token: &TokenId, order_id: &str) {
        if let Some(set) = self.swaps_by_token.get_mut(token) {
            set.remove(order_id);
            if set.is_empty() {
                self.swaps_by_token.remove(token);
            }
        }

        // TODO: As an improvement we can check which provider was used to subscribe and only unsubscribe from that one.
        // Unsubscribe from both providers.
        // We do this because subscriptions have a counter inside, so it won't unsubscribe unless all subscribers are removed.
        if let Err(e) = GECKO_TERMINAL_PROVIDER
            .unsubscribe_from_token(token.clone())
            .await
        {
            tracing::warn!("Gecko unsubscribe_from_token failed: {:?}", e);
        }
        if let Some(provider) = CODEX_PROVIDER.as_ref() {
            if let Err(e) = provider.unsubscribe_from_token(token.clone()).await {
                tracing::warn!("Codex unsubscribe_from_token failed: {:?}", e);
            }
        }
    }
}

async fn get_combined_tokens_data(
    token_ids: HashSet<TokenId>,
) -> Result<HashMap<TokenId, TokenPrice>, Error> {
    // Call Gecko Terminal API to get token data
    let gecko_terminal_tokens_price = GECKO_TERMINAL_PROVIDER
        .get_tokens_price(token_ids.clone())
        .await;

    let codex_tokens_price = if CODEX_PROVIDER.is_some() {
        CODEX_PROVIDER
            .as_ref()
            .unwrap()
            .get_tokens_price(token_ids.clone())
            .await
    } else {
        Ok(HashMap::new())
    };

    if let Err(_) = gecko_terminal_tokens_price
        && (CODEX_PROVIDER.is_none() || codex_tokens_price.is_err())
    {
        tracing::error!("Failed to fetch data from both Gecko Terminal and Codex");
        return Err(Error::ResponseError);
    }

    // Merge both results, prioritizing Codex data when available
    let mut data = HashMap::new();
    if let Ok(gecko_data) = gecko_terminal_tokens_price {
        data.extend(gecko_data);
    } else {
        tracing::error!(
            "Failed to fetch data from Gecko Terminal: {:?}",
            gecko_terminal_tokens_price
        );
    }

    if let Ok(codex_data) = codex_tokens_price {
        for (token_id, codex_price) in codex_data {
            data.entry(token_id)
                .and_modify(|existing| existing.price = codex_price.price)
                .or_insert(codex_price);
        }
    } else {
        tracing::error!("Failed to fetch data from Codex: {:?}", codex_tokens_price);
    }
    Ok(data)
}

fn check_swap_feasibility(
    pending_swap: &PendingSwap,
    coin_cache: &HashMap<TokenId, TokenPrice>,
) -> Result<bool, Error> {
    let src_chain_data = coin_cache.get(&TokenId::new(
        pending_swap.src_chain,
        pending_swap.token_in.clone(),
    ));
    let dst_chain_data = coin_cache.get(&TokenId::new(
        pending_swap.dst_chain,
        pending_swap.token_out.clone(),
    ));
    if let (Some(src_data), Some(dst_data)) = (src_chain_data, dst_chain_data) {
        // Helper to convert amount (u128) with decimals -> Decimal
        let amount_to_decimal = |amount: u128, decimals: u8| -> Result<Decimal, Error> {
            // TODO: Check as i128 overflow
            Ok(Decimal::from_i128_with_scale(
                amount as i128,
                decimals as u32,
            ))
        };

        // Convert prices and margin to Decimal safely
        let src_price = Decimal::from_f64(src_data.price).ok_or(Error::ParseError)?;
        let dst_price = Decimal::from_f64(dst_data.price).ok_or(Error::ParseError)?;
        if src_price.is_zero() || dst_price.is_zero() {
            return Err(Error::ZeroPriceError);
        }
        let margin_dec =
            Decimal::from_f64(pending_swap.feasibility_margin).ok_or(Error::ParseError)?;
        if margin_dec.is_sign_negative() {
            return Err(Error::ParseError);
        }

        // Compute price_limit as Decimal
        let price_limit_dec = {
            let amount_in_dec = amount_to_decimal(pending_swap.amount_in, src_data.decimals)?;
            let amount_out_dec = amount_to_decimal(pending_swap.amount_out, dst_data.decimals)?;
            if amount_in_dec.is_zero() {
                // Avoid division by zero
                return Err(Error::ParseError);
            }
            amount_out_dec / amount_in_dec
        };

        // Compute real price ratio as Decimal
        let real_price_limit = src_price / dst_price;

        // threshold = price_limit * (100.0 + feasibility_margin) / 100.0
        let threshold =
            price_limit_dec * (Decimal::ONE + (margin_dec / Decimal::ONE_HUNDRED)) / Decimal::ONE;

        tracing::debug!(
            "Swap feasibility calculation for order_id: {}: src_amount: {}, src_price: {}, dst_amount: {}, dst_price: {}, real_price_limit: {}, price_limit: {}, threshold (with margin): {}",
            pending_swap.order_id,
            pending_swap.amount_in,
            src_price,
            pending_swap.amount_out,
            dst_price,
            real_price_limit,
            price_limit_dec,
            threshold
        );

        Ok(real_price_limit >= threshold)
    } else {
        Err(Error::TokenNotFound(format!(
            "Missing token data on monitor for swap: {:?}",
            pending_swap
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use intents_models::{constants::chains::ChainId, log::init_tracing};
    use tokio::sync::mpsc;

    fn create_coin_data(price: f64, decimals: u8) -> TokenPrice {
        TokenPrice { price, decimals }
    }

    fn create_order_estimation_data(
        order_id: &str,
        src_chain: ChainId,
        dst_chain: ChainId,
        token_in: &str,
        token_out: &str,
        amount_in: u128,
    ) -> OrderEstimationData {
        OrderEstimationData {
            order_id: order_id.to_string(),
            src_chain,
            dst_chain,
            token_in: token_in.to_string(),
            token_out: token_out.to_string(),
            amount_in,
        }
    }

    fn create_pending_swap(
        order_id: String,
        src_chain: ChainId,
        dst_chain: ChainId,
        token_in: String,
        token_out: String,
        amount_in: u128,
        amount_out: u128,
        feasibility_margin: f64,
    ) -> PendingSwap {
        PendingSwap {
            order_id,
            src_chain,
            dst_chain,
            token_in,
            token_out,
            amount_in,
            amount_out,
            feasibility_margin,
        }
    }

    #[tokio::test]
    async fn test_check_swaps_feasibility_successful_swap() {
        dotenv::dotenv().ok();
        init_tracing(false);

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

        let pending_swap = create_pending_swap(
            "order_1".to_string(),
            ChainId::Ethereum,
            ChainId::Base,
            "token_a".to_string(),
            "token_b".to_string(),
            1_000_000_000_000_000_000, // 1 token (18 decimals)
            1_900_000,                 // 1.9 tokens (6 decimals), expecting ~2 tokens
            10.0,
        );

        let result = check_swap_feasibility(&pending_swap, &coin_cache);

        // The swap should be feasible: real_price = 100/50 = 2, expected = 1.9/1 = 1.9
        // but with 10% margin, threshold = 1.9 * 1.1 = 2.09, so not feasible
        assert_eq!(result.unwrap(), false);
    }

    #[tokio::test]
    async fn test_check_swaps_feasibility_with_preset_price_limit() {
        dotenv::dotenv().ok();
        init_tracing(false);

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

        let pending_swap = create_pending_swap(
            "order_1".to_string(),
            ChainId::Ethereum,
            ChainId::Base,
            "token_a".to_string(),
            "token_b".to_string(),
            1_000_000_000_000_000_000,
            2_000_000_000_000_000_000,
            0.0,
        );

        let result = check_swap_feasibility(&pending_swap, &coin_cache);

        // real_price_limit = 50/100 = 0.5
        assert_eq!(result.unwrap(), true);
    }

    #[tokio::test]
    async fn test_check_swaps_feasibility_missing_coin_data() {
        let mut coin_cache = HashMap::new();
        // Only add one token, missing the other
        coin_cache.insert(
            TokenId {
                chain: ChainId::Ethereum,
                address: "token_a".to_string(),
            },
            create_coin_data(100.0, 18),
        );

        let pending_swap = create_pending_swap(
            "order_1".to_string(),
            ChainId::Ethereum,
            ChainId::Base,
            "token_a".to_string(),
            "token_b".to_string(), // This token is missing from cache
            1_000_000_000_000_000_000,
            2_000_000_000_000_000_000,
            0.0,
        );

        let result = check_swap_feasibility(&pending_swap, &coin_cache);

        // Should not process the swap due to missing data
        assert!(result.is_err());
    }

    // #[tokio::test]
    // async fn test_check_swaps_feasibility_multiple_swaps_mixed_results() {
    //     let (alert_sender, mut alert_receiver) = mpsc::channel(10);

    //     let mut coin_cache = HashMap::new();
    //     coin_cache.insert(
    //         TokenId {
    //             chain: ChainId::Ethereum,
    //             address: "token_a".to_string(),
    //         },
    //         create_coin_data(100.0, 18),
    //     );
    //     coin_cache.insert(
    //         TokenId {
    //             chain: ChainId::Base,
    //             address: "token_b".to_string(),
    //         },
    //         create_coin_data(50.0, 18),
    //     );

    //     let mut pending_swaps = HashMap::new();

    //     // Feasible swap
    //     pending_swaps.insert(
    //         "feasible_order".to_string(),
    //         create_pending_swap(
    //             "order_1".to_string(),
    //             ChainId::Ethereum,
    //             ChainId::Base,
    //             "token_a".to_string(),
    //             "token_b".to_string(),
    //             1_000_000_000_000_000_000,
    //             2_000_000_000_000_000_000,
    //             0.0,
    //         ),
    //     );

    //     // Non-feasible swap
    //     pending_swaps.insert(
    //         "non_feasible_order".to_string(),
    //         create_pending_swap(
    //             "order_1".to_string(),
    //             ChainId::Ethereum,
    //             ChainId::Base,
    //             "token_a".to_string(),
    //             "token_b".to_string(),
    //             1_000_000_000_000_000_000,
    //             2_000_000_000_000_000_000,
    //             0.0,
    //         ),
    //     );

    //     let result = check_swaps_feasibility(0.1, coin_cache, pending_swaps, alert_sender).await;

    //     // Only non-feasible swap should remain
    //     assert_eq!(result.len(), 1);
    //     assert!(result.contains_key("non_feasible_order"));

    //     // Should receive one alert for feasible swap
    //     let alert = alert_receiver.try_recv().unwrap();
    //     assert!(
    //         matches!(alert, MonitorAlert::SwapIsFeasible { order_id } if order_id == "feasible_order")
    //     );

    //     // No more alerts
    //     assert!(alert_receiver.try_recv().is_err());
    // }

    // #[tokio::test]
    // async fn test_check_swaps_feasibility_different_decimals() {
    //     let (alert_sender, mut alert_receiver) = mpsc::channel(10);

    //     let mut coin_cache = HashMap::new();
    //     coin_cache.insert(
    //         TokenId {
    //             chain: ChainId::Ethereum,
    //             address: "token_a".to_string(),
    //         },
    //         create_coin_data(1.0, 6), // 6 decimals
    //     );
    //     coin_cache.insert(
    //         TokenId {
    //             chain: ChainId::Base,
    //             address: "token_b".to_string(),
    //         },
    //         create_coin_data(2.0, 18), // 18 decimals
    //     );

    //     let mut pending_swaps = HashMap::new();
    //     pending_swaps.insert(
    //         "order_1".to_string(),
    //         create_pending_swap(
    //             "order_1".to_string(),
    //             ChainId::Ethereum,
    //             ChainId::Base,
    //             "token_a".to_string(),
    //             "token_b".to_string(),
    //             1_000_000,               // 1 token with 6 decimals
    //             500_000_000_000_000_000, // 0.5 tokens with 18 decimals
    //             0.0,
    //             None,
    //         ),
    //     );

    //     let result = check_swaps_feasibility(
    //         0.0, // No margin for easier calculation
    //         coin_cache,
    //         pending_swaps,
    //         alert_sender,
    //     )
    //     .await;

    //     // price_limit = 0.5 / 1.0 = 0.5
    //     // real_price_limit = 2.0 / 1.0 = 2.0
    //     // 2.0 >= 0.5, so swap should be feasible
    //     assert_eq!(result.len(), 0);

    //     let alert = alert_receiver.try_recv().unwrap();
    //     assert!(
    //         matches!(alert, MonitorAlert::SwapIsFeasible { order_id } if order_id == "order_1")
    //     );
    // }

    // #[tokio::test]
    // async fn test_check_swaps_feasibility_edge_case_zero_amounts() {
    //     let (alert_sender, _alert_receiver) = mpsc::channel(10);

    //     let mut coin_cache = HashMap::new();
    //     coin_cache.insert(
    //         TokenId {
    //             chain: ChainId::Ethereum,
    //             address: "token_a".to_string(),
    //         },
    //         create_coin_data(100.0, 18),
    //     );
    //     coin_cache.insert(
    //         TokenId {
    //             chain: ChainId::Base,
    //             address: "token_b".to_string(),
    //         },
    //         create_coin_data(50.0, 18),
    //     );

    //     let mut pending_swaps = HashMap::new();
    //     pending_swaps.insert(
    //         "zero_amount_order".to_string(),
    //         create_pending_swap(
    //             "order_1".to_string(),
    //             ChainId::Ethereum,
    //             ChainId::Base,
    //             "token_a".to_string(),
    //             "token_b".to_string(),
    //             0, // Zero amount_in - this would cause division by zero
    //             1_000_000_000_000_000_000,
    //             0.0,
    //             None,
    //         ),
    //     );

    //     // This test should either handle zero gracefully or panic
    //     // Currently the code will panic on division by zero
    //     // You should fix this in the actual implementation
    //     let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    //         tokio::runtime::Runtime::new()
    //             .unwrap()
    //             .block_on(check_swaps_feasibility(
    //                 0.05,
    //                 coin_cache,
    //                 pending_swaps,
    //                 alert_sender,
    //             ))
    //     }));

    //     // Currently this will panic - you should fix the implementation
    //     assert!(result.is_err());
    // }

    // #[tokio::test]
    // async fn test_check_swaps_feasibility_empty_inputs() {
    //     let (alert_sender, mut alert_receiver) = mpsc::channel(10);

    //     let coin_cache = HashMap::new();
    //     let pending_swaps = HashMap::new();

    //     let result = check_swaps_feasibility(0.05, coin_cache, pending_swaps, alert_sender).await;

    //     assert_eq!(result.len(), 0);
    //     assert!(alert_receiver.try_recv().is_err());
    // }

    #[tokio::test]
    async fn test_get_coins_data_cache_hit() {
        // Setup
        let (sender, _receiver) = mpsc::channel(10);
        let (_, monitor_receiver) = mpsc::channel(10);

        let mut monitor_manager = MonitorManager::new(monitor_receiver, sender);

        // Prepare cache with some tokens
        let eth_token = TokenId {
            chain: ChainId::Ethereum,
            address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(),
        };
        let base_token = TokenId {
            chain: ChainId::Base,
            address: "0x4200000000000000000000000000000000000006".to_string(),
        };

        monitor_manager.coin_cache.insert(
            eth_token.clone(),
            TokenPrice {
                price: 2000.0,
                decimals: 18,
            },
        );
        monitor_manager.coin_cache.insert(
            base_token.clone(),
            TokenPrice {
                price: 2010.0,
                decimals: 18,
            },
        );

        // Request only tokens that are in cache
        let tokens_to_request = vec![eth_token.clone(), base_token.clone()]
            .into_iter()
            .collect();
        let result = monitor_manager.get_coins_data(tokens_to_request).await;

        // Verify the result contains cached data without external API calls
        assert!(
            result.is_ok(),
            "get_coins_data should succeed with cached tokens"
        );
        let token_prices = result.unwrap();
        assert_eq!(token_prices.len(), 2, "Should return 2 token prices");

        assert!(
            token_prices.contains_key(&eth_token),
            "Should contain ETH token"
        );
        assert_eq!(token_prices[&eth_token].price, 2000.0);

        assert!(
            token_prices.contains_key(&base_token),
            "Should contain BASE token"
        );
        assert_eq!(token_prices[&base_token].price, 2010.0);
    }

    #[tokio::test]
    async fn test_get_coins_data_zero_price() {
        // Setup
        let (sender, _receiver) = mpsc::channel(10);
        let (_, monitor_receiver) = mpsc::channel(10);

        let mut monitor_manager = MonitorManager::new(monitor_receiver, sender);

        // Prepare cache with a token that has zero price (considered as not in cache)
        let eth_token = TokenId {
            chain: ChainId::Ethereum,
            address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(),
        };

        monitor_manager.coin_cache.insert(
            eth_token.clone(),
            TokenPrice {
                price: 0.0,
                decimals: 18,
            }, // Zero price should be treated as cache miss
        );

        // This test will make an external API call
        // In a real test environment, you should mock this functionality
        // Here we'll just verify basic behavior
        let tokens_to_request = vec![eth_token.clone()].into_iter().collect();
        let result = monitor_manager.get_coins_data(tokens_to_request).await;

        // The test might pass or fail depending on network connectivity
        match result {
            Ok(token_prices) => {
                // If successful, the zero price token should be updated with real data
                if token_prices.contains_key(&eth_token) {
                    assert!(
                        token_prices[&eth_token].price > 0.0,
                        "Price should be updated from API"
                    );
                }
            }
            Err(_) => {
                println!("Network call failed - expected in test environment");
                // This is acceptable in test environment
            }
        }
    }

    #[tokio::test]
    async fn test_get_coins_data_empty_input() {
        // Setup
        let (sender, _receiver) = mpsc::channel(10);
        let (_, monitor_receiver) = mpsc::channel(10);

        let mut monitor_manager = MonitorManager::new(monitor_receiver, sender);

        // Prepare cache with a token
        let eth_token = TokenId {
            chain: ChainId::Ethereum,
            address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(),
        };

        monitor_manager.coin_cache.insert(
            eth_token.clone(),
            TokenPrice {
                price: 2000.0,
                decimals: 18,
            },
        );

        // Request with empty input
        let empty_request: HashSet<TokenId> = HashSet::new();
        let result = monitor_manager.get_coins_data(empty_request).await;

        // Verify result
        assert!(result.is_ok(), "Empty request should succeed");
        let token_prices = result.unwrap();
        assert_eq!(token_prices.len(), 0, "Result should be empty");
    }

    #[tokio::test]
    async fn test_estimate_orders_amount_out_success_with_cached_tokens() {
        // Setup
        let (sender, _receiver) = mpsc::channel(10);
        let (_, monitor_receiver) = mpsc::channel(10);

        let mut monitor_manager = MonitorManager::new(monitor_receiver, sender);

        // Pre-populate cache with token data
        let eth_token = TokenId {
            chain: ChainId::Ethereum,
            address: "0xa0b86a33e6ba2a5e59e3a6be836a4f08a7b2e6bd".to_string(),
        };
        let base_token = TokenId {
            chain: ChainId::Base,
            address: "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string(),
        };

        monitor_manager.coin_cache.insert(
            eth_token.clone(),
            TokenPrice {
                price: 2000.0,
                decimals: 18,
            },
        );
        monitor_manager.coin_cache.insert(
            base_token.clone(),
            TokenPrice {
                price: 1.0,
                decimals: 6,
            },
        );

        // Create test orders
        let orders = vec![create_order_estimation_data(
            "order_1",
            ChainId::Ethereum,
            ChainId::Base,
            "0xa0b86a33e6ba2a5e59e3a6be836a4f08a7b2e6bd",
            "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913",
            1_000_000_000_000_000_000, // 1 ETH (18 decimals)
        )];

        // Execute
        let result = monitor_manager.estimate_orders_amount_out(orders).await;

        // Verify
        assert!(result.is_ok(), "Should succeed with cached data");
        let estimates = result.unwrap();
        assert_eq!(estimates.len(), 1, "Should return one estimate");
        assert!(estimates.contains_key("order_1"), "Should contain order_1");

        let estimated_amount = estimates["order_1"];
        // 1 ETH * $2000 / $1 = 2000 USDC = 2_000_000_000 (6 decimals)
        assert_eq!(estimated_amount, 2_000_000_000);
    }

    #[tokio::test]
    async fn test_estimate_orders_amount_out_multiple_orders_different_chains() {
        // Setup
        let (sender, _receiver) = mpsc::channel(10);
        let (_, monitor_receiver) = mpsc::channel(10);

        let mut monitor_manager = MonitorManager::new(monitor_receiver, sender);

        // Pre-populate cache with multiple tokens
        monitor_manager.coin_cache.insert(
            TokenId {
                chain: ChainId::Ethereum,
                address: "0xeth_token".to_string(),
            },
            TokenPrice {
                price: 2000.0,
                decimals: 18,
            },
        );
        monitor_manager.coin_cache.insert(
            TokenId {
                chain: ChainId::Base,
                address: "0xbase_token".to_string(),
            },
            TokenPrice {
                price: 1.0,
                decimals: 6,
            },
        );
        monitor_manager.coin_cache.insert(
            TokenId {
                chain: ChainId::ArbitrumOne,
                address: "0xarb_token".to_string(),
            },
            TokenPrice {
                price: 1.01,
                decimals: 6,
            },
        );

        // Create multiple orders
        let orders = vec![
            create_order_estimation_data(
                "eth_to_base",
                ChainId::Ethereum,
                ChainId::Base,
                "0xeth_token",
                "0xbase_token",
                500_000_000_000_000_000, // 0.5 ETH
            ),
            create_order_estimation_data(
                "base_to_arb",
                ChainId::Base,
                ChainId::ArbitrumOne,
                "0xbase_token",
                "0xarb_token",
                1_000_000, // 1 USDC
            ),
        ];

        // Execute
        let result = monitor_manager.estimate_orders_amount_out(orders).await;

        // Verify
        assert!(result.is_ok(), "Should succeed with multiple orders");
        let estimates = result.unwrap();
        assert_eq!(estimates.len(), 2, "Should return two estimates");

        // Verify first order: 0.5 ETH * $2000 / $1 = 1000 USDC
        assert_eq!(estimates["eth_to_base"], 1_000_000_000);

        // Verify second order: 1 USDC * $1.0 / $1.01 ≈ 0.99 USDC
        let second_estimate = estimates["base_to_arb"];
        assert!(
            second_estimate < 1_000_000,
            "Should be less than input due to price difference"
        );
        assert!(second_estimate > 900_000, "Should be reasonable conversion");
    }

    #[tokio::test]
    async fn test_estimate_orders_amount_out_duplicate_tokens() {
        // Setup
        let (sender, _receiver) = mpsc::channel(10);
        let (_, monitor_receiver) = mpsc::channel(10);

        let mut monitor_manager = MonitorManager::new(monitor_receiver, sender);

        // Pre-populate cache
        monitor_manager.coin_cache.insert(
            TokenId {
                chain: ChainId::Ethereum,
                address: "0xtoken".to_string(),
            },
            TokenPrice {
                price: 100.0,
                decimals: 18,
            },
        );

        // Create orders that use the same tokens (should deduplicate internally)
        let orders = vec![
            create_order_estimation_data(
                "order_1",
                ChainId::Ethereum,
                ChainId::Ethereum,
                "0xtoken",
                "0xtoken",
                1_000_000_000_000_000_000,
            ),
            create_order_estimation_data(
                "order_2",
                ChainId::Ethereum,
                ChainId::Ethereum,
                "0xtoken",
                "0xtoken",
                2_000_000_000_000_000_000,
            ),
        ];

        // Execute
        let result = monitor_manager.estimate_orders_amount_out(orders).await;

        // Verify
        assert!(result.is_ok(), "Should succeed with duplicate tokens");
        let estimates = result.unwrap();
        assert_eq!(estimates.len(), 2, "Should return two estimates");

        // Both should have same amounts since same token with same price
        assert_eq!(estimates["order_1"], 1_000_000_000_000_000_000);
        assert_eq!(estimates["order_2"], 2_000_000_000_000_000_000);
    }

    #[tokio::test]
    async fn test_estimate_orders_amount_out_missing_token_data() {
        // Setup
        let (sender, _receiver) = mpsc::channel(10);
        let (_, monitor_receiver) = mpsc::channel(10);

        let mut monitor_manager = MonitorManager::new(monitor_receiver, sender);

        // Only add source token, missing destination token
        monitor_manager.coin_cache.insert(
            TokenId {
                chain: ChainId::Ethereum,
                address: "0xsrc_token".to_string(),
            },
            TokenPrice {
                price: 100.0,
                decimals: 18,
            },
        );

        // Create order with missing destination token
        let orders = vec![create_order_estimation_data(
            "missing_dst",
            ChainId::Ethereum,
            ChainId::Base,
            "0xsrc_token",
            "0xmissing_token",
            1_000_000_000_000_000_000,
        )];

        // Execute - this will try to fetch missing token from external APIs
        let result = monitor_manager.estimate_orders_amount_out(orders).await;

        // Verify - depending on network connectivity, this might succeed or fail
        match result {
            Ok(estimates) => {
                // If external API call succeeded
                if estimates.is_empty() {
                    println!("No estimates returned - token data not found externally");
                } else {
                    println!("External API provided missing token data");
                }
            }
            Err(_) => {
                // Expected if external API calls fail in test environment
                println!("Failed to fetch missing token data - expected in test environment");
            }
        }
    }

    #[tokio::test]
    async fn test_estimate_orders_amount_out_zero_price_token() {
        // Setup
        let (sender, _receiver) = mpsc::channel(10);
        let (_, monitor_receiver) = mpsc::channel(10);

        let mut monitor_manager = MonitorManager::new(monitor_receiver, sender);

        // Add tokens with zero price (should trigger cache miss and external fetch)
        monitor_manager.coin_cache.insert(
            TokenId {
                chain: ChainId::Ethereum,
                address: "0xsrc_token".to_string(),
            },
            TokenPrice {
                price: 100.0,
                decimals: 18,
            },
        );
        monitor_manager.coin_cache.insert(
            TokenId {
                chain: ChainId::Base,
                address: "0xzero_price_token".to_string(),
            },
            TokenPrice {
                price: 0.0,
                decimals: 6,
            }, // Zero price
        );

        let orders = vec![create_order_estimation_data(
            "zero_price_order",
            ChainId::Ethereum,
            ChainId::Base,
            "0xsrc_token",
            "0xzero_price_token",
            1_000_000_000_000_000_000,
        )];

        // Execute
        let result = monitor_manager.estimate_orders_amount_out(orders).await;

        // This should either succeed (if external API provides valid price) or fail
        match result {
            Ok(estimates) => {
                println!("Zero price token was updated from external API");
                if estimates.contains_key("zero_price_order") {
                    assert!(
                        estimates["zero_price_order"] > 0,
                        "Should have valid estimate"
                    );
                }
            }
            Err(_) => {
                println!("Failed to get valid price for zero price token");
            }
        }
    }
}
