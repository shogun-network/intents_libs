mod dca_order;
mod fulfillment;
mod limit_order;
mod limit_order_request;
mod user_response;

use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};

pub use dca_order::*;
pub use fulfillment::*;
pub use limit_order::*;
pub use limit_order_request::*;
pub use user_response::*;

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
