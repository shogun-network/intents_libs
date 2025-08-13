use crate::error::{Error, ModelResult};
use error_stack::report;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Common limit order data to trigger "take profit" or "stop loss" execution
pub struct CommonDcaOrderData {
    /// Timestamp (in seconds) when the user created and submitted the DCA order
    pub start_time: u32,
    /// Amount of tokens IN user is willing to spend per interval/trade
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub amount_in_per_interval: u128,
    /// Total number of intervals over which the DCA order will be executed
    pub total_intervals: u32,
    /// DCA interval duration, in seconds
    pub interval_duration: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Common values of DCA order state
pub struct CommonDcaOrderState {
    /// Total number of already executed intervals
    pub total_executed_intervals: u32,
    /// INDEX of last executed interval
    pub last_executed_interval_index: u32,
}

impl CommonDcaOrderData {
    pub fn get_current_interval_index(&self) -> u32 {
        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("We don't live in the past")
            .as_secs();

        // We can safely cast current timestamp in seconds to u32 until Feb 07 2106.
        self.get_interval_index(current_timestamp as u32)
    }

    /// Calculate interval index at specific timestamp
    pub fn get_interval_index(&self, timestamp: u32) -> u32 {
        if timestamp < self.start_time {
            0
        } else {
            (timestamp - self.start_time) / self.interval_duration + 1
        }
    }

    /// Calculate timestamp of next DCA interval start
    pub fn get_next_interval_start_timestamp(&self) -> u32 {
        let current_interval_index = self.get_current_interval_index();
        self.start_time + current_interval_index * self.interval_duration
    }

    pub fn check_current_dca_interval_can_be_fulfilled(
        &self,
        dca_state: &CommonDcaOrderState,
    ) -> ModelResult<()> {
        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("We don't live in the past")
            .as_secs();

        if current_timestamp < self.start_time as u64 {
            return Err(report!(Error::ValidationError)
                .attach_printable("Order can not be fulfilled: Order has not started yet"));
        }

        let current_interval_index = self.get_interval_index(current_timestamp as u32);

        if current_interval_index <= dca_state.last_executed_interval_index {
            return Err(report!(Error::ValidationError)
                .attach_printable("Current interval was already executed"));
        }

        if dca_state.total_executed_intervals >= self.total_intervals {
            return Err(
                report!(Error::ValidationError).attach_printable("DCA order was fully fulfilled")
            );
        }

        Ok(())
    }

    /// Validates common DCA order data
    pub fn validate(&self, min_interval_duration: u32) -> ModelResult<()> {
        if self.amount_in_per_interval == 0 {
            return Err(report!(Error::ValidationError)
                .attach_printable("Zero amount_in_per_interval".to_string()));
        }

        if self.interval_duration < min_interval_duration {
            return Err(report!(Error::ValidationError).attach_printable(format!(
                "DCA interval duration ({}) is below minimum ({min_interval_duration})",
                self.interval_duration
            )));
        }

        if self.total_intervals < 2 {
            return Err(report!(Error::ValidationError)
                .attach_printable("Invalid total number of DCA intervals".to_string()));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_interval_index() {
        let dca_data = CommonDcaOrderData {
            start_time: 1000,
            amount_in_per_interval: 200,
            total_intervals: 10,
            interval_duration: 30,
        };

        let interval_index = dca_data.get_interval_index(0);
        assert_eq!(interval_index, 0);

        let interval_index = dca_data.get_interval_index(1000);
        assert_eq!(interval_index, 1);

        let interval_index = dca_data.get_interval_index(1030);
        assert_eq!(interval_index, 2);

        let interval_index = dca_data.get_interval_index(1300);
        assert_eq!(interval_index, 11);
    }

    #[test]
    fn test_check_dca_order_can_be_fulfilled() {
        let mut dca_data = CommonDcaOrderData {
            start_time: 4_000_000_000,
            amount_in_per_interval: 200,
            total_intervals: 10,
            interval_duration: 30,
        };

        let dca_state = CommonDcaOrderState {
            total_executed_intervals: 5,
            last_executed_interval_index: 8,
        };

        let res = dca_data.check_current_dca_interval_can_be_fulfilled(&dca_state);
        assert!(res.is_err());

        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("We don't live in the past")
            .as_secs();
        dca_data.start_time = current_timestamp as u32 - 7 * 30;
        let res = dca_data.check_current_dca_interval_can_be_fulfilled(&dca_state);
        assert!(res.is_err());

        dca_data.total_intervals = 5;
        let res = dca_data.check_current_dca_interval_can_be_fulfilled(&dca_state);
        assert!(res.is_err());

        dca_data.total_intervals = 10;
        dca_data.start_time = current_timestamp as u32 - 8 * 30;
        let res = dca_data.check_current_dca_interval_can_be_fulfilled(&dca_state);
        assert!(res.is_ok());
        let current_interval_index = dca_data.get_interval_index(current_timestamp as u32);
        assert_eq!(current_interval_index, 9);
    }

    #[test]
    fn test_dca_order_validate() {
        let mut dca_data = CommonDcaOrderData {
            start_time: 1_000_000_000,
            amount_in_per_interval: 200,
            total_intervals: 10,
            interval_duration: 30,
        };

        let res = dca_data.validate(31);
        assert!(res.is_err());

        dca_data.amount_in_per_interval = 0;
        let res = dca_data.validate(30);
        assert!(res.is_err());

        dca_data.amount_in_per_interval = 0;
        let res = dca_data.validate(30);
        assert!(res.is_err());

        dca_data.amount_in_per_interval = 200;
        dca_data.total_intervals = 1;
        let res = dca_data.validate(30);
        assert!(res.is_err());

        dca_data.total_intervals = 2;
        let res = dca_data.validate(30);
        assert!(res.is_ok());
    }
}
