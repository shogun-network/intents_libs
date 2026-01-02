use crate::error::EstimatorResult;
use crate::routers::Slippage;
use crate::routers::estimate::{GenericEstimateRequest, TradeType};
use crate::routers::relay::{
    get_relay_max_slippage, update_relay_chain_id, update_relay_native_token,
};
use crate::utils::number_conversion::slippage_to_bps;
use serde::{Deserialize, Serialize};

pub static USER_PLACEHOLDER: &str = "0x1234567890098765432112345678900987654321";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum RelayTradeType {
    EXACT_INPUT,
    EXACT_OUTPUT,
    EXPECTED_OUTPUT,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayRequestedTx {
    pub to: String,
    pub value: String,
    pub data: String,
}

// https://docs.relay.link/references/api/get-quote
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayQuoteRequest {
    // Address that is depositing funds on the origin chain and submitting transactions or signatures
    pub user: String,
    pub origin_chain_id: u32,
    pub destination_chain_id: u32,
    pub origin_currency: String,
    pub destination_currency: String,
    // Amount to swap as the base amount (can be switched to exact input/output using the dedicated flag),
    // denoted in the smallest unit of the specified currency (e.g., wei for ETH)
    pub amount: String,
    // Whether to use the amount as the output or the input for the basis of the swap
    pub trade_type: RelayTradeType,

    // Address that is receiving the funds on the destination chain, if not specified then this will default to the user address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipient: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txs: Option<Vec<RelayRequestedTx>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referrer: Option<String>,
    // Address to send the refund to in the case of failure,
    // if not specified then the recipient address or user address is used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refund_to: Option<String>,
    // Always refund on the origin chain in case of any issues
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refund_on_origin: Option<bool>,
    // Enable this to route payments via a receiver contract. This contract will emit an event
    // when receiving payments before forwarding to the solver.
    // This is needed when depositing from a smart contract as the payment will be an internal transaction
    // and detecting such a transaction requires obtaining the transaction traces.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_receiver: Option<bool>,
    // Enabling will send any swap surplus when doing exact output operations to the solver EOA,
    // otherwise it will be swept to the recipient
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_true_exact_output: Option<bool>,
    // Enable this to use canonical+ bridging, trading speed for more liquidity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_external_liquidity: Option<bool>,
    // Enable this to use permit (eip3009) when bridging, only works on supported currency such as usdc
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_permit: Option<bool>,
    // Enable this to use a deposit address when bridging, in scenarios where calldata cannot be sent alongside
    // the transaction. only works on native currency bridges
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_deposit_address: Option<bool>,
    // Slippage tolerance for the swap, if not specified then the slippage tolerance is automatically calculated to
    // avoid front-running. This value is in basis points (1/100th of a percent), e.g. 50 for 0.5% slippage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slippage_tolerance: Option<String>,
    // If the request involves specifying transactions to be executed during the deposit transaction, an explicit gas
    // limit must be set when requesting the quote
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_limit_for_deposit_specified_txs: Option<u64>,
    // Gas overhead for 4337 user operations, to be used for fees calculation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_operation_gas_overhead: Option<u64>,
    // Force executing swap requests via the solver (by default, same-chain swap requests are self-executed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_solver_execution: Option<bool>,

    // ************************ SOLANA ONLY PARAMS ************************
    // Whether to include compute unit limit instruction for solana origin requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_compute_unit_limit: Option<bool>,

    // ************************ BRIDGE ONLY PARAMS ************************

    // If set, the destination fill will include a gas topup to the recipient (only supported for EVM chains if the
    // requested currency is not the gas currency on the destination chain)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topup_gas: Option<bool>,
    // The destination gas topup amount in USD decimal format, e.g 100000 = $1.
    // topup_gas is required to be enabled. Defaults to 2000000 ($2)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topup_gas_amount: Option<String>,
}

impl RelayQuoteRequest {
    pub fn from_generic_estimate_request(
        request: GenericEstimateRequest,
        user: Option<String>,
        recipient: Option<String>,
    ) -> EstimatorResult<Self> {
        Ok(Self {
            user: user.unwrap_or(USER_PLACEHOLDER.to_string()),
            origin_chain_id: update_relay_chain_id(request.chain_id),
            destination_chain_id: update_relay_chain_id(request.chain_id),
            origin_currency: update_relay_native_token(request.src_token),
            destination_currency: update_relay_native_token(request.dest_token),
            amount: request.amount_fixed.to_string(),
            trade_type: match request.trade_type {
                TradeType::ExactIn => RelayTradeType::EXACT_INPUT,
                TradeType::ExactOut => RelayTradeType::EXACT_OUTPUT,
            },
            recipient,
            txs: None,
            referrer: Some("gun.fun".to_string()),
            refund_to: None,
            refund_on_origin: Some(true),
            use_receiver: Some(true),
            // We want to send surplus ourselves
            enable_true_exact_output: Some(false),
            use_external_liquidity: None,
            use_permit: Some(false),
            use_deposit_address: None,
            slippage_tolerance: Some(match request.slippage {
                Slippage::Percent(percent) => slippage_to_bps(percent)?.to_string(),
                Slippage::AmountLimit {
                    fallback_slippage, ..
                } => slippage_to_bps(fallback_slippage)?.to_string(),
                Slippage::MaxSlippage => get_relay_max_slippage().to_string(),
            }),
            gas_limit_for_deposit_specified_txs: None,
            user_operation_gas_overhead: None,
            force_solver_execution: None,
            include_compute_unit_limit: Some(true),
            topup_gas: None,
            topup_gas_amount: None,
        })
    }
}
