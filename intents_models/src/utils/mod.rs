//! Purely logistical utilities, vendored from the personal `rust-utils`
//! crate (<https://github.com/edusu/rust-utils>).
//!
//! Generic plumbing the services reuse — nothing here knows about
//! intents or orders. Wired onto the crate's domain [`crate::error::Error`]
//! so callers get a single error taxonomy.

pub mod retry;
pub mod shutdown;
pub mod supervisor;

mod backoff;

pub use retry::Retry;
pub use shutdown::ShutdownController;
pub use supervisor::Supervisor;
