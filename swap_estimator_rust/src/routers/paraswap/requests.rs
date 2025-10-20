use super::update_paraswap_native_token;
use crate::{
    error::EstimatorResult,
    routers::{
        Slippage,
        estimate::{GenericEstimateRequest, TradeType},
        paraswap::get_paraswap_max_slippage,
        swap::GenericSwapRequest,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ParaswapParams {
    pub side: ParaswapSide,
    pub chain_id: u32,
    pub amount: u128,
    pub token_in: String,
    pub token_out: String,
    pub token0_decimals: u8,
    pub token1_decimals: u8,
    pub wallet_address: String,
    pub receiver_address: String,
    pub slippage: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::upper_case_acronyms)]
pub enum ParaswapSide {
    SELL,
    BUY,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetPriceRouteRequest {
    /// Source Token Address. Instead Token Symbol could be used for tokens listed in the /tokens endpoint.
    #[serde(rename = "srcToken")]
    pub src_token: String,
    /// Source Token Decimals. (Can be omitted if Token Symbol is used in srcToken).
    #[serde(rename = "srcDecimals")]
    pub src_decimals: u8,
    /// Destination Token Address. Instead Token Symbol could be used for tokens listed in the  /tokens endpoint.
    #[serde(rename = "destToken")]
    pub dest_token: String,
    /// srcToken amount (in case of SELL) or destToken amount (in case of BUY).
    /// The amount should be in WEI/Raw units (eg. 1WBTC -> 100000000)
    pub amount: String,
    /// SELL or BUY.
    /// Default: SELL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<ParaswapSide>,
    /// Network ID. (Mainnet - 1, Optimism - 10, BSC - 56, Polygon - 137, Fantom - 250, zkEVM - 1101, Base - 8453, Arbitrum - 42161, Avalanche - 43114, Gnosis - 100).
    /// Default: 1.
    #[serde(rename = "network")]
    pub chain_id: u32,
    /// If provided, others object is filled in the response with price quotes from other exchanges (if available for comparison).
    /// Default: false
    #[serde(rename = "otherExchangePrices")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_exchange_prices: Option<bool>,
    /// Comma Separated List of DEXs to include.
    /// All supported DEXs by chain can be found here
    /// eg: UniswapV3, CurveV1
    #[serde(rename = "includeDEXS")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_dexs: Option<String>,
    /// Comma Separated List of DEXs to exclude.
    /// All supported DEXs by chain can be found here
    /// eg: UniswapV3, CurveV1
    #[serde(rename = "excludeDEXS")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_dexs: Option<String>,
    /// Exclude all RFQs from pricing
    /// eg: AugustusRFQ, Hashflow
    /// Default: false
    #[serde(rename = "excludeRFQ")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_rfq: Option<bool>,
    /// Comma Separated List of Comma Separated List of Contract Methods to include in pricing (without spaces).
    /// View the list of the supported methods for V5 and V6
    /// eg: swapExactAmountIn,swapExactAmountInOnUniswapV2
    #[serde(rename = "includeContractMethods")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_contract_methods: Option<String>,
    /// Comma Separated List of Contract Methods to exclude from pricing (without spaces).
    /// View the list of the supported methods for V5 and V6
    #[serde(rename = "excludeContractMethods")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_contract_methods: Option<String>,
    /// User's Wallet Address.
    #[serde(rename = "userAddress")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_address: Option<String>,
    /// Dash (-) separated list of tokens (addresses or symbols from /tokens) to comprise the price route. Max 4 tokens.
    /// Note: If route is specified, the response will only comprise of the route specified which might not be the optimal route.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    /// Partner string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partner: Option<String>,
    /// Destination Token Decimals. (Can be omitted if Token Symbol is used in destToken).
    #[serde(rename = "destDecimals")]
    pub dest_decimals: u8,
    /// In %. It's a way to bypass the API price impact check (default = 15%).
    #[serde(rename = "maxImpact")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_impact: Option<u32>,
    /// Receiver's Wallet address. (Can be omitted if swapping tokens from and to same account)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver: Option<String>,
    /// If the source token is a tax token, you should specify the tax amount in BPS.
    /// For example: for a token with a 5% tax, you should set it to 500 as [(500/10000)*100=5%]
    /// Note: not all DEXs and contract methods support trading tax tokens, so we will filter those that don't.
    #[serde(rename = "srcTokenTransferFee")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_token_transfer_fee: Option<String>,
    /// If the destination token is a tax token, you should specify the tax amount in BPS.
    /// For example: for a token with a 5% tax, you should set it to 500 as [(500/10000)*100=5%]
    /// Note: not all DEXs and contract methods support trading tax tokens, so we will filter those that don't.
    #[serde(rename = "destTokenTransferFee")]
    pub dest_token_transfer_fee: Option<String>,
    /// If the source token is a tax token, you should specify the tax amount in BPS.
    /// Some tokens only charge tax when swapped in/out DEXs and not on ordinary transfers.
    /// For example: for a token with a 5% tax, you should set it to 500 as [(500/10000)*100=5%]
    /// Note: not all DEXs and contract methods support trading tax tokens, so we will filter those that don't.
    #[serde(rename = "srcTokenDexTransferFee")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_token_dex_transfer_fee: Option<String>,
    /// If the destination token is a tax token, you should specify the tax amount in BPS.
    /// Some tokens only charge tax when swapped in/out DEXs, not on ordinary transfers.
    /// For example: for a token with a 5% tax, you should set it to 500 as [(500/10000)*100=5%]
    /// Note: not all DEXs and contract methods support trading tax tokens, so we will filter those that don't.
    #[serde(rename = "destTokenDexTransferFee")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dest_token_dex_transfer_fee: Option<String>,
    /// To specify the protocol version. Values: 5 or 6.2
    /// Default: 5
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<f64>,
    /// Specify that methods without fee support should be excluded from the price route.
    /// Default: false
    #[serde(rename = "excludeContractMethodsWithoutFeeModel")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_contract_methods_without_fee_model: Option<bool>,
    /// If tokens USD prices are not available, Bad USD Price error will be thrown. Use this param to skip this check.
    /// Default: false
    #[serde(rename = "ignoreBadUsdPrice")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_bad_usd_price: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionsRequest {
    pub chain_id: u32,
    pub query_params: TransactionsQueryParams,
    pub body_params: TransactionsBodyParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionsQueryParams {
    #[serde(rename = "gasPrice")]
    pub gas_price: String,
    #[serde(rename = "ignoreChecks")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_checks: Option<bool>,
    #[serde(rename = "ignoreGasEstimate")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_gas_estimate: Option<bool>,
    #[serde(rename = "onlyParams")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub only_params: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eip1559: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionsBodyParams {
    #[serde(rename = "srcToken")]
    pub src_token: String,
    #[serde(rename = "srcDecimals")]
    pub src_decimals: u8,
    #[serde(rename = "destToken")]
    pub dest_token: String,
    #[serde(rename = "destDecimals")]
    pub dest_decimals: u8,
    #[serde(rename = "srcAmount")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_amount: Option<String>,
    #[serde(rename = "destAmount")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dest_amount: Option<String>,
    #[serde(rename = "priceRoute")]
    pub price_route: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slippage: Option<u32>,
    #[serde(rename = "userAddress")]
    pub user_address: String,
    #[serde(rename = "txOrigin")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_origin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver: Option<String>,
    #[serde(rename = "partnerAddress")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partner_address: Option<String>,
    #[serde(rename = "partnerFeeBps")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partner_fee_bps: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<u64>,
    #[serde(rename = "isCapSurplus")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_cap_surplus: Option<bool>,
    #[serde(rename = "takeSurplus")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub take_surplus: Option<bool>,
    #[serde(rename = "isSirplusToUser")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_surplus_to_user: Option<bool>,
    #[serde(rename = "isDirectFeeTransfer")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_direct_fee_transfer: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParaswapSwapCombinedRequest {
    #[serde(rename = "srcToken")]
    pub src_token: String,
    #[serde(rename = "srcDecimals")]
    pub src_decimals: u8,
    #[serde(rename = "destToken")]
    pub dest_token: String,
    pub amount: String,
    pub side: Option<ParaswapSide>,
    #[serde(rename = "network")]
    pub chain_id: u32,
    #[serde(rename = "otherExchangePrices")]
    pub other_exchange_prices: Option<bool>,
    #[serde(rename = "includeDEXS")]
    pub include_dexs: Option<String>,
    #[serde(rename = "excludeDEXS")]
    pub exclude_dexs: Option<String>,
    #[serde(rename = "excludeRFQ")]
    pub exclude_rfq: Option<bool>,
    #[serde(rename = "includeContractMethods")]
    pub include_contract_methods: Option<String>,
    #[serde(rename = "excludeContractMethods")]
    pub exclude_contract_methods: Option<String>,
    #[serde(rename = "userAddress")]
    pub user_address: String,
    pub route: Option<String>,
    pub partner: Option<String>,
    #[serde(rename = "destDecimals")]
    pub dest_decimals: u8,
    #[serde(rename = "maxImpact")]
    pub max_impact: Option<u32>,
    pub receiver: Option<String>,
    #[serde(rename = "srcTokenTransferFee")]
    pub src_token_transfer_fee: Option<String>,
    #[serde(rename = "destTokenTransferFee")]
    pub dest_token_transfer_fee: Option<String>,
    #[serde(rename = "srcTokenDexTransferFee")]
    pub src_token_dex_transfer_fee: Option<String>,
    #[serde(rename = "destTokenDexTransferFee")]
    pub dest_token_dex_transfer_fee: Option<String>,
    pub version: Option<f64>,
    #[serde(rename = "excludeContractMethodsWithoutFeeModel")]
    pub exclude_contract_methods_without_fee_model: Option<bool>,
    #[serde(rename = "ignoreBadUsdPrice")]
    pub ignore_bad_usd_price: Option<bool>,
    #[serde(rename = "gasPrice")]
    pub gas_price: String,
    #[serde(rename = "ignoreChecks")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_checks: Option<bool>,
    #[serde(rename = "ignoreGasEstimate")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_gas_estimate: Option<bool>,
    #[serde(rename = "onlyParams")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub only_params: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eip1559: Option<bool>,
    #[serde(rename = "srcAmount")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_amount: Option<String>,
    #[serde(rename = "destAmount")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dest_amount: Option<String>,
    #[serde(rename = "priceRoute")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_route: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slippage: Option<u32>,
    #[serde(rename = "txOrigin")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_origin: Option<String>,
    #[serde(rename = "partnerAddress")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partner_address: Option<String>,
    #[serde(rename = "partnerFeeBps")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partner_fee_bps: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<u64>,
    #[serde(rename = "isCapSurplus")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_cap_surplus: Option<bool>,
    #[serde(rename = "takeSurplus")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub take_surplus: Option<bool>,
    #[serde(rename = "isSirplusToUser")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_surplus_to_user: Option<bool>,
    #[serde(rename = "isDirectFeeTransfer")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_direct_fee_transfer: Option<bool>,
}

impl GetPriceRouteRequest {
    pub fn from_generic_estimate_request(
        request: &GenericEstimateRequest,
        src_decimals: u8,
        dest_decimals: u8,
    ) -> Self {
        let src_token = update_paraswap_native_token(request.src_token.clone()).to_string();
        let dest_token = update_paraswap_native_token(request.dest_token.clone()).to_string();
        Self {
            src_token,
            src_decimals,
            dest_token,
            dest_decimals,
            amount: request.amount_fixed.to_string(),
            side: Some(match request.trade_type {
                TradeType::ExactIn => ParaswapSide::SELL,
                TradeType::ExactOut => ParaswapSide::BUY,
            }),
            chain_id: request.chain_id as u32,
            other_exchange_prices: None,
            include_dexs: None,
            exclude_dexs: None,
            exclude_rfq: None,
            include_contract_methods: None,
            exclude_contract_methods: None,
            user_address: None,
            route: None,
            partner: None,
            max_impact: None,
            receiver: None,
            src_token_transfer_fee: None,
            dest_token_transfer_fee: None,
            src_token_dex_transfer_fee: None,
            dest_token_dex_transfer_fee: None,
            version: Some(6.2),
            exclude_contract_methods_without_fee_model: None,
            ignore_bad_usd_price: None,
        }
    }
}

impl TransactionsRequest {
    pub fn from_generic_swap_request(
        request: &GenericSwapRequest,
        src_decimals: u8,
        dest_decimals: u8,
        price_route: Value,
    ) -> EstimatorResult<Self> {
        let src_token = update_paraswap_native_token(request.src_token.clone()).to_string();
        let dest_token = update_paraswap_native_token(request.dest_token.clone()).to_string();
        let (src_amount, dest_amount, slippage) = {
            let (slippage, amount_limit) = match request.slippage {
                Slippage::Percent(slippage) => (Some((slippage * 100.0) as u32), None),
                Slippage::AmountLimit {
                    amount_limit,
                    fallback_slippage: _,
                } => (None, Some(amount_limit)),
                Slippage::MaxSlippage => (Some(get_paraswap_max_slippage()), None),
            };
            let (src_amount, dest_amount) = match request.trade_type {
                TradeType::ExactIn => (Some(request.amount_fixed), amount_limit),
                TradeType::ExactOut => (amount_limit, Some(request.amount_fixed)),
            };
            (src_amount, dest_amount, slippage)
        };

        Ok(Self {
            chain_id: request.chain_id as u32,
            query_params: TransactionsQueryParams {
                gas_price: "0".to_string(),
                ignore_checks: Some(true),
                ignore_gas_estimate: Some(true),
                only_params: Some(false),
                eip1559: None,
            },
            body_params: TransactionsBodyParams {
                src_token,
                src_decimals,
                dest_token,
                dest_decimals,
                src_amount: src_amount.map(|amt| amt.to_string()),
                dest_amount: dest_amount.map(|amt| amt.to_string()),
                price_route,
                slippage,
                user_address: request.spender.to_string(),
                tx_origin: None,
                receiver: Some(request.dest_address.to_string()),
                partner_address: None,
                partner_fee_bps: None,
                partner: None,
                permit: None,
                deadline: None,
                is_cap_surplus: None,
                take_surplus: None,
                is_surplus_to_user: None,
                is_direct_fee_transfer: None,
            },
        })
    }
}
