use crate::error::Error;
use crate::error::EstimatorResult;
use crate::routers::Slippage;
use crate::routers::estimate::TradeType;
use crate::utils::number_conversion::u128_to_f64;
use crate::utils::number_conversion::u128_to_u64;
use error_stack::report;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;

pub fn get_limit_amount(trade_type: TradeType, amount_quote: u128, slippage: Slippage) -> u128 {
    match slippage {
        Slippage::Percent(slippage) => {
            match trade_type {
                // calculating amountOutMin
                TradeType::ExactIn => {
                    amount_quote * ((100f64 - slippage) * 1_000_000_000f64) as u128
                        / 100_000_000_000u128
                }
                // calculating amountInMax
                TradeType::ExactOut => {
                    amount_quote * ((100f64 + slippage) * 1_000_000_000f64) as u128
                        / 100_000_000_000u128
                }
            }
        }
        Slippage::AmountLimit {
            amount_limit,
            amount_estimated,
        } => amount_limit,
        Slippage::MaxSlippage => match trade_type {
            TradeType::ExactIn => 0,
            TradeType::ExactOut => u128::MAX,
        },
    }
}

pub fn get_limit_amount_u64(
    trade_type: TradeType,
    amount_quote: u64,
    slippage: Slippage,
) -> EstimatorResult<u64> {
    Ok(match slippage {
        Slippage::Percent(slippage) => {
            match trade_type {
                // calculating amountOutMin
                TradeType::ExactIn => {
                    amount_quote * ((100f64 - slippage) * 10_000f64) as u64 / 1_000_000u64
                }
                // calculating amountInMax
                TradeType::ExactOut => {
                    amount_quote * ((100f64 + slippage) * 10_000f64) as u64 / 1_000_000u64
                }
            }
        }
        Slippage::AmountLimit {
            amount_limit,
            amount_estimated: _,
        } => u128_to_u64(amount_limit, "amount_limit")?,
        Slippage::MaxSlippage => match trade_type {
            TradeType::ExactIn => 0,
            TradeType::ExactOut => u64::MAX,
        },
    })
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
        let limit_amount = get_limit_amount(TradeType::ExactIn, 1000, Slippage::Percent(2.0));
        assert_eq!(limit_amount, 980);
        let limit_amount = get_limit_amount(TradeType::ExactOut, 1000, Slippage::Percent(2.0));
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
