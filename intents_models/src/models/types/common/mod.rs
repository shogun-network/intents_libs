mod dca_order;
mod fulfillment;
mod limit_order;
mod limit_order_request;
mod user_response;

use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};
use std::{fmt, str::FromStr};

pub use dca_order::*;
pub use fulfillment::*;
pub use limit_order::*;
pub use limit_order_request::*;
pub use user_response::*;

use crate::error::Error;

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
/// Transfer details
pub struct TransferDetails {
    /// Address of token to send
    pub token: String,
    /// Tokens receiver address
    pub receiver: String,
    /// Amount of tokens to send
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub amount: u128,
}

#[serde_as]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StopLossType {
    /// Fixed stop loss based on the current `token_in / token_out` price ratio.
    ///
    /// The stop triggers once the market price falls below `trigger_price`
    /// (expressed as an absolute ratio value).
    Fixed,

    /// Trailing stop loss with a fixed absolute distance from the maximum
    /// observed `token_in / token_out` price ratio.
    ///
    /// Example:
    ///   • Initial price: 100  (token IN to token OUT price at the moment of order creation)
    ///   • `trigger_price`: 90 (distance = -10)
    ///   • Price rises to 120 → trigger moves to 110 (120 - 10)
    ///   • Price falls to 109.9 → stop triggers (109.9 < 110)
    TrailingAbsolute,

    /// Trailing stop loss using a percentage distance from the maximum
    /// observed `token_in / token_out` price ratio.
    ///
    /// Example:
    ///   • Initial price: 100  (token IN to token OUT price at the moment of order creation)
    ///   • `trigger_price`: 90% of the peak
    ///   • Price rises to 120 → trigger moves to 108 (120 * 0.9)
    ///   • Price falls to 107.9 → stop triggers (107.9 < 108)
    TrailingPercent,
}

impl fmt::Display for StopLossType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            StopLossType::Fixed => "Fixed",
            StopLossType::TrailingAbsolute => "TrailingAbsolute",
            StopLossType::TrailingPercent => "TrailingPercent",
        };
        write!(f, "{value}")
    }
}

impl FromStr for StopLossType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Fixed" => Ok(StopLossType::Fixed),
            "TrailingAbsolute" => Ok(StopLossType::TrailingAbsolute),
            "TrailingPercent" => Ok(StopLossType::TrailingPercent),
            _ => Err(Error::ParseError),
        }
    }
}
