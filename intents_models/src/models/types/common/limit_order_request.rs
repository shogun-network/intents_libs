use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};

use crate::models::types::common::StopLossType;

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
    /// Stop loss type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_loss_type: Option<StopLossType>,
    /// Initial requested trigger price of token IN/token OUT to trigger stop loss
    #[serde_as(as = "Option<PickFirst<(DisplayFromStr, _)>>")]
    pub stop_loss_trigger_price: Option<f64>,
}
