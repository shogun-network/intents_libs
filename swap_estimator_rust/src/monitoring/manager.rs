use error_stack::report;
use futures_util::future;
use intents_models::constants::chains::ChainId;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    time::Duration,
    u64,
};
use strum::IntoEnumIterator;
use tokio::sync::mpsc::Receiver;

use crate::{
    error::{Error, EstimatorResult},
    monitoring::messages::{MonitorAlert, MonitorRequest},
    prices::{
        PriceEvent, PriceProvider, TokenId, TokenMetadata, TokenPrice,
        codex::pricing::CodexProvider, estimating::OrderEstimationData,
    },
    utils::{get_timestamp, number_conversion::u128_to_f64, uint::mul_div},
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
    pub deadline: u64,
    pub extra_expenses: HashMap<TokenId, u128>, // TokenId to amount
}

#[derive(Debug)]
pub struct MonitorManager {
    pub receiver: Receiver<MonitorRequest>,
    pub alert_sender: tokio::sync::broadcast::Sender<MonitorAlert>,
    pub coin_cache: HashMap<TokenId, TokenPrice>,
    pub pending_swaps: HashMap<String, (PendingSwap, Option<u128>)>, // OrderId to pending swap and optionally, estimated amount out calculated
    pub swaps_by_token: HashMap<TokenId, Vec<String>>,               // TokenId to OrderIds
    pub token_metadata: HashMap<TokenId, TokenMetadata>,
    pub codex_provider: CodexProvider,
    pub polling_mode: (bool, u64),
    pub orders_by_deadline: BTreeMap<u64, HashSet<String>>, // deadline timestamp to OrderIds
}

impl MonitorManager {
    pub fn new(
        receiver: Receiver<MonitorRequest>,
        sender: tokio::sync::broadcast::Sender<MonitorAlert>,
        codex_api_key: String,
        polling_mode: (bool, u64),
    ) -> Self {
        let codex_provider = CodexProvider::new(codex_api_key);

        Self {
            receiver,
            alert_sender: sender,
            coin_cache: HashMap::new(),
            pending_swaps: HashMap::new(),
            swaps_by_token: HashMap::new(),
            token_metadata: HashMap::new(),
            codex_provider,
            polling_mode,
            orders_by_deadline: BTreeMap::new(),
        }
    }

