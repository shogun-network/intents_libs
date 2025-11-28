use crate::error::{Error, EstimatorResult};
use crate::routers::estimate::TradeType;
use error_stack::report;

/// Replaces 32-bytes amount limit in calldata
///
/// Throws if `amount_quote` is not enough to satisfy `requested_amount_limit`
/// Throws if could not replace anything
pub fn replace_amount_limit_in_tx(
    call_data: String,
    trade_type: TradeType,
    amount_quote: u128,
    amount_limit: u128,
    requested_amount_limit: u128,
) -> EstimatorResult<String> {
    match trade_type {
        TradeType::ExactIn => {
            if amount_quote < requested_amount_limit {
                return Err(report!(Error::AggregatorError(format!(
                    "Amount quote {amount_quote} is lower than requested \
                    amount limit {requested_amount_limit} for exact IN trade"
                ))));
            }
        }
        TradeType::ExactOut => {
            if amount_quote > requested_amount_limit {
                return Err(report!(Error::AggregatorError(format!(
                    "Amount quote {amount_quote} is greater than requested \
                    amount limit {requested_amount_limit} for exact OUT trade"
                ))));
            }
        }
    }
    let amount_limit_hex = format!("{:064x}", amount_limit);
    let requested_amount_limit_hex = format!("{:064x}", requested_amount_limit);
    let new_tx_data = call_data.replace(&amount_limit_hex, &requested_amount_limit_hex);
    if new_tx_data.eq(&call_data) && amount_limit != requested_amount_limit {
        return Err(report!(Error::AggregatorError(format!(
            "Could not replace amount limit {amount_limit} with \
                requested amount limit {requested_amount_limit} in Relay calldata"
        ))));
    }

    Ok(new_tx_data)
}
