use crate::error::Error;
use crate::error::EstimatorResult;
use crate::routers::Slippage;
use crate::routers::estimate::{GenericEstimateRequest, GenericEstimateResponse, TradeType};
use crate::routers::swap::{EvmSwapResponse, GenericSwapRequest};
use crate::utils::limit_amount::get_limit_amount;
use crate::utils::uint::mul_div;
use error_stack::report;
use std::fmt::Debug;

/// We'll be adding 0.1 % on the top of initial quote to try to compensate swap fees
const INIT_MULTIPLIER_BASE: u128 = 10_000;
/// This may be adjusted in order to increase chances
const INIT_MULTIPLIER: u128 = 10_010;

/// This is 100%
const THRESHOLD_BASE: u128 = 10_000;
/// If result is within 0.5% threshold - we count it as success
/// The lower this value - the more attempts it may take
const SUCCESS_THRESHOLD_BPS: u128 = 50;

/// If we could not adjust amounts in 3 attempts - something's very wrong
const MAX_LOOP_ATTEMPTS: usize = 3;

#[derive(Debug, Clone, Copy)]
struct TryExactInValues {
    pub test_amount_in: u128,
    pub slippage_percent: f64,
    pub target_min_amount_out: u128,
    pub target_max_amount_out: u128,
    pub max_amount_in: Option<u128>,
}

pub trait ReverseQuoteRequest {
    fn get_init_values(&self) -> (TradeType, Slippage, u128);
    fn get_reversed_exact_in_with_slippage(&self, slippage_percent: f64) -> Self;
    fn get_exact_in_with_slippage_and_amount_in(
        &self,
        slippage_percent: f64,
        amount_in: u128,
    ) -> Self;
}

impl ReverseQuoteRequest for GenericEstimateRequest {
    fn get_init_values(&self) -> (TradeType, Slippage, u128) {
        (self.trade_type, self.slippage, self.amount_fixed)
    }

    fn get_reversed_exact_in_with_slippage(&self, slippage_percent: f64) -> Self {
        Self {
            trade_type: TradeType::ExactIn,
            chain_id: self.chain_id,
            src_token: self.dest_token.clone(),
            dest_token: self.src_token.clone(),
            amount_fixed: self.amount_fixed,
            slippage: Slippage::Percent(slippage_percent),
        }
    }

    fn get_exact_in_with_slippage_and_amount_in(
        &self,
        slippage_percent: f64,
        amount_in: u128,
    ) -> Self {
        Self {
            trade_type: TradeType::ExactIn,
            chain_id: self.chain_id,
            src_token: self.src_token.clone(),
            dest_token: self.dest_token.clone(),
            amount_fixed: amount_in,
            slippage: Slippage::Percent(slippage_percent),
        }
    }
}

impl ReverseQuoteRequest for GenericSwapRequest {
    fn get_init_values(&self) -> (TradeType, Slippage, u128) {
        (self.trade_type, self.slippage, self.amount_fixed)
    }

    fn get_reversed_exact_in_with_slippage(&self, slippage_percent: f64) -> Self {
        Self {
            trade_type: TradeType::ExactIn,
            chain_id: self.chain_id,
            spender: self.spender.clone(),
            dest_address: self.dest_address.clone(),
            src_token: self.dest_token.clone(),
            dest_token: self.src_token.clone(),
            amount_fixed: self.amount_fixed,
            slippage: Slippage::Percent(slippage_percent),
        }
    }

    fn get_exact_in_with_slippage_and_amount_in(
        &self,
        slippage_percent: f64,
        amount_in: u128,
    ) -> Self {
        Self {
            trade_type: TradeType::ExactIn,
            chain_id: self.chain_id,
            spender: self.spender.clone(),
            dest_address: self.dest_address.clone(),
            src_token: self.src_token.clone(),
            dest_token: self.dest_token.clone(),
            amount_fixed: amount_in,
            slippage: Slippage::Percent(slippage_percent),
        }
    }
}

pub trait ReverseQuoteResponse {
    fn get_amount_quote(&self) -> u128;
    fn get_amount_limit(&self) -> u128;
    fn update_with_amount_in(&mut self, amount_in: u128);
}

impl ReverseQuoteResponse for GenericEstimateResponse {
    fn get_amount_quote(&self) -> u128 {
        self.amount_quote
    }
    fn get_amount_limit(&self) -> u128 {
        self.amount_limit
    }
    fn update_with_amount_in(&mut self, amount_in: u128) {
        self.amount_quote = amount_in;
        self.amount_limit = amount_in;
    }
}

impl ReverseQuoteResponse for EvmSwapResponse {
    fn get_amount_quote(&self) -> u128 {
        self.amount_quote
    }
    fn get_amount_limit(&self) -> u128 {
        self.amount_limit
    }
    fn update_with_amount_in(&mut self, amount_in: u128) {
        self.amount_quote = amount_in;
        self.amount_limit = amount_in;
    }
}

