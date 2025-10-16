use crate::constants::chains::ChainId;
use crate::error::{Error, ModelResult};
use crate::models::types::common::TransferDetails;
use crate::models::types::order::OrderTypeFulfillmentData;
use crate::models::types::solver_types::{StartOrderEVMData, StartOrderSolanaData};
use error_stack::report;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
/*********************************************************************/
/**************************** START ORDER ****************************/
/*********************************************************************/
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Permission, granted to Solver to start single chain order execution
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

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Chain-specific data of permission, granted to Solver to start single chain order execution
pub enum SingleChainSolverStartOrderData {
    /// EVM-based chain data (e.g., Ethereum, Binance Smart Chain)
    EVM(StartOrderEVMData),
    /// Solana-based chain data
    Solana(StartOrderSolanaData),
    /// Sui-based chain data
    Sui(SingleChainStartOrderSuiData),
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
    pub fn try_get_sui_data(&self) -> ModelResult<&SingleChainStartOrderSuiData> {
        match self {
            SingleChainSolverStartOrderData::Sui(data) => Ok(data),
            _ => Err(report!(Error::LogicError(
                "Non-Sui data passed".to_string()
            ))),
        }
    }

    pub fn get_intent_id(&self) -> String {
        match &self {
            SingleChainSolverStartOrderData::EVM(evm_data) => {
                evm_data.order_type_data.get_intent_id()
            }
            SingleChainSolverStartOrderData::Solana(solana_data) => solana_data.order.to_owned(),
            SingleChainSolverStartOrderData::Sui(sui_data) => sui_data.order_id.to_owned(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Single chain order execution terms, provided to the Solver during auction
pub struct SingleChainExecutionTerms {
    /// Address of protocol fee token, receiver and protocol fee amount
    pub protocol_fee_transfer: TransferDetails,
    /// Deadline in seconds, by which Solver must execute the intent
    pub solver_execution_duration: u64,
    /// Fulfillment data for a specific order type
    pub order_type_specific_data: OrderTypeFulfillmentData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Set of common data to check single chain limit order execution
pub struct SingleChainOrderExecutionDetails {
    pub chain_id: ChainId,
    pub intent_id: String,
    pub tx_hash: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Sui-specific chain data, used by Solver to start single chain order execution
pub struct SingleChainStartOrderSuiData {
    /// Package ID, that should be interacted with
    pub package_id: String,
    /// Guard object address
    pub guard_id: String,
    /// Order object address
    pub order_id: String,
    /// Signer permission to start the order
    pub signed_permission_data: Vec<u8>,
    /// Auctioneer permission signature
    pub auctioneer_signature: Vec<u8>,
    /// Protocol fee amount that should be passed to function call
    pub protocol_fee_amount: u64,
    /// Type arguments for the function call
    /// `FeeToken, StableCoin, CoinIn, CoinOut, ExtraTransferCoinOut`
    pub types: [String; 5],
}
