use crate::error::{Error, ModelResult};
use crate::models::types::cross_chain::{
    CrossChainGenericDataEnum, EvmCrossChainFulfillmentData,
    EvmSuccessConfirmationCrossChainDcaOrderData, EvmSuccessConfirmationCrossChainLimitOrderData,
};
use crate::models::types::order::OrderTypeFulfillmentData;
use crate::models::types::solver_types::{StartOrderEVMData, StartOrderSolanaData};
use error_stack::report;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Data, used by Solver to start cross chain order execution
pub struct CrossChainSolverStartPermission {
    /// Solver wallet address on source chain, that will start order execution
    pub src_chain_solver_address: String,
    /// Solver wallet address on destination chain, that will trigger transaction of order fulfillment
    pub dest_chain_solver_address: String,
    /// Promised amount OUT by the solver
    #[serde_as(as = "DisplayFromStr")]
    pub expected_amount_out: u128,
    /// Is Solver allowed to swap token IN into stablecoin
    pub allow_swap: bool,
    /// Minimum amount of stablecoins Solver should provide after swap
    #[serde_as(as = "DisplayFromStr")]
    pub min_stablecoins_amount: u128,
    /// Address of stablecoins, tokens IN must be swapped into (if allowed)
    pub stablecoins_address: String,
    /// Deadline in seconds, by which Solver must execute the intent
    pub solver_deadline: u64,
    /// Contains chain-specific data to start order execution on source chain
    pub src_chain_specific_data: CrossChainSolverStartOrderData,
    /// Destination-chain-specific data, used by Solver to fulfill order on destination chain
    pub dest_chain_fulfillment_details: CrossChainSolverFulfillmentData,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Chain-specific data, used by Solver to start cross chain order execution
pub enum CrossChainSolverStartOrderData {
    /// EVM-based chain data (e.g., Ethereum, Binance Smart Chain)
    EVM(StartOrderEVMData),
    /// Sui-based chain data
    Sui(CrossChainStartOrderSuiData),
    /// Solana-based chain data
    Solana(StartOrderSolanaData),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Destination-chain-specific data, used by Solver to fulfill order on destination chain
pub enum CrossChainSolverFulfillmentData {
    /// EVM-based chain data (e.g., Ethereum, Binance Smart Chain)
    EVM(EvmCrossChainFulfillmentData),
    /// Sui-based chain data
    Sui,
    /// Solana-based chain data
    Solana,
}

impl CrossChainSolverStartOrderData {
    pub fn try_get_evm_data(&self) -> ModelResult<&StartOrderEVMData> {
        match self {
            CrossChainSolverStartOrderData::EVM(data) => Ok(data),
            _ => Err(report!(Error::LogicError(
                "Non-EVM data passed".to_string()
            ))),
        }
    }
    pub fn try_get_solana_data(&self) -> ModelResult<&StartOrderSolanaData> {
        match self {
            CrossChainSolverStartOrderData::Solana(data) => Ok(data),
            _ => Err(report!(Error::LogicError(
                "Non-Solana data passed".to_string()
            ))),
        }
    }
    pub fn try_get_sui_data(&self) -> ModelResult<&CrossChainStartOrderSuiData> {
        match self {
            CrossChainSolverStartOrderData::Sui(data) => Ok(data),
            _ => Err(report!(Error::LogicError(
                "Non-Sui data passed".to_string()
            ))),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Sui-specific chain data, used by Solver to start cross chain order execution
pub struct CrossChainStartOrderSuiData {
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
    /// Collateral amount that should be passed to function call
    pub collateral_amount: u64,
    /// Protocol fee amount that should be passed to function call
    pub protocol_fee_amount: u64,
    /// Type arguments for the function call
    pub types: [String; 3],
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
/// Terms of execution of cross-chain intent, provided to Solver, used for bidding estimations and execution
pub struct CrossChainExecutionTerms {
    /// Amount of collateral for as solver to lock
    #[serde_as(as = "DisplayFromStr")]
    pub collateral_amount: u128,
    /// Amount of protocol fees to pay for order execution
    #[serde_as(as = "DisplayFromStr")]
    pub protocol_fee: u128,
    /// Address of token that is taken as protocol fee/collateral
    pub collateral_token_address: String,
    /// Is Solver allowed to swap token IN into stablecoin
    pub allow_swap: bool,
    /// Minimum amount of stablecoins Solver should provide after swap
    #[serde_as(as = "DisplayFromStr")]
    pub min_stablecoins_amount: u128,
    /// Address of stablecoins there are locked in the order
    pub stablecoin_address: String,
    /// Deadline in seconds, by which Solver must execute the intent
    pub solver_execution_duration: u64,

    /// Were tokens IN already swapped to stablecoins?
    pub tokens_in_were_swapped_to_stablecoins: bool,
    /// Amount of stablecoins locked after tokens IN swap. 0 If tokens IN were not swapped
    #[serde_as(as = "DisplayFromStr")]
    pub stablecoins_locked: u128,
    /// Fulfillment data for a specific order type
    pub order_type_specific_data: OrderTypeFulfillmentData,
}

/*********************************************************************/
/************************ CONFIRM FULFILLMENT ************************/
/*********************************************************************/

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
/// Auctioneer data collected after checking cross-chain order execution
pub struct DestChainFulfillmentDetails {
    /// Actually received main amount OUT
    #[serde_as(as = "DisplayFromStr")]
    pub main_amount_out: u128,
    /// Since we may require multiple transfers, sometimes we can't be sure which one
    /// was successful and which one wasn't. That's why we provide data about what
    /// transactions don't contain which transfers
    pub amounts_inconsistencies: Option<Vec<AmountInconsistency>>,
    /// `true` - transaction was signed by the Solver
    pub has_valid_tx_signer: bool,
    /// Timestamp of main transaction execution
    pub tx_timestamp: u64,
    /// Array of extra transfers fulfillment details
    pub extra_transfer_fulfillment_details: Option<Vec<TransferFulfillmentDetails>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Auctioneer data collected after checking cross-chain extra transfer execution
pub struct TransferFulfillmentDetails {
    /// `true` - transaction was signed by the Solver
    pub has_valid_tx_signer: bool,
    /// Timestamp of transaction execution
    pub tx_timestamp: u64,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
/// Auctioneer data collected after checking cross-chain extra transfer execution.
/// Contains inconsistencies of expected/received tokens amounts
pub struct AmountInconsistency {
    /// Transaction hash provided
    pub tx_hash: String,
    /// Token, expected to receive
    pub token: String,
    /// Token receiver
    pub receiver: String,
    /// Requested amount to receive
    #[serde_as(as = "DisplayFromStr")]
    pub requested_to_receive: u128,
    /// Actually received amount
    #[serde_as(as = "DisplayFromStr")]
    pub actually_received: u128,
}

/**********************************************************************/
/**************************** CLAIM TOKENS ****************************/
/**********************************************************************/

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Success confirmation, provided to Solver after successful order execution.
/// Allows Solver to claim tokens in source chain
pub struct CrossChainSolverSuccessConfirmation {
    /// Unique order identifier
    pub order_id: String,
    /// Solver address that executed the intent
    pub src_chain_solver_address: String,
    /// Contains the common data for the intent
    pub generic_data: CrossChainGenericDataEnum,
    /// Fulfillment data for a specific order type
    pub order_type_specific_data: OrderTypeFulfillmentData,
    /// Contains chain-specific data
    pub chain_specific_data: SolverSuccessConfirmationData,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Enum for the chain-specific data of success confirmation, provided to Solver after successful
/// cross-chain order execution. Allows Solver to claim tokens in source chain
pub enum SolverSuccessConfirmationData {
    /// EVM-based chain data (e.g., Ethereum, Binance Smart Chain)
    EVM(SuccessConfirmationEVMData),
    /// Sui-based chain data
    Sui(SuccessConfirmationSuiData),
    /// Solana-based chain data
    Solana(SuccessConfirmationSolanaData),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// EVM-specific data of success confirmation, provided to Solver after successful cross-chain order execution
pub struct SuccessConfirmationEVMData {
    /// Guard contract that should be called by the solver
    pub guard_contract: String,
    /// Auctioneer confirmation signature
    pub auctioneer_signature: String,
    /// Type-specific data for order execution
    pub order_type_data: EvmSuccessConfirmationOrderTypeData,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum EvmSuccessConfirmationOrderTypeData {
    CrossChainLimit(EvmSuccessConfirmationCrossChainLimitOrderData),
    CrossChainDca(EvmSuccessConfirmationCrossChainDcaOrderData),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Sui-specific data of success confirmation, provided to Solver after successful cross-chain order execution
pub struct SuccessConfirmationSuiData {
    /// Package ID, that should be interacted with
    pub package_id: String,
    /// Guard object address
    pub guard_id: String,
    /// Order object address
    pub order_id: String,
    /// Success confirmation data that should be passed to contract
    pub signed_success_confirmation_data: Vec<u8>,
    /// Auctioneer confirmation signature
    pub auctioneer_signature: Vec<u8>,
    /// Type arguments for the function call
    pub types: [String; 3],
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Solana-specific data of success confirmation, provided to Solver after successful cross-chain order execution
pub struct SuccessConfirmationSolanaData {
    /// Program ID, that should be interacted with
    pub program_id: String,
    /// Guard account address
    pub guard: String,
    /// Order account address
    pub order: String,
    /// Serialized and hex-encoded success confirmation
    pub serialized_success_confirmation: String,
    /// Hex-encoded signature, generated by Auctioneer after signing success confirmation
    pub signature: String,
    /// Hex-encoded data for Ed25519SigVerify111111111111111111111111111 program instruction
    pub verify_ix_data: String,
}
