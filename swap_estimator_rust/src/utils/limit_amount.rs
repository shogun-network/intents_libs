use crate::error::Error;
use crate::error::EstimatorResult;
use crate::routers::estimate::TradeType;
use error_stack::report;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;

pub fn get_limit_amount(trade_type: TradeType, amount_quote: u128, slippage: f64) -> u128 {
    match trade_type {
        // calculating amountOutMin
        TradeType::ExactIn => {
            amount_quote * ((100f64 - slippage) * 1_000_000_000f64) as u128 / 100_000_000_000u128
        }
        // calculating amountInMax
        TradeType::ExactOut => {
            amount_quote * ((100f64 + slippage) * 1_000_000_000f64) as u128 / 100_000_000_000u128
        }
    }
}

pub fn get_limit_amount_u64(trade_type: TradeType, amount_quote: u64, slippage: f64) -> u64 {
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
        let limit_amount = get_limit_amount(TradeType::ExactIn, 1000, 2.0);
        assert_eq!(limit_amount, 980);
        let limit_amount = get_limit_amount(TradeType::ExactOut, 1000, 2.0);
        assert_eq!(limit_amount, 1020);
    }

    #[test]
    fn test_get_limit_amount_u64() {
        let limit_amount = get_limit_amount_u64(TradeType::ExactIn, 1000, 2.0);
        assert_eq!(limit_amount, 980);
        let limit_amount = get_limit_amount_u64(TradeType::ExactOut, 1000, 2.0);
        assert_eq!(limit_amount, 1020);
    }
}
