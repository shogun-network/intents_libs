use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};

pub mod types;
pub mod ws_messages;

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DisplayU128(#[serde_as(as = "PickFirst<(DisplayFromStr, _)>")] pub u128);
