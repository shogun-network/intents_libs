use crate::constants::chains::ChainId;
use crate::error::{Error, ModelResult};
use crate::models::types::solver_types::{StartOrderEVMData, StartOrderSolanaData};
use crate::models::types::user_types::TransferDetails;
use error_stack::report;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
/*********************************************************************/
/**************************** START ORDER ****************************/
/*********************************************************************/
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SingleChainSolverStartPermission {
    /// Solver wallet address that will start order execution
    pub solver_address: String,
    /// Promised amount OUT by the solver
    #[serde_as(as = "DisplayFromStr")]
    pub expected_amount_out: u128,
    /// Deadline in seconds, by which Solver must execute the intent
    pub solver_deadline: u64,
    /// Address of protocol fee token, receiver and protocol fee amount
    pub protocol_fee_transfer: TransferDetails,
    /// Contains chain-specific data
    pub chain_specific_data: SingleChainSolverStartOrderData,
}

/// Enum for the chain-independent data
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SingleChainSolverStartOrderData {
    /// EVM-based chain data (e.g., Ethereum, Binance Smart Chain)
    EVM(StartOrderEVMData),
    /// Solana-based chain data
    Solana(StartOrderSolanaData),
}

impl SingleChainSolverStartOrderData {
    pub fn try_get_evm_data(&self) -> ModelResult<&StartOrderEVMData> {
        match self {
            SingleChainSolverStartOrderData::EVM(data) => Ok(data),
            _ => Err(report!(Error::LogicError(
                "Non-EVM data passed".to_string()
            ))),
        }
    }
    pub fn try_get_solana_data(&self) -> ModelResult<&StartOrderSolanaData> {
        match self {
            SingleChainSolverStartOrderData::Solana(data) => Ok(data),
            _ => Err(report!(Error::LogicError(
                "Non-Solana data passed".to_string()
            ))),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SingleChainExecutionTerms {
    /// Address of protocol fee token, receiver and protocol fee amount
    pub protocol_fee_transfer: TransferDetails,
    /// Deadline in seconds, by which Solver must execute the intent
    pub solver_execution_duration: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Set of common data to check single chain limit order execution
pub struct SingleChainOrderExecutionDetails {
    pub chain_id: ChainId,
    pub intent_id: String,
    pub tx_hash: String,
}
