//! Purely logistical utilities, vendored from the personal `rust-utils`
//! crate (<https://github.com/edusu/rust-utils>).
//!
//! Generic plumbing the services reuse — nothing here knows about
//! intents or orders. Wired onto the crate's domain [`crate::error::Error`]
//! so callers get a single error taxonomy.

pub mod retry;

mod backoff;

pub use retry::Retry;
