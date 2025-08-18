use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Call mode for EVM external calls
pub enum EvmCallMode {
    /// Approve tokens to call target and call it
    /// If tokens are not taken - send to fallback address
    /// If token is native (address(0)) use msg.value to send tokens
    ApproveAndCall,
    /// Transfer tokens to call target and call it
    /// If token is native (address(0)) use `call` to send tokens
    TransferAndCall,
}