/// Tries to find such exact IN quote for given exact OUT quote, that
/// `amount_limit` of resulting exact IN quote be as close as possible to
/// `amount_fixed` of given quote
///
/// ### Arguments
///
/// * `request` - Exact OUT request
/// * `quote_fn` - Function to use for exact IN quotes
///
/// ### Returns
///
/// * Estimate response
/// * Number of attempts, that estimation took. We consider 1st exact_in quote to be 1st attempt
pub async fn quote_exact_out_with_exact_in<F, Fut, Request, Response>(
    request: Request,
    quote_exact_in_fn: F,
) -> EstimatorResult<(Response, usize)>
where
    Request: ReverseQuoteRequest + Debug,
    Response: ReverseQuoteResponse + Debug,
    F: Fn(Request) -> Fut + Send + Sync,
    Fut: Future<Output = EstimatorResult<Response>> + Send,
{
    // Let's say we need to know how much to spend ETH to get 3500 USDC
    // The approach will be:
    // 1. Quote quote_exact_in(3500 USDC -> ETH).
    //      Let's say result will be 0.99 ETH
    // 2. Increase that amount a bit - lets say to 1 ETH
    // 3. quote_exact_in(1 ETH -> USDC)
    // 3.1. If result is just a bit above 3500 USDC - we found it!
    // 3.2. If it's lower or much higher - adjust amount IN proportionally and retry in the loop

    let (requested_trade_type, requested_slippage, requested_amount_out) =
        request.get_init_values();

    if requested_trade_type != TradeType::ExactOut {
        return Err(report!(Error::LogicError(
            "ExactOut trade must be passed to quote_exact_out_with_exact_in".to_string()
        )));
    }

    let (slippage_percent, max_amount_in) = match requested_slippage {
        Slippage::Percent(slippage_percent) => (slippage_percent, None),
        Slippage::AmountLimit {
            amount_limit,
            fallback_slippage,
        } => (fallback_slippage, Some(amount_limit)),
        Slippage::MaxSlippage => {
            return Err(report!(Error::LogicError(
                "ExactOut trade does not support MaxSlippage".to_string()
            )));
        }
    };

    let target_min_amount_out = requested_amount_out;
    let target_max_amount_out = mul_div(
        target_min_amount_out,
        THRESHOLD_BASE + SUCCESS_THRESHOLD_BPS,
        THRESHOLD_BASE,
        true,
    )?;

    let exact_in_request = request.get_reversed_exact_in_with_slippage(slippage_percent);

    let quote_response = quote_exact_in_fn(exact_in_request).await?;

    let test_amount_in = get_limit_amount(
        TradeType::ExactOut,
        // Increasing quote amount in attempt to compensate swap fees
        mul_div(
            quote_response.get_amount_quote(),
            INIT_MULTIPLIER,
            INIT_MULTIPLIER_BASE,
            true,
        )?,
        Slippage::Percent(slippage_percent),
    )?;

    let mut try_values = TryExactInValues {
        test_amount_in,
        slippage_percent,
        target_min_amount_out,
        target_max_amount_out,
        max_amount_in,
    };

    let (mut quote_response, success) =
        try_exact_in(&request, try_values, &quote_exact_in_fn).await?;

    if success {
        quote_response.update_with_amount_in(try_values.test_amount_in);
        return Ok((quote_response, 1));
    }

    let mut attempt_number = 0;
    // Rounding up
    let target_amount_out = (target_min_amount_out + target_max_amount_out + 1) / 2;
    // Adjusting amount IN proportionally to amount_out_min
    try_values.test_amount_in = mul_div(
        try_values.test_amount_in,
        target_amount_out,
        quote_response.get_amount_limit(),
        target_amount_out > quote_response.get_amount_limit(),
    )?;
    while attempt_number < MAX_LOOP_ATTEMPTS {
        attempt_number += 1;
        let (mut quote_response, success) =
            try_exact_in(&request, try_values, &quote_exact_in_fn).await?;
        if success {
            quote_response.update_with_amount_in(try_values.test_amount_in);
            return Ok((quote_response, attempt_number + 1));
        }
        // Adjusting amount IN proportionally to amount_out_min
        try_values.test_amount_in = mul_div(
            try_values.test_amount_in,
            target_amount_out,
            quote_response.get_amount_limit(),
            target_amount_out > quote_response.get_amount_limit(),
        )?;
    }

    Err(report!(Error::AggregatorError(format!(
        "Failed to estimate exact OUT with exact IN in {MAX_LOOP_ATTEMPTS} attempts"
    ))))
}

