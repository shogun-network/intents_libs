use crate::error::Error;
use crate::error::EstimatorResult;
use crate::routers::Slippage;
use crate::routers::estimate::{GenericEstimateRequest, GenericEstimateResponse, TradeType};
use crate::utils::limit_amount::get_limit_amount;
use error_stack::report;

/// We'll be adding 0.1 % on the top of initial quote to try to compensate swap fees
const INIT_MULTIPLIER_BASE: u128 = 10_000;
/// This may be adjusted in order to increase chances
const INIT_MULTIPLIER: u128 = 10_010;

/// This is 100%
const THRESHOLD_BASE: u128 = 10_000;
/// If result is within 0.2% threshold - we count it as success
/// The lower this value - the more attempts it may take
const SUCCESS_THRESHOLD_BPS: u128 = 20;

/// If we could not adjust amounts in 3 attempts - something's very wrong
const MAX_LOOP_ATTEMPTS: usize = 3;

/// Tries to find such exact IN quote for given exact OUT quote, that
/// `amount_limit` of resulting exact IN quote be as close as possible to
/// `amount_fixed` of given quote
///
/// ### Arguments
///
/// * `quote_request` - Exact OUT quote request
/// * `quote_fn` - Function to use for exact IN quotes
///
/// ### Returns
///
/// * Estimate response
/// * Number of attempts, that estimation took. We consider 1st exact_in quote to be 1st attempt
pub async fn quote_exact_out_with_exact_in<F, Fut>(
    quote_request: GenericEstimateRequest,
    quote_fn: F,
) -> EstimatorResult<(GenericEstimateResponse, usize)>
where
    F: Fn(GenericEstimateRequest) -> Fut + Send + Sync,
    Fut: Future<Output = EstimatorResult<GenericEstimateResponse>> + Send,
{
    // Let's say we need to know how much to spend ETH to get 3500 USDC
    // The approach will be:
    // 1. Quote quote_exact_in(3500 USDC -> ETH).
    //      Let's say result will be 0.99 ETH
    // 2. Increase that amount a bit - lets say to 1 ETH
    // 3. quote_exact_in(1 ETH -> USDC)
    // 3.1. If result is just a bit above 3500 USDC - we found it!
    // 3.2. If it's lower or much higher - adjust amount IN proportionally and retry in the loop

    if quote_request.trade_type != TradeType::ExactOut {
        return Err(report!(Error::LogicError(
            "ExactOut trade must be passed to quote_exact_out_with_exact_in".to_string()
        )));
    }

    let (slippage_percent, max_amount_in) = match quote_request.slippage {
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

    let target_min_amount_out = quote_request.amount_fixed;
    let target_max_amount_out =
        target_min_amount_out * (THRESHOLD_BASE + SUCCESS_THRESHOLD_BPS) / THRESHOLD_BASE;

    let exact_in_request = GenericEstimateRequest {
        trade_type: TradeType::ExactIn,
        chain_id: quote_request.chain_id,
        src_token: quote_request.dest_token.clone(),
        dest_token: quote_request.src_token.clone(),
        amount_fixed: quote_request.amount_fixed,
        slippage: Slippage::Percent(slippage_percent),
    };

    let quote_response = quote_fn(exact_in_request).await?;

    let test_amount_in = get_limit_amount(
        TradeType::ExactOut,
        // Increasing quote amount in attempt to compensate swap fees
        quote_response.amount_quote * INIT_MULTIPLIER / INIT_MULTIPLIER_BASE,
        Slippage::Percent(slippage_percent),
    )?;

    let mut try_values = TryExactInValues {
        test_amount_in,
        slippage_percent,
        target_min_amount_out,
        target_max_amount_out,
        max_amount_in,
    };

    let (quote_response, success) = try_exact_in(&quote_request, try_values, &quote_fn).await?;

    if success {
        return Ok((quote_response, 1));
    }

    let mut attempt_number = 0;
    let target_amount_out = (target_min_amount_out + target_max_amount_out) / 2;
    // Adjusting amount IN proportionally to amount_out_min
    try_values.test_amount_in =
        try_values.test_amount_in * target_amount_out / quote_response.amount_limit;
    while attempt_number < MAX_LOOP_ATTEMPTS {
        attempt_number += 1;
        let (quote_response, success) = try_exact_in(&quote_request, try_values, &quote_fn).await?;
        if success {
            return Ok((quote_response, attempt_number + 1));
        }
        // Adjusting amount IN proportionally to amount_out_min
        try_values.test_amount_in =
            try_values.test_amount_in * target_amount_out / quote_response.amount_limit;
    }

    Err(report!(Error::AggregatorError(format!(
        "Failed to estimate exact OUT with exact IN in {MAX_LOOP_ATTEMPTS} attempts"
    ))))
}

#[derive(Debug, Clone, Copy)]
struct TryExactInValues {
    pub test_amount_in: u128,
    pub slippage_percent: f64,
    pub target_min_amount_out: u128,
    pub target_max_amount_out: u128,
    pub max_amount_in: Option<u128>,
}

/// Tries to quote with exact amount IN
/// If `amount_limit` is within threshold - return success
///
/// ### Returns
///
/// * Estimate response
async fn try_exact_in<F, Fut>(
    quote_request: &GenericEstimateRequest,
    values: TryExactInValues,
    quote_fn: &F,
) -> EstimatorResult<(GenericEstimateResponse, bool)>
where
    F: Fn(GenericEstimateRequest) -> Fut + Send + Sync,
    Fut: Future<Output = EstimatorResult<GenericEstimateResponse>> + Send,
{
    let TryExactInValues {
        test_amount_in,
        slippage_percent,
        target_min_amount_out,
        target_max_amount_out,
        max_amount_in,
    } = values;

    let target_request = GenericEstimateRequest {
        trade_type: TradeType::ExactIn,
        chain_id: quote_request.chain_id,
        src_token: quote_request.src_token.clone(),
        dest_token: quote_request.dest_token.clone(),
        amount_fixed: test_amount_in,
        slippage: Slippage::Percent(slippage_percent),
    };

    let quote_response = quote_fn(target_request).await?;

    let success = if quote_response.amount_limit <= target_max_amount_out
        && quote_response.amount_limit >= target_min_amount_out
    {
        if let Some(max_amount_in) = max_amount_in
            && max_amount_in > test_amount_in
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
    use crate::routers::jupiter::jupiter::get_jupiter_quote;
    use intents_models::constants::chains::ChainId;

    #[tokio::test]
    async fn test_quote_exact_out_with_exact_in_jupiter() {
        dotenv::dotenv().ok();

        // Searching for amount of Sol to spend to receive at least 1 Million USDT
        let quote_request = GenericEstimateRequest {
            trade_type: TradeType::ExactOut,
            chain_id: ChainId::Solana,
            src_token: "So11111111111111111111111111111111111111112".to_string(), // SOL
            dest_token: "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB".to_string(), // USDT
            amount_fixed: 1_000_000_000,
            slippage: Slippage::Percent(2.0),
        };

        let jupiter_url = std::env::var("JUPITER_URL").unwrap();

        let res = quote_exact_out_with_exact_in(
            quote_request,
            async |generic_estimate_request: GenericEstimateRequest| {
                let res = get_jupiter_quote(&generic_estimate_request, &jupiter_url, None).await?;

                Ok(res.0)
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

    // todo test Slippage::AmountLimit
}
