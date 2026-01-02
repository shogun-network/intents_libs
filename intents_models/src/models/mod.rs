use std::ops::Deref;

use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};

pub mod types;
pub mod ws_messages;

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DisplayU128(#[serde_as(as = "PickFirst<(DisplayFromStr, _)>")] pub u128);

impl DisplayU128 {
    /// Create from raw u128.
    pub fn new(value: u128) -> Self {
        DisplayU128(value)
    }

    /// Get the inner u128 by value.
    pub fn into_inner(self) -> u128 {
        self.0
    }

    /// Get the inner u128 by reference.
    pub fn as_u128(&self) -> &u128 {
        &self.0
    }
}

impl From<u128> for DisplayU128 {
    fn from(value: u128) -> Self {
        DisplayU128(value)
    }
}

impl From<DisplayU128> for u128 {
    fn from(value: DisplayU128) -> Self {
        value.0
    }
}

impl Deref for DisplayU128 {
    type Target = u128;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
