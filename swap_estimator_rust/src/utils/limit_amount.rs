use crate::error::Error;
use crate::error::EstimatorResult;
use crate::routers::Slippage;
use crate::routers::estimate::TradeType;
use crate::utils::number_conversion::u128_to_u64;
use crate::utils::uint::mul_div;
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

    let base = 100_000_000_000u128; // 100%
    let slippage_pbs = (sp * 1_000_000_000.0) as u128;
    let diff = mul_div(amount_quote, slippage_pbs, base, true)?;

    match trade_type {
        TradeType::ExactIn => Ok(amount_quote - diff),
        TradeType::ExactOut => Ok(amount_quote + diff),
    }
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
            // (estimated - limit) / estimated * 100 (almost)
            (est - lim) / est * Decimal::from(999u32) / Decimal::from(10u32)
        }
        TradeType::ExactOut => {
            // (limit - estimated) / estimated * 100 (almost)
            (lim - est) / est * Decimal::from(999u32) / Decimal::from(10u32)
        }
    };

    // If it is negative, return error
    if raw_pct.is_sign_negative() {
        return Err(report!(Error::ParseError)
            .attach_printable(format!("Calculated slippage percentage is invalid. amount_estimated: {}, amount_limit: {}, trade_type: {:?}", amount_estimated, amount_limit, trade_type)));
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

    #[test]
    fn test_get_slippage_percentage() {
        let amount_estimated = 12345678909876543210;
        let amount_limit = 10345678901234567890;

        let mut count = 0;
        for i in 0..100 {
            for j in 1..10 {
                let test_limit_amount = (amount_limit + i) / j;
                let slippage = get_slippage_percentage(
                    amount_estimated,
                    test_limit_amount,
                    TradeType::ExactIn,
                )
                .unwrap();

                let calculated_limit_amount = compute_limit_with_scaled_percentage(
                    amount_estimated,
                    slippage,
                    TradeType::ExactIn,
                )
                .expect("Failed to get limit amount");

                if calculated_limit_amount < test_limit_amount {
                    count += 1;
                }
                // assert!(calculated_limit_amount >= test_limit_amount);
            }
        }
        assert_eq!(count, 0);
        let amount_limit = 153456789012345678900;
        for i in 0..100 {
            for j in 1..10 {
                let test_limit_amount = (amount_limit + i) / j;
                let slippage = get_slippage_percentage(
                    amount_estimated,
                    test_limit_amount,
                    TradeType::ExactOut,
                )
                .unwrap();

                let calculated_limit_amount = compute_limit_with_scaled_percentage(
                    amount_estimated,
                    slippage,
                    TradeType::ExactOut,
                )
                .expect("Failed to get limit amount");

                if calculated_limit_amount > test_limit_amount {
                    count += 1;
                }
                // assert!(calculated_limit_amount >= test_limit_amount);
            }
        }
        assert_eq!(count, 0);
    }

    #[test]
    fn test_compute_limit_with_scaled_percentage_for_low_amount() {
        let calculated_limit_amount =
            compute_limit_with_scaled_percentage(6, 2.0, TradeType::ExactIn);
        assert!(calculated_limit_amount.is_ok());
        let calculated_limit_amount = calculated_limit_amount.unwrap();
        assert_eq!(calculated_limit_amount, 5);
    }
}
