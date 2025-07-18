use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
/// Collected common on chain cross-chain order data about current on chain order state
pub struct CrossChainOnChainOrderData {
    /// `true` - At least one execution has started
    pub execution_has_started: bool,
    /// `true` - token IN were already swapped to stablecoins
    pub tokens_in_were_swapped_to_stablecoins: bool,
    /// amount of collateral tokens locked in the order
    #[serde_as(as = "DisplayFromStr")]
    pub locked_collateral: u128,
    /// Collateral token address
    pub collateral_token_address: String,
    /// Amount of stablecoins locked in the order
    #[serde_as(as = "DisplayFromStr")]
    pub locked_stablecoins: u128,
    /// Stablecoin address
    pub stablecoin_address: String,
    /// If possible - determine if order was deactivated by cancelling or in other way
    pub deactivated: Option<bool>,
}
