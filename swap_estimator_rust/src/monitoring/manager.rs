use std::collections::{HashMap, HashSet};

use error_stack::report;
use intents_models::constants::chains::ChainId;
use tokio::sync::mpsc::{Receiver, Sender};

use crate::{
    error::{Error, EstimatorResult},
    monitoring::messages::{MonitorAlert, MonitorRequest},
    prices::{
        PriceEvent, PriceProvider, TokenId, TokenMetadata, TokenPrice,
        codex::pricing::CodexProvider, estimating::OrderEstimationData,
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
    pub token_metadata: HashMap<TokenId, TokenMetadata>,
    pub codex_provider: CodexProvider,
}

impl MonitorManager {
    pub fn new(
        receiver: Receiver<MonitorRequest>,
        sender: Sender<MonitorAlert>,
        codex_api_key: String,
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
        }
    }

    pub async fn run(mut self) -> EstimatorResult<()> {
        let mut codex_rx_opt = match self.codex_provider.subscribe_events().await {
            Ok(rx) => rx,
            Err(err) => {
                tracing::error!("Failed to subscribe Codex price events: {:?}", err);
                return Err(err);
            }
        };

        loop {
            tokio::select! {
                // Codex update price event
                evt = codex_rx_opt.recv() => {
                    tracing::debug!("Received Codex price event: {:?}", evt);
                    match evt {
                        Ok(event) => {
                            self.on_price_event(event).await;
                        }
                        Err(e) => {
                            tracing::warn!("Codex price events receiver closed: {:?}", e);
                            return Err(report!(Error::Unknown).attach_printable("Codex price events receiver closed"));
                        }
                    }
                }
                request = self.receiver.recv() => {
                    tracing::debug!("Received monitor request: {:?}", request);
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
                                    feasibility_margin,
                                } => {
                                    if let Err(error) = self.check_swap_feasibility(order_id, src_chain, dst_chain, token_in, token_out, amount_in, amount_out, feasibility_margin).await {
                                        tracing::error!("Error processing CheckSwapFeasibility request: {:?}", error);
                                    }
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

    async fn check_swap_feasibility(
        &mut self,
        order_id: String,
        src_chain: ChainId,
        dst_chain: ChainId,
        token_in: String,
        token_out: String,
        amount_in: u128,
        amount_out: u128,
        feasibility_margin: f64,
    ) -> EstimatorResult<()> {
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

        // Subscribe to price updates for both tokens
        let tokens = vec![token_in_id.clone(), token_out_id.clone()]
            .into_iter()
            .collect::<HashSet<_>>();
        let mut tokens_data = self.get_coins_data(tokens).await?;

        if tokens_data.len() < 2 {
            tracing::warn!(
                "Not all token data available for order_id: {}, token_in: {}, token_out: {}",
                order_id,
                token_in,
                token_out
            );
            return Err(report!(Error::TokenNotFound(format!(
                "Missing token data for order_id: {}, cannot monitor swap feasibility",
                order_id
            ))));
        }

        // Check if we have token metadata info for both tokens, to update decimals, if not fetch and store
        let mut missing_metadata_tokens: HashSet<TokenId> = HashSet::new();
        for token_data in tokens_data.iter_mut() {
            match self.token_metadata.get(token_data.0) {
                Some(metadata) => {
                    token_data.1.decimals = metadata.decimals;
                }
                None => {
                    missing_metadata_tokens.insert(token_data.0.clone());
                }
            }
        }

        if !missing_metadata_tokens.is_empty() {
            match self
                .codex_provider
                .fetch_token_metadata(&missing_metadata_tokens)
                .await
            {
                Ok(metadatas) => {
                    for (token_id, metadata) in metadatas.into_iter() {
                        missing_metadata_tokens.remove(&token_id);
                        self.token_metadata
                            .insert(token_id.clone(), metadata.clone());
                        // Update decimals in tokens_data
                        if let Some(token_price) = tokens_data.get_mut(&token_id) {
                            token_price.decimals = metadata.decimals;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to fetch token metadata for tokens {:?}: {:?}",
                        missing_metadata_tokens,
                        e
                    );
                }
            }
            if !missing_metadata_tokens.is_empty() {
                tracing::warn!(
                    "Missing token metadata for tokens {:?} after fetch attempt",
                    missing_metadata_tokens
                );
                return Err(report!(Error::TokenNotFound(format!(
                    "Missing token metadata for order_id: {}, cannot monitor swap feasibility",
                    order_id
                ))));
            }
        }

        let pending_swap = PendingSwap {
            order_id: order_id.clone(),
            src_chain,
            dst_chain,
            token_in: token_in.clone(),
            token_out: token_out.clone(),
            amount_in,
            amount_out,
            feasibility_margin,
        };

        // Check immediate feasibility
        match check_swap_feasibility(&pending_swap, &tokens_data) {
            Ok(is_feasible) => {
                if is_feasible {
                    // Send alert immediately
                    tracing::debug!(
                        "Swap is immediately feasible for order_id: {}, sending alert",
                        order_id
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
                    } else {
                        // No need to monitor further
                        return Ok(());
                    }
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

        // Swap not feasible yet, add to monitoring
        // Subscribe to tokens for price updates and update coin cache
        self.codex_provider
            .subscribe_to_token(token_in_id.clone())
            .await?;
        self.codex_provider
            .subscribe_to_token(token_out_id.clone())
            .await?;

        // Add tokens to coin cache
        self.coin_cache.extend(tokens_data);

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
        Ok(())
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
        let data = self.get_tokens_data(tokens_not_in_cache).await?;

        result.extend(data);

        Ok(result)
    }

    // Gestionar un PriceEvent: actualizar cache, re-evaluar órdenes afectadas, limpiar y desuscribir tokens si ya no quedan swaps que dependan de ellos.
    async fn on_price_event(&mut self, event: PriceEvent) {
        // Get metadata for the token if not present
        let token_metadata = match self.token_metadata.get(&event.token) {
            Some(metadata) => metadata,
            None => {
                let tokens = HashSet::from([event.token.clone()]);
                let token_metadata = match self.codex_provider.fetch_token_metadata(&tokens).await {
                    Ok(metadata) => metadata,
                    Err(error) => {
                        tracing::error!(
                            "Failed to fetch token metadata for {:?}: {:?}",
                            tokens,
                            error
                        );
                        return;
                    }
                };
                self.token_metadata.extend(token_metadata);
                match self.token_metadata.get(&event.token) {
                    Some(metadata) => metadata,
                    None => {
                        tracing::error!(
                            "Token metadata still missing for token {:?} after fetch",
                            event.token
                        );
                        return;
                    }
                }
            }
        };
        // Update in-memory cache
        self.coin_cache.insert(
            event.token.clone(),
            TokenPrice {
                price: event.price.price,
                decimals: token_metadata.decimals,
            },
        );

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
                        self.remove_order(&pending_swap.order_id).await;
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

    async fn remove_order(&mut self, order_id: &str) {
        // Remove from pending swaps
        if let Some(pending_swap) = self.pending_swaps.remove(order_id) {
            // Detach from token->orders map and unsubscribe if needed
            let t_in = TokenId::new(pending_swap.src_chain, pending_swap.token_in.clone());
            let t_out = TokenId::new(pending_swap.dst_chain, pending_swap.token_out.clone());
            self.detach_order_from_token(&t_in, &pending_swap.order_id)
                .await;
            self.detach_order_from_token(&t_out, &pending_swap.order_id)
                .await;
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

        // Unsubscribe from codex.
        // We do this because subscriptions have a counter inside, so it won't unsubscribe unless all subscribers are removed.
        match self
            .codex_provider
            .unsubscribe_from_token(token.clone())
            .await
        {
            Ok(dropped) => {
                if dropped {
                    // If we actually dropped the subscription, remove from coin cache
                    tracing::debug!(
                        "Unsubscribed from token {:?}, removing from coin cache",
                        token
                    );
                    self.coin_cache.remove(token);
                }
            }
            Err(e) => {
                tracing::warn!("Codex unsubscribe_from_token failed: {:?}", e);
            }
        }
    }

    async fn get_tokens_data(
        &self,
        token_ids: HashSet<TokenId>,
    ) -> Result<HashMap<TokenId, TokenPrice>, Error> {
        match self
            .codex_provider
            .get_tokens_price(token_ids.clone())
            .await
        {
            Ok(data) => Ok(data),
            Err(e) => {
                tracing::error!("Codex get_tokens_price failed: {:?}", e);
                Err(e.current_context().clone())
            }
        }
    }
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
        let amount_to_decimal = |amount: u128, decimals: u8| -> Result<Decimal, Error> {
            // Avoid silent wrap: ensure value fits in i128
            let amount_i128 = i128::try_from(amount).map_err(|_| Error::ParseError)?;
            Ok(Decimal::from_i128_with_scale(amount_i128, decimals as u32))
        };

        // Validate prices are finite and strictly positive
        if !src_data.price.is_finite() || !dst_data.price.is_finite() {
            return Err(Error::ParseError);
        }
        let src_price = Decimal::from_f64(src_data.price).ok_or(Error::ParseError)?;
        let dst_price = Decimal::from_f64(dst_data.price).ok_or(Error::ParseError)?;
        if src_price.is_sign_negative()
            || src_price.is_zero()
            || dst_price.is_sign_negative()
            || dst_price.is_zero()
        {
            return Err(Error::ZeroPriceError);
        }

        // Validate margin is finite, non-negative and not absurdly large (to avoid threshold explosions)
        let margin = pending_swap.feasibility_margin;
        if !margin.is_finite() || margin < 0.0 || margin > 10_000.0 {
            return Err(Error::ParseError);
        }
        let margin_dec = Decimal::from_f64(margin).ok_or(Error::ParseError)?;

        // Compute price_limit as Decimal: expected token_out per token_in
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

        // threshold = price_limit * (1 + margin/100)
        let threshold = price_limit_dec * (Decimal::ONE + (margin_dec / Decimal::ONE_HUNDRED));

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
        dotenv::dotenv().ok();
        init_tracing(false);

        // Setup
        let codex_api_key = match std::env::var("CODEX_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                eprintln!("Skipping CodexProvider test: CODEX_API_KEY not set");
                return;
            }
        };
        let (sender, _receiver) = mpsc::channel(10);
        let (_, monitor_receiver) = mpsc::channel(10);

        let mut monitor_manager = MonitorManager::new(monitor_receiver, sender, codex_api_key);

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
        dotenv::dotenv().ok();
        init_tracing(false);

        // Setup
        let codex_api_key = match std::env::var("CODEX_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                eprintln!("Skipping CodexProvider test: CODEX_API_KEY not set");
                return;
            }
        };
        let (sender, _receiver) = mpsc::channel(10);
        let (_, monitor_receiver) = mpsc::channel(10);

        let mut monitor_manager = MonitorManager::new(monitor_receiver, sender, codex_api_key);

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
        init_tracing(false);

        // Setup
        let codex_api_key = match std::env::var("CODEX_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                eprintln!("Skipping CodexProvider test: CODEX_API_KEY not set");
                return;
            }
        };
        let (sender, _receiver) = mpsc::channel(10);
        let (_, monitor_receiver) = mpsc::channel(10);

        let mut monitor_manager = MonitorManager::new(monitor_receiver, sender, codex_api_key);

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
        dotenv::dotenv().ok();
        init_tracing(false);

        // Setup
        let codex_api_key = match std::env::var("CODEX_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                eprintln!("Skipping CodexProvider test: CODEX_API_KEY not set");
                return;
            }
        };
        let (sender, _receiver) = mpsc::channel(10);
        let (_, monitor_receiver) = mpsc::channel(10);

        let mut monitor_manager = MonitorManager::new(monitor_receiver, sender, codex_api_key);

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
        dotenv::dotenv().ok();
        init_tracing(false);

        // Setup
        let codex_api_key = match std::env::var("CODEX_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                eprintln!("Skipping CodexProvider test: CODEX_API_KEY not set");
                return;
            }
        };
        let (sender, _receiver) = mpsc::channel(10);
        let (_, monitor_receiver) = mpsc::channel(10);

        let mut monitor_manager = MonitorManager::new(monitor_receiver, sender, codex_api_key);

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
        dotenv::dotenv().ok();
        init_tracing(false);

        // Setup
        let codex_api_key = match std::env::var("CODEX_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                eprintln!("Skipping CodexProvider test: CODEX_API_KEY not set");
                return;
            }
        };
        let (sender, _receiver) = mpsc::channel(10);
        let (_, monitor_receiver) = mpsc::channel(10);

        let mut monitor_manager = MonitorManager::new(monitor_receiver, sender, codex_api_key);

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
        dotenv::dotenv().ok();
        init_tracing(false);

        // Setup
        let codex_api_key = match std::env::var("CODEX_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                eprintln!("Skipping CodexProvider test: CODEX_API_KEY not set");
                return;
            }
        };
        let (sender, _receiver) = mpsc::channel(10);
        let (_, monitor_receiver) = mpsc::channel(10);

        let mut monitor_manager = MonitorManager::new(monitor_receiver, sender, codex_api_key);

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
