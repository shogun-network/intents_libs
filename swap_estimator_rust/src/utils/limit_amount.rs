use crate::routers::estimate::TradeType;

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