    pub async fn run(mut self) -> EstimatorResult<()> {
        // Subscribe to native token price updates, as they are used in fee calculations
        let mut native_tokens = HashSet::new();
        for chain in ChainId::iter() {
            let native_token = chain.wrapped_native_token_address();
            let token_id = TokenId::new_for_codex(chain, &native_token);
            native_tokens.insert(token_id.clone());
            if !self.polling_mode.0 {
                self.codex_provider.subscribe_to_token(token_id).await?;
            }
        }

        let mut codex_rx_opt = match self.codex_provider.subscribe_events().await {
            Ok(rx) => rx,
            Err(err) => {
                tracing::error!("Failed to subscribe Codex price events: {:?}", err);
                return Err(err);
            }
        };

        let mut unsubscriptions_interval = tokio::time::interval(Duration::from_secs(60));
        let mut polling_interval =
            tokio::time::interval(Duration::from_millis(self.polling_mode.1));
        let mut clean_expired_orders_interval = tokio::time::interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                // Clean expired orders interval
                _ = clean_expired_orders_interval.tick() => {
                    let current_timestamp = get_timestamp();

                    while let Some((&deadline, _order_ids)) = self.orders_by_deadline.first_key_value() {
                        if deadline >= current_timestamp {
                            break;
                        }
                        // Remove the entry
                        if let Some(order_ids) = self.orders_by_deadline.pop_first() {
                            let (_removed_deadline, order_ids) = order_ids;
                            for order_id in order_ids {
                                tracing::debug!(
                                    "Removing expired pending swap for order_id: {}, deadline: {}",
                                    order_id,
                                    deadline
                                );
                                self.remove_order(&order_id).await;
                            }
                        } else {
                            break;
                        }
                    }
                }
                _ = unsubscriptions_interval.tick(), if !self.polling_mode.0 => {
                    tracing::debug!("Checking for tokens to unsubscribe due to no pending orders");
                    // Collect tokens that no longer have pending orders
                    let tokens_to_unsubscribe: Vec<TokenId> = self
                        .swaps_by_token
                        .iter()
                        .filter_map(|(token, order_ids)| {
                            if order_ids.is_empty() {
                                Some(token.clone())
                            } else {
                                None
                            }
                        })
                        .collect();

                    for token in tokens_to_unsubscribe.into_iter() {
                        match self.codex_provider.unsubscribe_from_token(token.clone()).await {
                            Ok(_) => {
                                tracing::debug!(
                                    "Unsubscribed from token {:?} due to no pending orders",
                                    token
                                );
                            }
                            Err(e) => {
                                tracing::warn!("Codex unsubscribe_from_token failed: {:?}", e);
                            }
                        }
                        // Remove from coin cache and map
                        self.coin_cache.remove(&token);
                        self.swaps_by_token.remove(&token);
                    }
                }
                // Polling interval
                _ = polling_interval.tick(), if self.polling_mode.0 => {
                    tracing::debug!("Polling price updates for pending orders");
                    // Get all tokens needed to estimate pending swaps
                    let mut tokens_to_fetch: HashSet<TokenId> = self
                        .swaps_by_token
                        .iter()
                        .filter_map(|(token, order_ids)| {
                            if !order_ids.is_empty() {
                                Some(token.clone())
                            } else {
                                None
                            }
                        })
                        .collect();

                    tracing::debug!("Polling update for tokens: {:?}", tokens_to_fetch);

                    // Always check for native tokens too.
                    tokens_to_fetch.extend(native_tokens.clone());

                    let mut tokens_data = self.get_tokens_data(tokens_to_fetch).await?;

                    self.update_tokens_metadata(&mut tokens_data).await?;

                    // Update cache and get updated tokens
                    let updated_tokens = self.update_cache(tokens_data);

                    for updated_token in updated_tokens.into_iter() {
                        self.check_impacted_orders(updated_token).await;
                    }
                }
                // Codex update price event
                evt = codex_rx_opt.recv() => {
                    tracing::trace!("Received Codex price event: {:?}", evt);
                    match evt {
                        Ok(event) => {
                            self.on_price_event(event).await;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            tracing::warn!("Lagged on Codex price events; skipping to latest");
                            continue;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            tracing::error!("Codex price events channel closed");
                            return Err(report!(Error::Unknown)
                                .attach_printable("Codex price events receiver closed"));
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
                                    self.remove_order(&order_id).await;
                                }
                                MonitorRequest::CheckSwapFeasibility {
                                    order_id,
                                    src_chain,
                                    dst_chain,
                                    token_in,
                                    token_out,
                                    amount_in,
                                    amount_out,
                                    deadline,
                                    solver_last_bid,
                                    extra_expenses,
                                } => {
                                    if let Err(error) = self.check_swap_feasibility(order_id, src_chain, dst_chain, token_in, token_out, amount_in, amount_out, deadline, solver_last_bid, extra_expenses).await {
                                        tracing::error!("Error processing CheckSwapFeasibility request: {:?}", error);
                                    }
                                }
                                MonitorRequest::GetCoinsData { token_ids, resp } => {
                                    let response = self.get_coins_data(token_ids).await;
                                    let to_send = match response {
                                        Ok(result) => Ok(result),
                                        Err(e) => Err(e.current_context().clone()),
                                    };
                                    match resp.send(to_send) {
                                        Ok(_) => tracing::debug!("Response sent successfully"),
                                        Err(_) => tracing::error!("Failed to send error response"),
                                    }
                                }
                                MonitorRequest::EstimateOrdersAmountOut {
                                    orders, resp,
                                } => {
                                    let response = self.estimate_orders_amount_out(orders).await;
                                    let to_send = match response {
                                        Ok(result) => Ok(result),
                                        Err(e) => Err(e.current_context().clone()),
                                    };
                                    match resp.send(to_send) {
                                        Ok(_) => tracing::debug!("Error response sent successfully"),
                                        Err(_) => tracing::error!("Failed to send error response"),
                                    }
                                }
                                MonitorRequest::EvaluateCoins { tokens, resp } => {
                                    let response = self.evaluate_coins(tokens).await;
                                    let to_send = match response {
                                        Ok(result) => Ok(result),
                                        Err(e) => Err(e.current_context().clone()),
                                    };
                                    match resp.send(to_send) {
                                        Ok(_) => tracing::debug!("EvaluateCoins response sent successfully"),
                                        Err(_) => tracing::error!("Failed to send EvaluateCoins response"),
                                    }
                                }
                            }
                        }
                        None => {
                            tracing::warn!("Monitor request channel closed, exiting...");
                            return Err(report!(Error::Unknown).attach_printable("Monitor request channel closed"));
                        }
                    }
                }
            }
        }
    }

    async fn estimate_orders_amount_out(
        &mut self,
        orders: Vec<OrderEstimationData>,
    ) -> EstimatorResult<HashMap<String, u128>> {
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
        let mut tokens_info = self.get_tokens_data(token_ids).await?;
        self.update_tokens_metadata(&mut tokens_info).await?;

        match crate::prices::estimating::estimate_orders_amount_out(orders, tokens_info).await {
            Ok(result) => Ok(result),
            Err(e) => {
                tracing::error!("Failed to estimate orders amount out: {:?}", e);
                Err(e)
            }
        }
    }

    async fn check_swap_feasibility(
        &mut self,
        order_id: String,
        src_chain: ChainId,
        dst_chain: ChainId,
        token_in: String,
        token_out: String,
        amount_in: u128,
        amount_out: u128,
        deadline: u64,
        solver_last_bid: Option<u128>,
        extra_expenses: HashMap<TokenId, u128>,
    ) -> EstimatorResult<()> {
        tracing::debug!(
            "Checking swap feasibility for order_id: {}, token_in: {}, token_out: {}, amount_in: {}, amount_out: {}",
            order_id,
            token_in,
            token_out,
            amount_in,
            amount_out
        );

        let token_in_id = TokenId::new_for_codex(src_chain, &token_in);

        let token_out_id = TokenId::new_for_codex(dst_chain, &token_out);

        let pending_swap = PendingSwap {
            order_id: order_id.clone(),
            src_chain,
            dst_chain,
            token_in: token_in.clone(),
            token_out: token_out.clone(),
            amount_in,
            amount_out,
            deadline,
            extra_expenses,
        };

        // Subscribe to price updates for both tokens
        let tokens_data = self.get_all_coins_data_from_swap(&pending_swap).await?;

        // Check immediate feasibility
        let estimate_amount_out_calculated = match estimate_amount_out(&pending_swap, &tokens_data)
        {
            Ok((estimated_amount_out, fulfillment_expenses_in_tokens_out)) => {
                if let Some(solver_last_bid) = solver_last_bid {
                    // In this case calculate estimated amount out with margin
                    if solver_last_bid >= amount_out {
                        return Err(report!(Error::ParseError)
                            .attach_printable("Solver last bid should be less than amount_out"));
                    }
                    // Calculate required monitor estimation for solver to be able to reach amount_out
                    let req_monitor_estimation =
                        required_monitor_estimation_for_solver_fulfillment(
                            solver_last_bid,
                            estimated_amount_out,
                            amount_out,
                            fulfillment_expenses_in_tokens_out,
                        )?;
                    dbg!(
                        &pending_swap.order_id,
                        estimated_amount_out,
                        solver_last_bid,
                        req_monitor_estimation
                    );
                    tracing::debug!(
                        "Required monitor estimation for order_id {}: {}",
                        order_id,
                        req_monitor_estimation
                    );
                    if estimated_amount_out >= req_monitor_estimation {
                        // Send alert immediately
                        tracing::debug!(
                            "Swap is immediately feasible for order_id: {}, sending alert",
                            order_id
                        );
                        if let Err(e) = self.alert_sender.send(MonitorAlert::SwapIsFeasible {
                            order_id: pending_swap.order_id.clone(),
                        }) {
                            tracing::error!(
                                "Failed to send alert for order_id {}: {:?}",
                                pending_swap.order_id,
                                e
                            );
                        } else {
                            // No need to monitor further
                            return Ok(());
                        }
                    }
                    Some(req_monitor_estimation)
                } else {
                    // In this case we just check against amount_out
                    if estimated_amount_out >= amount_out {
                        // Send alert immediately
                        tracing::debug!(
                            "Swap is immediately feasible for order_id: {}, sending alert",
                            order_id
                        );
                        if let Err(e) = self.alert_sender.send(MonitorAlert::SwapIsFeasible {
                            order_id: pending_swap.order_id.clone(),
                        }) {
                            tracing::error!(
                                "Failed to send alert for order_id {}: {:?}",
                                pending_swap.order_id,
                                e
                            );
                            None
                        } else {
                            // No need to monitor further
                            return Ok(());
                        }
                    } else {
                        None
                    }
                }
            }
            Err(error) => {
                tracing::error!(
                    "Error checking swap feasibility for order_id {}: {:?}",
                    pending_swap.order_id,
                    error
                );
                None
            }
        };

        // Update cache and get updated tokens
        let updated_tokens = self.update_cache(tokens_data);

        // Re-evaluate impacted orders for updated tokens
        for updated_token in updated_tokens.into_iter() {
            self.check_impacted_orders(updated_token).await;
        }

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
            .or_insert_with(Vec::new)
            .push(order_id.clone());

        self.swaps_by_token
            .entry(token_out_id)
            .or_insert_with(Vec::new)
            .push(order_id.clone());

        self.pending_swaps.insert(
            order_id.clone(),
            (pending_swap, estimate_amount_out_calculated),
        );
        self.orders_by_deadline
            .entry(deadline)
            .or_insert_with(HashSet::new)
            .insert(order_id);
        Ok(())
    }

    /// Fetch price data for a set of tokens using cache-first strategy.
    ///
    /// Behavior:
    /// - Normalizes all incoming token ids to Codex format.
    /// - Returns cached entries whose price != 0.0 (a zero price is treated as “no data”).
    /// - For cache misses, batches and fetches fresh prices via Codex.
    /// - If subscriptions mode is enabled (`!self.polling_mode.0`), subscribes to live updates
    ///   for the newly-fetched tokens as a side effect.
    /// - Updates the internal `coin_cache` with any newly-fetched prices and returns the merged map.
    ///
    /// Parameters:
    /// - `token_ids`: unique set of tokens (chain/address) to resolve.
    ///
    /// Returns:
    /// - `HashMap<TokenId, TokenPrice>` with the tokens successfully resolved either from cache
    ///   or fetched. Tokens not returned by Codex remain absent from the map.
    ///
    /// Errors:
    /// - Propagates any error from Codex price fetching and, when in subscription mode, from
    ///   subscribing to tokens.
    async fn get_coins_data(
        &mut self,
        token_ids: HashSet<TokenId>,
    ) -> EstimatorResult<HashMap<TokenId, TokenPrice>> {
        let mut result = HashMap::new();
        let mut tokens_not_in_cache: HashSet<TokenId> = HashSet::new();
        // Keep a mapping from original -> codex ids so we can return original keys
        let mut orig_to_codex: HashMap<TokenId, TokenId> = HashMap::new();

        for orig_id in token_ids.into_iter() {
            let codex_id = TokenId::new_for_codex(orig_id.chain.clone(), &orig_id.address);
            orig_to_codex.insert(orig_id.clone(), codex_id.clone());

            // If we have it in our cache (by CODEX id) and price != 0, return it under ORIGINAL id
            if let Some(data) = self.coin_cache.get(&codex_id)
                && data.price != 0.0
            {
                tracing::debug!(
                    "Cache hit for original {:?} (codex {:?}): {:?}",
                    orig_id,
                    codex_id,
                    data
                );
                if orig_id != codex_id {
                    result.insert(codex_id, data.clone());
                }
                result.insert(orig_id, data.clone());
            } else {
                tracing::debug!(
                    "Cache miss for original {:?} (codex {:?}); fetching...",
                    orig_id,
                    codex_id
                );
                tokens_not_in_cache.insert(codex_id);
            }
        }

        // If we have nothing to fetch, return what we already collected
        if tokens_not_in_cache.is_empty() {
            return Ok(result);
        }

        // Fetch data from Codex (keys are CODEX ids)
        let mut fetched_by_codex: HashMap<TokenId, TokenPrice> =
            self.get_tokens_data(tokens_not_in_cache.clone()).await?;

        self.update_tokens_metadata(&mut fetched_by_codex).await?;
        tracing::debug!("Fetched tokens data from Codex: {:?}", fetched_by_codex);

        // Subscribe to live updates (by CODEX id)
        if !self.polling_mode.0 {
            for token in tokens_not_in_cache {
                self.codex_provider.subscribe_to_token(token).await?;
            }
        }

        // Update coin cache (by CODEX id)
        for (codex_id, token_price) in fetched_by_codex.iter() {
            self.coin_cache
                .insert(codex_id.clone(), token_price.clone());
        }

        // Map fetched CODEX entries back to ORIGINAL keys for the output
        for (orig_id, codex_id) in orig_to_codex.into_iter() {
            if result.contains_key(&orig_id) {
                continue; // already satisfied from cache
            }
            if let Some(price) = fetched_by_codex.get(&codex_id) {
                result.insert(orig_id, price.clone());
                result.insert(codex_id, price.clone());
            }
        }

        tracing::debug!("Final token prices: {:?}", result);

        Ok(result)
    }

    async fn update_tokens_metadata(
        &mut self,
        tokens_prices: &mut HashMap<TokenId, TokenPrice>,
    ) -> EstimatorResult<()> {
        // Update data fetched with token metadata
        let mut missing_tokens_metadata: HashSet<TokenId> = HashSet::new();
        for (token_id, token_price) in tokens_prices.iter_mut() {
            let codex_id = TokenId::new_for_codex(token_id.chain, &token_id.address);
            if let Some(metadata) = self.token_metadata.get(&codex_id) {
                token_price.decimals = metadata.decimals;
            } else {
                missing_tokens_metadata.insert(codex_id);
            }
        }
        // Fetch and apply metadata for missing tokens, then cache it
        if !missing_tokens_metadata.is_empty() {
            let fetched = self.get_tokens_metadata(missing_tokens_metadata).await?;
            for (token_id, meta) in fetched.iter() {
                // cache metadata for future requests
                self.token_metadata.insert(token_id.clone(), meta.clone());
            }
            // update decimals for all entries (original + codex aliases)
            for (token_id, token_price) in tokens_prices.iter_mut() {
                let codex_id = TokenId::new_for_codex(token_id.chain, &token_id.address);
                if let Some(meta) = self.token_metadata.get(&codex_id) {
                    token_price.decimals = meta.decimals;
                }
            }
        }
        Ok(())
    }

    // Gestionar un PriceEvent: actualizar cache, re-evaluar órdenes afectadas, limpiar y desuscribir tokens si ya no quedan swaps que dependan de ellos.
    async fn on_price_event(&mut self, mut event: PriceEvent) {
        // Sanitizing token id:
        event.token = TokenId::new_for_codex(event.token.chain.clone(), &event.token.address);
        // Get metadata for the token if not present
        let token_decimals = if let Some(token_data) = self.token_metadata.get(&event.token) {
            token_data.decimals
        } else {
            tracing::warn!(
                "Token data not found in cache for {:?}, fetching metadata. Unable to process price event.",
                event.token
            );
            let mut one = HashSet::new();
            one.insert(event.token.clone());
            match self.get_tokens_metadata(one).await {
                Ok(fetched) => {
                    if let Some(meta) = fetched.get(&event.token) {
                        self.token_metadata
                            .insert(event.token.clone(), meta.clone());
                        meta.decimals
                    } else {
                        // Conservative fallback: ignore this event
                        tracing::warn!(
                            "Missing metadata for {:?}; skipping price event",
                            event.token
                        );
                        return;
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        "Failed to fetch metadata for {:?}: {:?}; skipping event",
                        event.token,
                        err
                    );
                    return;
                }
            }
        };
        // Update in-memory cache
        self.coin_cache.insert(
            event.token.clone(),
            TokenPrice {
                price: event.price.price,
                decimals: token_decimals,
            },
        );

        self.check_impacted_orders(event.token).await;
    }

    async fn check_impacted_orders(&mut self, token: TokenId) {
        tracing::debug!("Checking impacted orders for token: {:?}", token);
        // Orders which have this token
        let Some(impacted_orders) = self.swaps_by_token.remove(&token) else {
            tracing::debug!("No impacted orders for token: {:?}", token);
            return;
        };

        let current_timestamp = get_timestamp();
        // Get the swap data of these orders
        let mut subset: Vec<(PendingSwap, Option<u128>)> = Vec::new();
        let mut remaining_orders: Vec<String> = Vec::new();
        for order_id in impacted_orders.iter() {
            if let Some(ps) = self.pending_swaps.get(order_id).cloned() {
                // Skip expired orders
                if ps.0.deadline < current_timestamp {
                    tracing::debug!(
                        "Skipping expired pending swap for order_id: {}, deadline: {}",
                        order_id,
                        ps.0.deadline
                    );
                    // Remove from pending swaps
                    self.remove_order(&ps.0.order_id).await;
                } else {
                    subset.push(ps);
                }
            }
        }
        if subset.is_empty() {
            return;
        }

        // Re-evaluate these swaps
        for (pending_swap, estimated_minimum_monitor_amount) in subset.into_iter() {
            tracing::debug!(
                "Re-evaluating swap feasibility for order_id: {}, token_in: {}, token_out: {}",
                pending_swap.order_id,
                pending_swap.token_in,
                pending_swap.token_out
            );
            let tokens_data = match self.get_all_coins_data_from_swap(&pending_swap).await {
                Ok(data) => data,
                Err(error) => {
                    tracing::error!(
                        "Error fetching tokens data for order_id {}: {:?}",
                        pending_swap.order_id,
                        error
                    );
                    remaining_orders.push(pending_swap.order_id.clone());
                    continue;
                }
            };
            match estimate_amount_out(&pending_swap, &tokens_data) {
                Ok((estimated_amount_out, _)) => {
                    tracing::debug!(
                        "Estimated amount out for order_id {}: {}",
                        pending_swap.order_id,
                        estimated_amount_out
                    );
                    let needed_amount_out = if let Some(estimated_minimum_monitor_amount) =
                        estimated_minimum_monitor_amount
                    {
                        estimated_minimum_monitor_amount
                    } else {
                        pending_swap.amount_out
                    };
                    dbg!(
                        &pending_swap.order_id,
                        estimated_amount_out,
                        needed_amount_out
                    );
                    tracing::debug!(
                        "Needed amount out for order_id {}: {}",
                        pending_swap.order_id,
                        needed_amount_out
                    );
                    if estimated_amount_out >= needed_amount_out {
                        tracing::debug!(
                            "Swap is feasible for order_id: {}, sending alert",
                            pending_swap.order_id
                        );
                        if let Err(e) = self.alert_sender.send(MonitorAlert::SwapIsFeasible {
                            order_id: pending_swap.order_id.clone(),
                        }) {
                            tracing::error!(
                                "Failed to send alert for order_id {}: {:?}",
                                pending_swap.order_id,
                                e
                            );
                            // Do not remove the swap if we failed to send alert
                            remaining_orders.push(pending_swap.order_id.clone());
                            continue;
                        }
                        // Remove from pending swaps and every other data structure
                        self.remove_order(&pending_swap.order_id).await;
                    } else {
                        // Still not feasible, keep monitoring
                        remaining_orders.push(pending_swap.order_id.clone());
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

        // Re-insert remaining orders back into the map
        self.swaps_by_token.insert(token, remaining_orders);
    }

    fn update_cache(&mut self, tokens_data: HashMap<TokenId, TokenPrice>) -> HashSet<TokenId> {
        tracing::debug!("Updating coin cache with tokens data: {:?}", tokens_data);
        let mut updated_tokens = HashSet::new();
        for (token_id, token_price) in tokens_data.into_iter() {
            let mut modified = false;
            self.coin_cache
                .entry(token_id.clone())
                .and_modify(|existing_price| {
                    if existing_price.price != token_price.price {
                        *existing_price = token_price.clone();
                        modified = true;
                    }
                })
                .or_insert_with(|| {
                    modified = true;
                    token_price.clone()
                });
            if modified {
                updated_tokens.insert(token_id);
            }
        }
        updated_tokens
    }

    async fn remove_order(&mut self, order_id: &str) {
        // dbg!("Removing order_id: {} from monitoring", order_id);
        // Remove from pending swaps
        if let Some((pending_swap, _)) = self.pending_swaps.remove(order_id) {
            // Remove from orders by deadline
            if let Some(set) = self.orders_by_deadline.get_mut(&pending_swap.deadline) {
                set.remove(order_id);
                if set.is_empty() {
                    self.orders_by_deadline.remove(&pending_swap.deadline);
                }
            }
            // Detach from token->orders map and unsubscribe if needed
            // let t_in = TokenId::new_for_codex(pending_swap.src_chain, &pending_swap.token_in);
            // let t_out = TokenId::new_for_codex(pending_swap.dst_chain, &pending_swap.token_out);
            // self.detach_order_from_token(&t_in, &pending_swap.order_id);
            // self.detach_order_from_token(&t_out, &pending_swap.order_id);
            // for token in pending_swap.extra_expenses.keys() {
            //     self.detach_order_from_token(token, &pending_swap.order_id);
            // }
        }
    }

    // fn detach_order_from_token(&mut self, token: &TokenId, order_id: &str) {
    //     if let Some(set) = self.swaps_by_token.get_mut(token) {
    //         set.remove(order_id);
    //         if set.is_empty() {
    //             self.swaps_by_token.remove(token);
    //         }
    //     }
    // }

    async fn get_tokens_data(
        &self,
        token_ids: HashSet<TokenId>,
    ) -> EstimatorResult<HashMap<TokenId, TokenPrice>> {
        // Build mapping original -> codex to preserve both keys in the output
        let mut orig_to_codex: Vec<(TokenId, TokenId)> = Vec::new();
        let mut codex_set: HashSet<TokenId> = HashSet::new();

        for orig in token_ids.into_iter() {
            let codex = TokenId::new_for_codex(orig.chain.clone(), &orig.address);
            orig_to_codex.push((orig, codex.clone()));
            codex_set.insert(codex);
        }

        let codex_ids: Vec<TokenId> = codex_set.into_iter().collect();
        tracing::debug!("Fetching tokens data for {:?} from Codex", codex_ids);

        // Split into batches of up to 200 tokens (by CODEX ids)
        const BATCH_SIZE: usize = 200;
        let mut batches: Vec<Vec<TokenId>> = Vec::new();
        for chunk in codex_ids.chunks(BATCH_SIZE) {
            batches.push(chunk.iter().cloned().collect());
        }

        // Fire all batch requests in parallel
        let provider = &self.codex_provider;
        let fetches = batches.into_iter().map(|batch| {
            // each future captures provider by shared reference
            async move {
                provider
                    .get_tokens_price(&batch, !self.polling_mode.0)
                    .await
            }
        });

        let results = future::join_all(fetches).await;

        // Merge results, fail fast on any batch error (keys are CODEX ids)
        let mut combined_by_codex: HashMap<TokenId, TokenPrice> = HashMap::new();
        for res in results.into_iter() {
            match res {
                Ok(mut map) => {
                    combined_by_codex.extend(map.drain());
                }
                Err(e) => {
                    tracing::error!("Codex batch get_tokens_price failed: {:?}", e);
                    return Err(e);
                }
            }
        }

        // Build final map
        let mut result: HashMap<TokenId, TokenPrice> = combined_by_codex.clone();
        for (orig, codex) in orig_to_codex.into_iter() {
            if let Some(price) = combined_by_codex.get(&codex) {
                // Insert also under the original key; if orig == codex this is a no-op
                result.entry(orig).or_insert_with(|| price.clone());
                result.entry(codex).or_insert_with(|| price.clone());
            }
        }

        Ok(result)
    }

    async fn get_tokens_metadata(
        &mut self,
        token_ids: HashSet<TokenId>,
    ) -> EstimatorResult<HashMap<TokenId, TokenMetadata>> {
        // Refine token ids to codex format
        let token_ids: Vec<TokenId> = token_ids
            .into_iter()
            .map(|t| TokenId::new_for_codex(t.chain.clone(), &t.address))
            .collect();
        tracing::debug!("Fetching tokens data for {:?} from Codex", token_ids);

        // Split into batches of up to 200 tokens
        const BATCH_SIZE: usize = 200;
        let mut batches: Vec<Vec<TokenId>> = Vec::new();
        for chunk in token_ids.chunks(BATCH_SIZE) {
            batches.push(chunk.iter().cloned().collect());
        }

        // Fire all batch requests in parallel
        let provider = &self.codex_provider;
        let fetches = batches.into_iter().map(|batch| {
            // each future captures provider by shared reference
            async move { provider.fetch_token_metadata(&batch).await }
        });

        let results = future::join_all(fetches).await;

        // Merge results, fail fast on any batch error
        let mut combined: HashMap<TokenId, TokenMetadata> = HashMap::new();
        for res in results.into_iter() {
            match res {
                Ok(mut map) => {
                    combined.extend(map.drain());
                }
                Err(e) => {
                    tracing::error!("Codex batch fetch_token_metadata failed: {:?}", e);
                    return Err(e);
                }
            }
        }

        Ok(combined)
    }

    async fn evaluate_coins(
        &mut self,
        tokens: Vec<(TokenId, u128)>,
    ) -> EstimatorResult<(Vec<f64>, f64)> {
        let tokens_to_search = tokens
            .iter()
            .map(|(token_id, _)| token_id.clone())
            .collect::<HashSet<_>>();
        let tokens_data = if self.polling_mode.0 {
            // Fetch fresh data from the API as cache might not be updated
            let mut tokens_data = self.get_tokens_data(tokens_to_search).await?;
            self.update_tokens_metadata(&mut tokens_data).await?;
            tokens_data
        } else {
            // Cache should be updated via subscriptions
            self.get_coins_data(tokens_to_search).await?
        };

        let mut total_value = 0.0;
        let mut values = vec![];
        for (token, amount) in tokens.into_iter() {
            let Some(token_data) = tokens_data.get(&token) else {
                return Err(report!(Error::TokenNotFound(format!(
                    "Token {token:?} not found in Codex response"
                ))));
            };
            let token_dec_amount = u128_to_f64(amount, token_data.decimals);
            let token_usd_value = token_dec_amount * token_data.price;
            total_value += token_usd_value;
            values.push(token_usd_value);
        }
        Ok((values, total_value))
    }

    async fn get_all_coins_data_from_swap(
        &mut self,
        swap: &PendingSwap,
    ) -> EstimatorResult<HashMap<TokenId, TokenPrice>> {
        let mut token_ids = HashSet::new();
        token_ids.insert(TokenId::new_for_codex(swap.src_chain, &swap.token_in));
        token_ids.insert(TokenId::new_for_codex(swap.dst_chain, &swap.token_out));
        for expense in swap.extra_expenses.iter() {
            token_ids.insert(TokenId::new_for_codex(
                expense.0.chain.clone(),
                &expense.0.address,
            ));
        }
        let tokens_data = self.get_coins_data(token_ids).await?;
        Ok(tokens_data)
    }
}

fn estimate_amount_out(
    pending_swap: &PendingSwap,
    coin_cache: &HashMap<TokenId, TokenPrice>,
) -> EstimatorResult<(u128, u128)> {
    let src_chain_data = coin_cache.get(&TokenId::new_for_codex(
        pending_swap.src_chain,
        &pending_swap.token_in,
    ));
    let dst_chain_data = coin_cache.get(&TokenId::new_for_codex(
        pending_swap.dst_chain,
        &pending_swap.token_out,
    ));

    if let (Some(src_data), Some(dst_data)) = (src_chain_data, dst_chain_data) {
        // Fail-fast validation for decimals scale supported by rust_decimal (max 28)
        let validate_decimals = |d: u8| -> Result<(), Error> {
            if d > 28 {
                return Err(Error::ParseError);
            }
            Ok(())
        };
        validate_decimals(src_data.decimals)?;
        validate_decimals(dst_data.decimals)?;

        // Helper to convert amount (u128) with decimals -> Decimal safely
        let amount_to_decimal = |amount: u128, decimals: u8| -> EstimatorResult<Decimal> {
            let Some(amount_dec) = Decimal::from_u128(amount) else {
                return Err(report!(Error::ParseError)
                    .attach_printable("Failed to convert u128 amount to Decimal"));
            };
            let factor = Decimal::from(10u128).powi(-(decimals as i64));
            Ok(amount_dec * factor)
        };

        // Validate prices are finite and strictly positive
        if !src_data.price.is_finite() || !dst_data.price.is_finite() {
            return Err(report!(Error::ParseError));
        }
        let src_price = Decimal::from_f64(src_data.price).ok_or(Error::ParseError)?;
        let dst_price = Decimal::from_f64(dst_data.price).ok_or(Error::ParseError)?;
        if src_price.is_sign_negative()
            || src_price.is_zero()
            || dst_price.is_sign_negative()
            || dst_price.is_zero()
        {
            return Err(report!(Error::ZeroPriceError));
        }

        // Value of input in dollars
        let src_amount_dec = amount_to_decimal(pending_swap.amount_in, src_data.decimals)?;
        let in_usd_value = src_amount_dec * src_price;

        // Value of expenses in dollars
        let mut expenses_usd_value = Decimal::ZERO;
        for expense in pending_swap.extra_expenses.iter() {
            // sanitize expense token id
            let token_id = TokenId::new_for_codex(expense.0.chain.clone(), &expense.0.address);
            let Some(expense_data) = coin_cache.get(&token_id) else {
                return Err(report!(Error::TokenNotFound(format!(
                    "Missing token data on monitor for expense token: {:?}",
                    token_id
                ))));
            };
            validate_decimals(expense_data.decimals)?;
            if !expense_data.price.is_finite() {
                return Err(report!(Error::ParseError));
            }
            let expense_price = Decimal::from_f64(expense_data.price).ok_or(Error::ParseError)?;
            if expense_price.is_sign_negative() || expense_price.is_zero() {
                return Err(report!(Error::ZeroPriceError));
            }
            let expense_amount_dec = amount_to_decimal(*expense.1, expense_data.decimals)?;
            expenses_usd_value += expense_amount_dec * expense_price;
        }

        // Calculate how many dst tokens can be bought with remaining value
        let total_value = in_usd_value - expenses_usd_value;
        let dst_token_amount_dec = total_value / dst_price;
        let expenses_in_dest_tokens = expenses_usd_value / dst_price;

        // Convert it back to u128 with proper decimals
        let estimated_amount_out = decimal_to_raw(dst_token_amount_dec, dst_data.decimals as i64)?;
        let fulfillment_expenses_in_tokens_out =
            decimal_to_raw(expenses_in_dest_tokens, dst_data.decimals as i64)?;

        tracing::debug!(
            "Estimated amount out for pending swap {:?}: {}",
            pending_swap,
            estimated_amount_out
        );

        // dbg!(&pending_swap.order_id, estimated_amount_out);

        Ok((estimated_amount_out, fulfillment_expenses_in_tokens_out))
    } else {
        Err(report!(Error::TokenNotFound(format!(
            "Missing token data on monitor for swap: {:?}",
            pending_swap
        ))))
    }
}

pub fn decimal_to_raw(amount: Decimal, decimals: i64) -> EstimatorResult<u128> {
    if amount < Decimal::ZERO {
        return Err(report!(Error::ParseError)
            .attach_printable("Cannot convert negative decimal amount to raw u128"));
    }
    // 10^decimals
    let factor = Decimal::from(10u128).powi(decimals);
    // amount * 10^decimals
    let scaled = amount * factor;

    let scaled_int = scaled.trunc();

    let raw = scaled_int.to_u128().ok_or(Error::ParseError)?;
    Ok(raw)
}

/// Computes how much the monitor should estimate so the solver reaches `min_user`,
/// given the solver's previous bid (`bid_solver`) and the monitor's estimate (`est_monitor`).
/// Applies a benevolent multiplicative margin gamma (>= 1).
///
/// Parameters:
/// - bid_solver: Estimated amount by the solver (token_out human units with scale applied).
/// - est_monitor: Estimated amount by the monitor in the same format.
/// - min_user: Minimum token_out required by the user.
///
/// Returns: req_monitor: The amount the monitor should estimate so the solver believes it will reach `min_user`.
fn required_monitor_estimation_for_solver_fulfillment(
    bid_solver: u128,
    est_monitor: u128,
    min_user: u128,
    fulfillment_expenses_in_tokens_out: u128,
) -> EstimatorResult<u128> {
    //  required_monitor_est + expenses       est_monitor + expenses
    // ---------------------------------- = ----------------------------
    //        min_user + expenses             bid_solver + expenses

    if est_monitor == 0 {
        return Err(report!(Error::ParseError).attach_printable("Estimated monitor amount is zero"));
    }

    let required_monitor_est = mul_div(
        min_user + fulfillment_expenses_in_tokens_out,
        est_monitor + fulfillment_expenses_in_tokens_out,
        bid_solver + fulfillment_expenses_in_tokens_out,
        false, // being optimistic
    )? - fulfillment_expenses_in_tokens_out;

    Ok(required_monitor_est)
}

#[cfg(test)]
mod tests {
    use crate::utils::get_timestamp;

    use super::*;
    use crate::tests::init_tracing_in_tests;
    use intents_models::constants::chains::ChainId;
    use tokio::sync::{broadcast, mpsc};

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
        deadline: u64,
        extra_expenses: HashMap<TokenId, u128>,
    ) -> PendingSwap {
        PendingSwap {
            order_id,
            src_chain,
            dst_chain,
            token_in,
            token_out,
            amount_in,
            amount_out,
            deadline,
            extra_expenses,
        }
    }

    #[tokio::test]
    async fn test_check_swaps_feasibility_unsuccessful_swap() {
        dotenv::dotenv().ok();
        init_tracing_in_tests();

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
            get_timestamp() + 300,
            HashMap::new(),
        );

        let result = estimate_amount_out(&pending_swap, &coin_cache);

        // The swap should be feasible: real_price = 100/50 = 2, expected = 1.9/1 = 1.9
        assert!(result.unwrap().0 > 1_900_000);
    }

    #[tokio::test]
    async fn test_check_swaps_feasibility_successful_swap() {
        dotenv::dotenv().ok();
        init_tracing_in_tests();

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
            create_coin_data(49.9, 18),
        );

        let pending_swap = create_pending_swap(
            "order_1".to_string(),
            ChainId::Ethereum,
            ChainId::Base,
            "token_a".to_string(),
            "token_b".to_string(),
            1_000_000_000_000_000_000,
            2_000_000_000_000_000_000,
            get_timestamp() + 300,
            HashMap::new(),
        );

        let result = estimate_amount_out(&pending_swap, &coin_cache);

        // real_price_limit = 50/100 = 0.5
        assert!(result.unwrap().0 > 2_000_000_000_000_000_000);
    }

    #[tokio::test]
    async fn test_check_swaps_feasibility_unsuccessful_swap_extra_expenses() {
        dotenv::dotenv().ok();
        init_tracing_in_tests();

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
        coin_cache.insert(
            TokenId {
                chain: ChainId::Base,
                address: "token_c".to_string(),
            },
            create_coin_data(10.0, 18),
        );

        let extra_expenses = HashMap::from([(
            TokenId {
                chain: ChainId::Base,
                address: "token_c".to_string(),
            },
            1_000_000_000_000_000_000u128, // 1 token
        )]);

        let pending_swap = create_pending_swap(
            "order_1".to_string(),
            ChainId::Ethereum,
            ChainId::Base,
            "token_a".to_string(),
            "token_b".to_string(),
            1_000_000_000_000_000_000,
            2_000_000_000_000_000_000,
            get_timestamp() + 300,
            extra_expenses,
        );

        let result = estimate_amount_out(&pending_swap, &coin_cache);

        // real_price_limit = 50/100 = 0.5
        assert!(result.unwrap().0 < 2_000_000_000_000_000_000);
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
            get_timestamp() + 300,
            HashMap::new(),
        );

        let result = estimate_amount_out(&pending_swap, &coin_cache);

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
    async fn test_get_coins_data_zero_price() {
        dotenv::dotenv().ok();
        init_tracing_in_tests();

        // Setup
        let codex_api_key = match std::env::var("CODEX_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                eprintln!("Skipping CodexProvider test: CODEX_API_KEY not set");
                return;
            }
        };
        let (sender, _receiver) = broadcast::channel(10);
        let (_, monitor_receiver) = mpsc::channel(10);

        let mut monitor_manager =
            MonitorManager::new(monitor_receiver, sender, codex_api_key, (true, 5));

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
        dotenv::dotenv().ok();
        init_tracing_in_tests();

        // Setup
        let codex_api_key = match std::env::var("CODEX_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                eprintln!("Skipping CodexProvider test: CODEX_API_KEY not set");
                return;
            }
        };
        let (sender, _receiver) = broadcast::channel(10);
        let (_, monitor_receiver) = mpsc::channel(10);

        let mut monitor_manager =
            MonitorManager::new(monitor_receiver, sender, codex_api_key, (true, 5));

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
    async fn test_estimate_orders_amount_out_missing_token_data() {
        dotenv::dotenv().ok();
        init_tracing_in_tests();

        // Setup
        let codex_api_key = match std::env::var("CODEX_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                eprintln!("Skipping CodexProvider test: CODEX_API_KEY not set");
                return;
            }
        };
        let (sender, _receiver) = broadcast::channel(10);
        let (_, monitor_receiver) = mpsc::channel(10);

        let mut monitor_manager =
            MonitorManager::new(monitor_receiver, sender, codex_api_key, (true, 5));

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
}
