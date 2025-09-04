use crate::constants::chains::ChainId;
use crate::error::{Error, ModelResult};
use crate::models::types::cross_chain::{
    CrossChainDcaOrderSolverStartPermission, CrossChainExecutionTerms,
    CrossChainLimitOrderSolverStartPermission, CrossChainSolverStartPermissionEnum,
    StartEvmCrossChainDcaOrderData, StartEvmCrossChainLimitOrderData,
};
use crate::models::types::single_chain::{
    SingleChainDcaOrderSolverStartPermission, SingleChainExecutionTerms,
    SingleChainLimitOrderSolverStartPermission, SingleChainSolverStartPermissionEnum,
    StartEvmSingleChainDcaOrderData, StartEvmSingleChainLimitOrderData,
};
use error_stack::report;
use serde::{Deserialize, Serialize};

/*********************************************************************/
/**************************** START ORDER ****************************/
/*********************************************************************/

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
/// Terms of execution of specific intent
pub enum ExecutionTerms {
    CrossChain(CrossChainExecutionTerms),
    SingleChain(SingleChainExecutionTerms),
}

impl ExecutionTerms {
    pub fn try_get_single_chain(&self) -> ModelResult<&SingleChainExecutionTerms> {
        match self {
            ExecutionTerms::CrossChain(_) => Err(report!(Error::LogicError(
                "Non-single-chain terms passed".to_string()
            ))),
            ExecutionTerms::SingleChain(terms) => Ok(terms),
        }
    }
    pub fn try_get_cross_chain(&self) -> ModelResult<&CrossChainExecutionTerms> {
        match self {
            ExecutionTerms::CrossChain(terms) => Ok(terms),
            ExecutionTerms::SingleChain(_) => Err(report!(Error::LogicError(
                "Non-cross-chain terms passed".to_string()
            ))),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
/// Data, used by Solver to start order execution
pub enum SolverStartPermission {
    SingleChainLimit(SingleChainLimitOrderSolverStartPermission),
    SingleChainDca(SingleChainDcaOrderSolverStartPermission),
    CrossChainLimit(CrossChainLimitOrderSolverStartPermission),
    CrossChainDca(CrossChainDcaOrderSolverStartPermission),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
/// Data, used by Solver to start order execution (sorted by chain number)
pub enum SolverStartPermissionChainNumber {
    SingleChain(SingleChainSolverStartPermissionEnum),
    CrossChain(CrossChainSolverStartPermissionEnum),
}

impl SolverStartPermission {
    pub fn get_solver_amount_out(&self) -> u128 {
        match self {
            SolverStartPermission::SingleChainLimit(permission) => {
                permission.common_data.expected_amount_out
            }
            SolverStartPermission::SingleChainDca(permission) => {
                permission.common_data.expected_amount_out
            }
            SolverStartPermission::CrossChainLimit(permission) => {
                permission.common_data.expected_amount_out
            }
            SolverStartPermission::CrossChainDca(permission) => {
                permission.common_data.expected_amount_out
            }
        }
    }
    pub fn get_src_chain_id(&self) -> ChainId {
        match self {
            SolverStartPermission::SingleChainLimit(permission) => {
                permission.generic_data.common_data.chain_id
            }
            SolverStartPermission::SingleChainDca(permission) => {
                permission.generic_data.common_data.chain_id
            }
            SolverStartPermission::CrossChainLimit(permission) => {
                permission.generic_data.common_data.src_chain_id
            }
            SolverStartPermission::CrossChainDca(permission) => {
                permission.generic_data.common_data.src_chain_id
            }
        }
    }
    pub fn get_dest_chain_id(&self) -> ChainId {
        match self {
            SolverStartPermission::SingleChainLimit(permission) => {
                permission.generic_data.common_data.chain_id
            }
            SolverStartPermission::SingleChainDca(permission) => {
                permission.generic_data.common_data.chain_id
            }
            SolverStartPermission::CrossChainLimit(permission) => {
                permission.generic_data.common_data.dest_chain_id
            }
            SolverStartPermission::CrossChainDca(permission) => {
                permission.generic_data.common_data.dest_chain_id
            }
        }
    }
    pub fn try_into_cross_chain(self) -> ModelResult<CrossChainSolverStartPermissionEnum> {
        match self {
            SolverStartPermission::CrossChainLimit(permission) => {
                Ok(CrossChainSolverStartPermissionEnum::Limit(permission))
            }
            SolverStartPermission::CrossChainDca(permission) => {
                Ok(CrossChainSolverStartPermissionEnum::Dca(permission))
            }
            SolverStartPermission::SingleChainLimit(_)
            | SolverStartPermission::SingleChainDca(_) => Err(report!(Error::LogicError(
                "Non-cross-chain permission passed".to_string()
            ))),
        }
    }
    pub fn into_chains_num(self) -> SolverStartPermissionChainNumber {
        match self {
            SolverStartPermission::SingleChainLimit(permission) => {
                SolverStartPermissionChainNumber::SingleChain(
                    SingleChainSolverStartPermissionEnum::Limit(permission),
                )
            }
            SolverStartPermission::SingleChainDca(permission) => {
                SolverStartPermissionChainNumber::SingleChain(
                    SingleChainSolverStartPermissionEnum::Dca(permission),
                )
            }
            SolverStartPermission::CrossChainLimit(permission) => {
                SolverStartPermissionChainNumber::CrossChain(
                    CrossChainSolverStartPermissionEnum::Limit(permission),
                )
            }
            SolverStartPermission::CrossChainDca(permission) => {
                SolverStartPermissionChainNumber::CrossChain(
                    CrossChainSolverStartPermissionEnum::Dca(permission),
                )
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// EVM-specific data for start order execution
pub struct StartOrderEVMData {
    /// Guard contract that should be called by the solver
    pub guard_contract: String,
    /// User Permit2 signature
    pub user_signature: String,
    /// Auctioneer start permission signature
    pub auctioneer_start_permission_signature: String,
    /// Type-specific data for order execution
    pub order_type_data: StartEvmOrderTypeData,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
/// Type-specific order data required for execution start
pub enum StartEvmOrderTypeData {
    SingleChainLimit(StartEvmSingleChainLimitOrderData),
    SingleChainDca(StartEvmSingleChainDcaOrderData),
    CrossChainLimit(StartEvmCrossChainLimitOrderData),
    CrossChainDca(StartEvmCrossChainDcaOrderData),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Solana-specific data for start order execution
pub struct StartOrderSolanaData {
    /// Program ID, that should be interacted with
    pub program_id: String,
    /// Guard account address
    pub guard: String,
    /// Order account address
    pub order: String,
    /// Serialized and hex-encoded start execution permission
    pub serialized_permission: String,
    /// Hex-encoded signature, generated by Auctioneer after signing permission
    pub signature: String,
    /// Hex-encoded data for Ed25519SigVerify111111111111111111111111111 program instruction
    pub verify_ix_data: String,
}
