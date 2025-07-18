use crate::error::{Error, ModelResult};
use error_stack::report;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Common limit order data to trigger "take profit" or "stop loss" execution
pub struct CommonLimitOrderData {
    /// If Some: Minimum amount OUT required for order to be executed
    /// Can be ignored if `stop_loss_max_out` is None. `amount_out_min` will be used instead
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub take_profit_min_out: Option<u128>,
    /// If Some: Trigger amount OUT considering amount IN and tokens IN/OUT prices
    /// to start execution "Stop loss" order
    /// E.g.: If `amount_in * token_in_usd_price / token_out_usd_price <= stop_loss_max_out` - trigger "Stop loss"
    /// Must be higher than `amount_out_min`
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_loss_max_out: Option<u128>,
    /// `stop_loss_max_out` threshold was reached and now immediate marker order must be executed
    pub stop_loss_triggered: bool,
}

impl CommonLimitOrderData {
    pub fn get_amount_out_min(&self, amount_out_min: u128) -> u128 {
        match self.stop_loss_max_out {
            // If no "stop loss" is requested, we just use `amount_out_min`
            None => amount_out_min,
            Some(_) => {
                if self.stop_loss_triggered {
                    // If "stop loss" was triggered, we use `amount_out_min`
                    amount_out_min
                } else {
                    match self.take_profit_min_out {
                        None => amount_out_min,
                        Some(take_profit_min_out) => {
                            // If "stop loss" was not triggered and "take profit" is set, we aim for "take profit"
                            std::cmp::max(amount_out_min, take_profit_min_out)
                        }
                    }
                }
            }
        }
    }

    pub fn check_order_can_be_fulfilled(&self) -> ModelResult<()> {
        // If no "stop loss" is requested order can be fulfilled
        if self.stop_loss_max_out.is_none()
            // If "stop loss" was triggered, order must be fulfilled immediately
            || self.stop_loss_triggered
            // If "stop loss" was requested while "take profit" was not
            // This mean the only way to fulfill the order is wait for "stop loss" conditions
            // If "take profit" was requested as well - order can be fulfilled 
            // by matching "take profit" conditions
            || self.take_profit_min_out.is_some()
        {
            Ok(())
        } else {
            Err(report!(Error::ValidationError).attach_printable(
                "Order can not be fulfilled:\
                     Only 'stop loss' was requested, but it's threshold was not triggered",
            ))
        }
    }

    /// Validates common limit order data
    pub fn validate(&self, amount_out_min: u128) -> ModelResult<()> {
        if let Some(stop_loss_max_out) = self.stop_loss_max_out
            && amount_out_min >= stop_loss_max_out
        {
            return Err(report!(Error::ValidationError).attach_printable(format!(
                "amount_out_min ({amount_out_min}) \
                must be lower than stop_loss_max_out ({stop_loss_max_out})"
            )));
        }

        if let (Some(stop_loss_max_out), Some(take_profit_min_out)) =
            (self.stop_loss_max_out, self.take_profit_min_out)
            && stop_loss_max_out >= take_profit_min_out
        {
            return Err(report!(Error::ValidationError).attach_printable(format!(
                "stop_loss_max_out ({stop_loss_max_out}) \
                must be lower than take_profit_min_out ({take_profit_min_out})"
            )));
        }

        if let (None, Some(take_profit_min_out)) =
            (self.stop_loss_max_out, self.take_profit_min_out)
            && amount_out_min != take_profit_min_out
        {
            return Err(report!(Error::ValidationError).attach_printable(
                "If 'stop loss' is not required, take_profit_min_out must be omitted or equal to amount_out_min"
            ));
        }

        Ok(())
    }
}
