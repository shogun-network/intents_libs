use crate::error::Error;
use crate::error::EstimatorResult;
use crate::routers::Slippage;
use crate::routers::estimate::TradeType;
use crate::utils::number_conversion::u128_to_u64;
use error_stack::report;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;

pub fn get_limit_amount(
    trade_type: TradeType,
    amount_quote: u128,
    slippage: Slippage,
) -> EstimatorResult<u128> {
    Ok(match slippage {
        Slippage::Percent(slippage) => {
            compute_limit_with_scaled_percentage(amount_quote, slippage, trade_type)?
        }
        Slippage::AmountLimit {
            amount_limit,
            fallback_slippage: _,
        } => amount_limit,
        Slippage::MaxSlippage => match trade_type {
            TradeType::ExactIn => 0,
            TradeType::ExactOut => u128::MAX,
        },
    })
}

pub fn get_limit_amount_u64(
    trade_type: TradeType,
    amount_quote: u64,
    slippage: Slippage,
) -> EstimatorResult<u64> {
    Ok(match slippage {
        Slippage::Percent(slippage) => u128_to_u64(
            compute_limit_with_scaled_percentage(amount_quote as u128, slippage, trade_type)?,
            "limit_amount",
        )?,
        Slippage::AmountLimit {
            amount_limit,
            fallback_slippage: _,
        } => u128_to_u64(amount_limit, "amount_limit")?,
        Slippage::MaxSlippage => match trade_type {
            TradeType::ExactIn => 0,
            TradeType::ExactOut => u64::MAX,
        },
    })
}

fn compute_limit_with_scaled_percentage(
    amount_quote: u128,
    slippage_percent: f64,
    trade_type: TradeType,
) -> EstimatorResult<u128> {
    // Guardrails for invalid values
    let sp = if !slippage_percent.is_finite() {
        return Err(
            report!(Error::ParseError).attach_printable("Slippage percentage is not finite")
        );
    } else if slippage_percent < 0.0 {
        return Err(report!(Error::ParseError).attach_printable("Slippage percentage is negative"));
    } else {
        slippage_percent
    };

    // For ExactIn, 100%+ slippage means min out is below zero
    if matches!(trade_type, TradeType::ExactIn) && sp > 100.0 {
        return Err(report!(Error::ParseError)
            .attach_printable("Slippage percentage too high, results in zero limit amount"));
    }

    // Try high-precision scales first, then degrade to avoid overflow
    const SCALES: [u128; 5] = [1_000_000_000, 1_000_000, 10_000, 100, 1];

    for scale in SCALES {
        // Convert percentage to integer with chosen scale, rounded
        let p_scaled = (sp * scale as f64).round() as u128;

        // Build fraction: numerator/denominator = (100 ± p)/100 using the scale
        // ExactIn: amount_out_min = amount_quote * (100 - p)/100
        // ExactOut: amount_in_max = amount_quote * (100 + p)/100
        let hundred_scaled = 100u128.checked_mul(scale).unwrap_or(u128::MAX); // safe cap

        let (num, den) = match trade_type {
            TradeType::ExactIn => (hundred_scaled.saturating_sub(p_scaled), hundred_scaled),
            TradeType::ExactOut => (hundred_scaled.saturating_add(p_scaled), hundred_scaled),
        };

        // Reduce fraction to minimize overflow
        let g = gcd_u128(num, den);
        let n = num / g;
        let d = den / g;

        // Prefer dividing first to reduce magnitude, then multiply
        let a_div = amount_quote / d;
        if let Some(res) = a_div.checked_mul(n) {
            return Ok(res);
        }

        // If dividing first loses too much precision (a_div == 0), try mul then div
        if let Some(tmp) = amount_quote.checked_mul(n) {
            return Ok(tmp / d);
        }
    }

    Err(report!(Error::ParseError).attach_printable("Unable to compute limit amount without overflow"))
}

#[inline]
fn gcd_u128(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}

pub fn get_slippage_percentage(
    amount_estimated: u128,
    amount_limit: u128,
    trade_type: TradeType,
) -> EstimatorResult<f64> {
    // Convertir los u128 a Decimal
    let est = Decimal::from(amount_estimated);
    if est.is_zero() {
        return Ok(0.0);
    }
    let lim = Decimal::from(amount_limit);

    let raw_pct = match trade_type {
        TradeType::ExactIn => {
            // (estimated - limit) / estimated * 100
            (est - lim) / est * Decimal::from(100u32)
        }
        TradeType::ExactOut => {
            // (limit - estimated) / estimated * 100
            (lim - est) / est * Decimal::from(100u32)
        }
    };

    // If it is negative, return error
    if raw_pct.is_sign_negative() {
        return Err(report!(Error::ParseError)
            .attach_printable("Calculated slippage percentage is negative"));
    }

    raw_pct
        .round_dp(6) // Round to 6 decimal places to avoid floating point precision issues
        .to_f64()
        .ok_or(report!(Error::ParseError).attach_printable("Error calculating slippage percentage"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_limit_amount() {
        let limit_amount = get_limit_amount(TradeType::ExactIn, 1000, Slippage::Percent(2.0))
            .expect("Failed to get limit amount");
        assert_eq!(limit_amount, 980);
        let limit_amount = get_limit_amount(TradeType::ExactOut, 1000, Slippage::Percent(2.0))
            .expect("Failed to get limit amount");
        assert_eq!(limit_amount, 1020);
    }

    #[test]
    fn test_get_limit_amount_u64() {
        let limit_amount = get_limit_amount_u64(TradeType::ExactIn, 1000, Slippage::Percent(2.0))
            .expect("Failed to get limit amount");
        assert_eq!(limit_amount, 980);
        let limit_amount = get_limit_amount_u64(TradeType::ExactOut, 1000, Slippage::Percent(2.0))
            .expect("Failed to get limit amount");
        assert_eq!(limit_amount, 1020);
    }
}
