use error_stack::{Report, report};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::fmt;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::error::Error;

pub const NATIVE_TOKEN_EVM_ADDRESS: &str = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
pub const EVM_NULL_ADDRESS: &str = "0x0000000000000000000000000000000000000000";

pub const NATIVE_TOKEN_EVM_ADDRESSES: [&str; 2] = [NATIVE_TOKEN_EVM_ADDRESS, EVM_NULL_ADDRESS];

pub fn is_native_token_evm_address(address: &str) -> bool {
    NATIVE_TOKEN_EVM_ADDRESSES.contains(&address.to_lowercase().as_str())
}

pub const NATIVE_TOKEN_SOLANA_ADDRESS: &str = "So11111111111111111111111111111111111111111";

pub const NATIVE_TOKEN_SOLANA_ADDRESSES: [&str; 2] = [
    NATIVE_TOKEN_SOLANA_ADDRESS,
    "11111111111111111111111111111111",
];

pub fn is_native_token_solana_address(address: &str) -> bool {
    NATIVE_TOKEN_SOLANA_ADDRESSES.contains(&address)
}

pub const WRAPPED_NATIVE_TOKEN_SOLANA_ADDRESS: &str = "So11111111111111111111111111111111111111112";

pub const NATIVE_TOKEN_SUI_ADDRESS: &str = "0x2::sui::SUI";

pub const WRAPPED_NATIVE_TOKEN_HYPE_ADDRESS: &str = "0x5555555555555555555555555555555555555555";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize_repr, Deserialize_repr, EnumIter, Hash)]
#[repr(u32)]
pub enum ChainId {
    Ethereum = 1,
    Bsc = 56,
    ArbitrumOne = 42161,
    Base = 8453,
    Monad = 143,

    Solana = 7565164,

    Sui = 101,
    Optimism = 10,
    HyperEVM = 999,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, Hash)]
pub enum ChainType {
    EVM,
    Solana,
    Sui,
}

impl ChainId {
    pub fn supported_chains() -> Vec<ChainId> {
        let supported_chains: Vec<_> = ChainId::iter().collect();

        supported_chains
    }

    pub fn to_chain_type(&self) -> ChainType {
        match self {
            Self::Solana => ChainType::Solana,
            Self::Sui => ChainType::Sui,
            _ => ChainType::EVM,
        }
    }
}

impl TryFrom<u32> for ChainId {
    type Error = Report<Error>;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        serde_json::from_str(&value.to_string()).map_err(|e| {
            Report::new(Error::ParseError)
                .attach_printable(format!("Failed to parse chain ID: {e}"))
        })
    }
}

impl fmt::Display for ChainId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Ethereum => write!(f, "Ethereum"),
            Self::Bsc => write!(f, "BSC"),
            Self::ArbitrumOne => write!(f, "Arbitrum One"),
            Self::Base => write!(f, "Base"),
            Self::Monad => write!(f, "Monad"),
            Self::Solana => write!(f, "Solana"),
            Self::Sui => write!(f, "Sui"),
            Self::Optimism => write!(f, "Optimism"),
            Self::HyperEVM => write!(f, "HyperEVM"),
        }
    }
}

impl fmt::Display for ChainType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::EVM => write!(f, "EVM"),
            Self::Solana => write!(f, "Solana"),
            Self::Sui => write!(f, "Sui"),
        }
    }
}

impl TryFrom<&str> for ChainId {
    type Error = Report<Error>;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "Ethereum" | "1" => Ok(Self::Ethereum),
            "Bsc" | "BSC" | "56" => Ok(Self::Bsc),
            "ArbitrumOne" | "Arbitrum One" | "42161" => Ok(Self::ArbitrumOne),
            "Base" | "8453" => Ok(Self::Base),
            "Monad" | "143" => Ok(Self::Monad),
            "Solana" => Ok(Self::Solana),
            "Sui" | "101" => Ok(Self::Sui),
            "Optimism" | "10" => Ok(Self::Optimism),
            "HyperEVM" | "999" => Ok(Self::HyperEVM),
            _ => Err(report!(Error::ChainError(format!(
                "Invalid chain name: {value}"
            )))),
        }
    }
}

impl ChainId {
    pub fn is_native_token(self, address: &str) -> bool {
        match self {
            ChainId::Ethereum
            | ChainId::Bsc
            | ChainId::ArbitrumOne
            | ChainId::Base
            | ChainId::Optimism
            | ChainId::Monad
            | ChainId::HyperEVM => is_native_token_evm_address(address),
            ChainId::Solana => is_native_token_solana_address(address),
            ChainId::Sui => address == NATIVE_TOKEN_SUI_ADDRESS,
        }
    }

