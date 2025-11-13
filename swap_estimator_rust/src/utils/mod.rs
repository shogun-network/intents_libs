use std::time::{SystemTime, UNIX_EPOCH};

pub mod exact_in_reverse_quoter;
pub mod limit_amount;
pub mod number_conversion;
mod uint;

pub fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("We don't live in the past")
        .as_secs()
}