/// Tries to quote with exact amount IN
/// If `amount_limit` is within threshold - return success
///
/// ### Returns
///
/// * Estimate response
async fn try_exact_in<F, Fut, Request, Response>(
    quote_request: &Request,
    values: TryExactInValues,
    quote_exact_in_fn: &F,
) -> EstimatorResult<(Response, bool)>
where
    Request: ReverseQuoteRequest,
    Response: ReverseQuoteResponse,
    F: Fn(Request) -> Fut + Send + Sync,
    Fut: Future<Output = EstimatorResult<Response>> + Send,
{
    let TryExactInValues {
        test_amount_in,
        slippage_percent,
        target_min_amount_out,
        target_max_amount_out,
        max_amount_in,
    } = values;

    let target_request =
        quote_request.get_exact_in_with_slippage_and_amount_in(slippage_percent, test_amount_in);

    let quote_response = quote_exact_in_fn(target_request).await?;

    let amount_limit = quote_response.get_amount_limit();
    let success = if amount_limit <= target_max_amount_out && amount_limit >= target_min_amount_out
    {
        if let Some(max_amount_in) = max_amount_in
            && test_amount_in > max_amount_in
        {
            return Err(report!(Error::AggregatorError(format!(
                "Estimated amount IN {test_amount_in} is above maximum requested {max_amount_in}"
            ))));
        }
        true
    } else {
        false
    };

    Ok((quote_response, success))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routers::RouterType;
    use intents_models::constants::chains::ChainId;

    async fn mock_jupiter_quote(
        generic_estimate_request: &GenericEstimateRequest,
    ) -> EstimatorResult<GenericEstimateResponse> {
        let slippage = match generic_estimate_request.slippage {
            Slippage::Percent(slippage) => slippage,
            Slippage::AmountLimit {
                fallback_slippage, ..
            } => fallback_slippage,
            Slippage::MaxSlippage => panic!("MaxSlippage not allowed"),
        };
        // Let's say SOL/USDT price is 150
        let amount_out = generic_estimate_request.amount_fixed
            // SOL (9 decimals) - USDT (6 decimals)
            * 1000
            // Dividing by price
            / 150
            // simulating swap expenses
            * 98
            / 100;

        Ok(GenericEstimateResponse {
            amount_quote: amount_out,
            amount_limit: get_limit_amount(
                TradeType::ExactIn,
                amount_out,
                Slippage::Percent(slippage),
            )?,
            router: RouterType::Jupiter,
            router_data: Default::default(),
        })
    }

    #[tokio::test]
    async fn test_quote_exact_out_with_exact_in() {
        // Searching for amount of Sol to spend to receive at least 1 Million USDT
        let quote_request = GenericEstimateRequest {
            trade_type: TradeType::ExactOut,
            chain_id: ChainId::Solana,
            src_token: "So11111111111111111111111111111111111111112".to_string(), // SOL
            dest_token: "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB".to_string(), // USDT
            amount_fixed: 1_000_000_000,
            slippage: Slippage::Percent(2.0),
        };

        let res = quote_exact_out_with_exact_in(
            quote_request,
            async |generic_estimate_request: GenericEstimateRequest| {
                let res = mock_jupiter_quote(&generic_estimate_request).await?;

                Ok(res)
            },
        )
        .await;
        assert!(
            res.is_ok(),
            "Expected successful quote response: {:?}",
            res.err()
        );

        let (_, attempts) = res.unwrap();
        println!("Success in {attempts} attempts");
        assert!(attempts >= 1 && attempts <= 2);
    }

    #[tokio::test]
    async fn test_quote_exact_out_with_exact_in_with_amount_limit() {
        // Searching for amount of Sol to spend to receive at least 1 Million USDT
        let quote_request = GenericEstimateRequest {
            trade_type: TradeType::ExactOut,
            chain_id: ChainId::Solana,
            src_token: "So11111111111111111111111111111111111111112".to_string(), // SOL
            dest_token: "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB".to_string(), // USDT
            amount_fixed: 1_000_000_000,
            slippage: Slippage::AmountLimit {
                amount_limit: 100_000_000_000, // Max 100 SOL to spend should be enough
                fallback_slippage: 2.0,
            },
        };

        let res = quote_exact_out_with_exact_in(
            quote_request,
            async |generic_estimate_request: GenericEstimateRequest| {
                let res = mock_jupiter_quote(&generic_estimate_request).await?;

                Ok(res)
            },
        )
        .await;
        assert!(
            res.is_ok(),
            "Expected successful quote response: {:?}",
            res.err()
        );

        let (_, attempts) = res.unwrap();
        println!("Success in {attempts} attempts");
        assert!(attempts >= 1 && attempts <= 2);
    }

    #[tokio::test]
    async fn test_quote_exact_out_with_exact_in_with_amount_limit_error() {
        // Searching for amount of Sol to spend to receive at least 1 Million USDT
        let quote_request = GenericEstimateRequest {
            trade_type: TradeType::ExactOut,
            chain_id: ChainId::Solana,
            src_token: "So11111111111111111111111111111111111111112".to_string(), // SOL
            dest_token: "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB".to_string(), // USDT
            amount_fixed: 1_000_000_000,
            slippage: Slippage::AmountLimit {
                amount_limit: 100_000_000, // Max 0.1 SOL to spend should not be enough
                fallback_slippage: 2.0,
            },
        };

        let res = quote_exact_out_with_exact_in(
            quote_request,
            async |generic_estimate_request: GenericEstimateRequest| {
                let res = mock_jupiter_quote(&generic_estimate_request).await?;

                Ok(res)
            },
        )
        .await;
        assert!(res.is_err());
    }
}