    pub fn wrapped_native_token_address(self) -> String {
        match self {
            ChainId::Solana => WRAPPED_NATIVE_TOKEN_SOLANA_ADDRESS.to_string(),
            ChainId::HyperEVM => WRAPPED_NATIVE_TOKEN_HYPE_ADDRESS.to_string(),
            ChainId::Sui => NATIVE_TOKEN_SUI_ADDRESS.to_string(),
            ChainId::Bsc => "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c".to_string(),
            ChainId::Ethereum => "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".to_string(),
            ChainId::ArbitrumOne => "0x82af49447d8a07e3bd95bd0d56f35241523fbab1".to_string(),
            ChainId::Base => "0x4200000000000000000000000000000000000006".to_string(),
            ChainId::Optimism => "0x4200000000000000000000000000000000000006".to_string(),
            ChainId::Monad => "0x3bd359C1119dA7Da1D913D1C4D2B7c461115433A".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_id_display() {
        assert_eq!(ChainId::Ethereum.to_string(), "Ethereum");
        assert_eq!(ChainId::Bsc.to_string(), "BSC");
        assert_eq!(ChainId::ArbitrumOne.to_string(), "Arbitrum One");
        assert_eq!(ChainId::Base.to_string(), "Base");
        assert_eq!(ChainId::Solana.to_string(), "Solana");
        assert_eq!(ChainId::Sui.to_string(), "Sui");
        assert_eq!(ChainId::Optimism.to_string(), "Optimism");
    }

    #[test]
    fn test_chain_type_display() {
        assert_eq!(ChainType::EVM.to_string(), "EVM");
        assert_eq!(ChainType::Solana.to_string(), "Solana");
        assert_eq!(ChainType::Sui.to_string(), "Sui");
    }

    #[test]
    fn test_is_native_token_evm_address() {
        // Valid addresses
        assert!(is_native_token_evm_address(
            "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
        ));
        assert!(is_native_token_evm_address(
            "0x0000000000000000000000000000000000000000"
        ));

        // Case insensitive checks
        assert!(is_native_token_evm_address(
            "0xEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE"
        ));
        assert!(is_native_token_evm_address(
            "0x0000000000000000000000000000000000000000"
        ));

        // Invalid addresses
        assert!(!is_native_token_evm_address(
            "0x1111111111111111111111111111111111111111"
        ));
        assert!(!is_native_token_evm_address("not_an_address"));
        assert!(!is_native_token_evm_address(""));
    }

    #[test]
    fn test_supported_chains() {
        let chains = ChainId::supported_chains();

        // Test count
        assert!(chains.len() >= 7, "Should have at least 7 supported chains");

        // Test all chains are included
        assert!(chains.contains(&ChainId::Ethereum));
        assert!(chains.contains(&ChainId::Bsc));
        assert!(chains.contains(&ChainId::ArbitrumOne));
        assert!(chains.contains(&ChainId::Base));
        assert!(chains.contains(&ChainId::Solana));
        assert!(chains.contains(&ChainId::Sui));
        assert!(chains.contains(&ChainId::Optimism));
    }

    #[test]
    fn test_to_chain_type() {
        // Test EVM chains
        assert_eq!(ChainId::Ethereum.to_chain_type(), ChainType::EVM);
        assert_eq!(ChainId::Bsc.to_chain_type(), ChainType::EVM);
        assert_eq!(ChainId::ArbitrumOne.to_chain_type(), ChainType::EVM);
        assert_eq!(ChainId::Base.to_chain_type(), ChainType::EVM);
        assert_eq!(ChainId::Optimism.to_chain_type(), ChainType::EVM);

        // Test non-EVM chains
        assert_eq!(ChainId::Solana.to_chain_type(), ChainType::Solana);
        assert_eq!(ChainId::Sui.to_chain_type(), ChainType::Sui);
    }

    #[test]
    fn test_from_u32() {
        // Test valid conversions
        assert_eq!(
            ChainId::try_from(1).expect("Should work"),
            ChainId::Ethereum
        );
        assert_eq!(ChainId::try_from(56).expect("Should work"), ChainId::Bsc);
        assert_eq!(
            ChainId::try_from(42161).expect("Should work"),
            ChainId::ArbitrumOne
        );
        assert_eq!(ChainId::try_from(8453).expect("Should work"), ChainId::Base);
        assert_eq!(
            ChainId::try_from(7565164).expect("Should work"),
            ChainId::Solana
        );
        assert_eq!(ChainId::try_from(101).expect("Should work"), ChainId::Sui);
        assert_eq!(
            ChainId::try_from(10).expect("Should work"),
            ChainId::Optimism
        );
    }

    #[test]
    #[should_panic]
    fn test_from_u32_invalid() {
        // This should panic as 9999 isn't a valid chain ID
        let result = ChainId::try_from(9999);
        println!("{result:?}");
        assert!(result.is_err());
        result.unwrap();
    }

    #[test]
    fn test_chain_id_functions() {
        let base_chain_id = ChainId::Base;
        assert!(ChainId::supported_chains().contains(&base_chain_id));
        assert!(base_chain_id.to_chain_type() == ChainType::EVM);
        assert!(&base_chain_id.to_chain_type().to_string() == "EVM");

        let _: ChainId = 8453u32.try_into().expect("Invalid chain ID");
    }
}
