use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractsAddresses {
    pub evm: HashMap<u32, EvmContractsAddresses>,
    pub solana: SolanaContractsAddresses,
    pub sui: SuiContractsAddresses,
}

// ================================= EVM =================================

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvmContractsAddresses {
    pub single_chain: SingleChainEvmContracts,
    pub cross_chain: CrossChainEvmContracts,
    pub permit2: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SingleChainEvmContracts {
    pub guard_limit: String,
    pub guard_dca: String,
    pub protocol_fee_token: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossChainEvmContracts {
    pub guard_limit: String,
    pub guard_dca: String,
    pub collateral_token: String,
    pub stablecoin: String,
    pub destination_chain_guard: String,
}

// ================================ SOLANA ================================
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolanaContractsAddresses {
    pub single_chain: SingleChainSolanaContracts,
    pub cross_chain: CrossChainSolanaContracts,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SingleChainSolanaContracts {
    pub guard_program_id: String,
    pub guard_account: String,
    pub protocol_fee_token: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossChainSolanaContracts {
    pub guard_program_id: String,
    pub guard_account: String,
    pub collateral_token_mint: String,
    pub collateral_token_program: String,
    pub stablecoin_token_mint: String,
    pub stablecoin_token_program: String,
}

// ================================= SUI =================================
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuiContractsAddresses {
    pub package_id: String,
    pub guard: String,
    pub guard_collateral_type: String,
    pub guard_stablecoin_type: String,
}
