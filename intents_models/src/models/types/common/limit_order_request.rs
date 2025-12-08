use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

use crate::models::types::common::StopLoss;

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Common limit order data to trigger "take profit" or "stop loss" execution
pub struct CommonLimitOrderUserRequestData {
    /// If Some: Minimum amount OUT required for order to be executed
    /// Can be ignored if `stop_loss_max_out` is None. `amount_out_min` will be used instead
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub take_profit_min_out: Option<u128>,
    /// If Some: Trigger amount OUT considering amount IN and tokens IN/OUT prices
    /// to start execution "Stop loss" order
    /// E.g.: If `amount_in * token_in_usd_price / token_out_usd_price <= stop_loss_max_out` - trigger "Stop loss"
    /// Must be higher than `amount_out_min`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_loss_max_out: Option<StopLoss>,
}
