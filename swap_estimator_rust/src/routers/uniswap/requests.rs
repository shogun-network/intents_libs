use crate::routers::Slippage;
use crate::routers::estimate::{GenericEstimateRequest, TradeType};
use crate::routers::uniswap::{get_uniswap_max_slippage, update_uniswap_native_token};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub static SWAPPER_PLACEHOLDER: &str = "0x1234567890098765432112345678900987654321";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(non_camel_case_types)]
pub enum UniswapQuoteType {
    #[default]
    EXACT_INPUT,
    EXACT_OUTPUT,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(non_camel_case_types)]
pub enum UniswapRoutingPreferences {
    #[default]
    BEST_PRICE,
    FASTEST,
    CLASSIC,
    BEST_PRICE_V2,
    UNISWAPX_V2,
    V3_ONLY,
    V2_ONLY,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum UniswapProtocol {
    V2,
    V3,
    V4,
    UNISWAPX_V2,
    UNISWAPX_V3,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(non_camel_case_types)]
pub enum UniswapV4HookOptions {
    #[default]
    V4_HOOKS_INCLUSIVE,
    V4_HOOKS_ONLY,
    V4_NO_HOOKS,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(non_camel_case_types)]
pub enum UniswapSpreadOptimisation {
    #[default]
    EXECUTION,
    PRICE,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(non_camel_case_types)]
pub enum UniswapUrgency {
    #[default]
    normal,
    fast,
    urgent,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(non_camel_case_types)]
pub enum UniswapPermitAmount {
    FULL,
    #[default]
    EXACT,
}

// https://api-docs.uniswap.org/api-reference/swapping/quote
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UniswapQuoteRequest {
    // The handling of the amount field. EXACT_INPUT means the requester will send the specified amount of input
    // tokens and get a quote with a variable quantity of output tokens. EXACT_OUTPUT means the requester will
    // receive the specified amount of output tokens and get a quote with a variable quantity of input tokens.
    #[serde(rename = "type")]
    pub quote_type: UniswapQuoteType,
    // The quantity of tokens denominated in the token's base units. (For example, for an ERC20 token one token
    // is 1x10^18 base units. For one USDC token one token is 1x10^6 base units.) This value must be greater than 0.
    pub amount: String,
    // The unique ID of the blockchain. For a list of supported chains see the FAQ.
    // https://api-docs.uniswap.org/guides/supported_chains
    pub token_in_chain_id: u32,
    // The unique ID of the blockchain. For a list of supported chains see the FAQ.
    // https://api-docs.uniswap.org/guides/supported_chains
    pub token_out_chain_id: u32,
    // The token which will be sent, specified by its token address. For a list of supported tokens, see the FAQ.
    pub token_in: String,
    // The token which will be received, specified by its token address. For a list of supported tokens, see the FAQ.
    pub token_out: String,
    // Indicates whether you want to receive a permit2 transaction to sign and submit onchain, or a permit message to sign.
    // When set to `true`, the quote response returns the Permit2 as a calldata which the user signs and broadcasts.
    // When set to `false` (the default), the quote response returns the Permit2 as a message which the user signs but
    // does not need to broadcast. When using a 7702-delegated wallet, set this field to `true`. Except for this scenario,
    // it is recommended that this field is set to `false`. Note that a Permit2 calldata (e.g. `true`), will provide
    // indefinite permission for Permit2 to spend a token, in contrast to a Permit2 message (e.g. `false`) which is
    // only valid for 30 days. Further, a Permit2 calldata (e.g. `true`) requires the user to pay gas to submit the
    // transaction, whereas the Permit2 message (e.g. `false` ) does not require the user to submit a transaction and
    // requires no gas.
    pub generate_permit_as_transaction: bool,
    // The wallet address which will be used to send the token.
    pub swapper: String,
    // The slippage tolerance as a percentage up to a maximum of two decimal places.
    // For Uniswap Protocols (v2, v3, v4), the slippage tolerance is the maximum amount the price can change
    // between the time the transaction is submitted and the time it is executed. The slippage tolerance is a
    // percentage of the total value of the swap.
    //
    // When submitting a quote, note that slippage tolerance works differently in UniswapX swaps where it does
    // not set a limit on the Spread in an order. See here (https://api-docs.uniswap.org/guides/faqs) for more information.
    //
    // Note that if the trade type is `EXACT_INPUT`, then the slippage is in terms of the output token. If the
    // trade type is `EXACT_OUTPUT`, then the slippage is in terms of the input token.
    //
    // When submitting a request, `slippage_tolerance` may not be set when `auto_slippage` is defined. One of
    // `slippage_tolerance` or `auto_slippage` must be defined.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slippage_tolerance: Option<f64>,
    // The auto slippage strategy to employ. Presently there is a single slippage strategy, "DEFAULT",
    // which uses a  combination of the estimated gas cost and swap size to calcualte a slippage.
    // Note that the DEFAULT slippage strategy is bounded between (and including) 0.5% and 5.5%.
    //
    // If the trade type is EXACT_INPUT, then the slippage is in terms of the output token.
    // If the trade type is EXACT_OUTPUT, then the slippage is in terms of the input token.
    //
    // When submitting a request, auto_slippage may not be set when slippage_tolerance is defined.
    // One of slippage_tolerance or auto_slippage must be defined.
    // Available options: DEFAULT
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_slippage: Option<String>,
    // The routing_preference specifies the preferred strategy to determine the quote.
    // If the routing_preference is BEST_PRICE, then the quote will propose a route through the specified
    // whitelisted protocols (or all, if none are specified) that provides the best price.
    // When the routing_preference is FASTEST, the quote will propose the first route which is found to complete the swap.
    // Note that the values CLASSIC, BEST_PRICE_V2, UNISWAPX_V2, V3_ONLY, and V2_ONLY are deprecated and will be
    // removed in a future release. See the Token Trading Workflow page for more information.
    // Available options:
    //  - BEST_PRICE,
    //  - FASTEST,
    //  - CLASSIC,
    //  - BEST_PRICE_V2,
    //  - UNISWAPX_V2,
    //  - V3_ONLY,
    //  - V2_ONLY
    //
    // default:BEST_PRICE
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing_preference: Option<UniswapRoutingPreferences>,
    // The protocols to use for the swap/order. If the protocols field is defined,
    // then you can only set the routing_preference to BEST_PRICE.
    pub protocols: Vec<UniswapProtocol>,
    // The hook options to use for V4 pool quotes.
    // V4_HOOKS_INCLUSIVE will get quotes for V4 pools with or without hooks.
    // V4_HOOKS_ONLY will only get quotes for V4 pools with hooks.
    // V4_NO_HOOKS will only get quotes for V4 pools without hooks.
    // Defaults to V4_HOOKS_INCLUSIVE if V4 is included in protocols and hookOptions is not set.
    // This field is ignored if V4 is not passed in protocols.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks_options: Option<UniswapV4HookOptions>,
    // For UniswapX swaps, when set to EXECUTION, quotes optimize for looser spreads at higher fill rates.
    // When set to PRICE, quotes optimize for tighter spreads at lower fill rates.
    // This field is not applicable to Uniswap Protocols (v2, v3, v4), bridging,
    // or wrapping/unwrapping and will be ignored if set.
    //
    // default: EXECUTION
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spread_optimization: Option<UniswapSpreadOptimisation>,
    // The urgency impacts the estimated gas price of the transaction. The higher the urgency, the higher the gas price,
    // and the faster the transaction is likely to be selected from the mempool. The default value is `urgent`.
    //
    // default: urgent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urgency: Option<UniswapUrgency>,
    // For Uniswap Protocols (v2, v3, v4) swaps, specify the input token spend allowance (e.g. quantity) to be set in the
    // permit. FULL can be used to specify an unlimited token quantity, and may prevent the wallet from needing to sign
    // another permit for the same token in the future. EXACT can be used to specify the exact input token quantity for
    // this request. Defaults to FULL.
    //
    // default: FULL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permit_amount: Option<UniswapPermitAmount>,
}

impl UniswapQuoteRequest {
    pub fn from_generic_estimate_request(
        request: GenericEstimateRequest,
        swapper: Option<String>,
    ) -> Self {
        Self {
            quote_type: match request.trade_type {
                TradeType::ExactIn => UniswapQuoteType::EXACT_INPUT,
                TradeType::ExactOut => UniswapQuoteType::EXACT_OUTPUT,
            },
            amount: request.amount_fixed.to_string(),
            token_in_chain_id: request.chain_id as u32,
            token_out_chain_id: request.chain_id as u32,
            token_in: update_uniswap_native_token(request.src_token),
            token_out: update_uniswap_native_token(request.dest_token),
            generate_permit_as_transaction: true,
            // Setting swapper to dummy address for the cases we don't have any
            swapper: swapper.unwrap_or(SWAPPER_PLACEHOLDER.to_string()),
            slippage_tolerance: Some(match request.slippage {
                Slippage::Percent(slippage) => slippage,
                Slippage::AmountLimit {
                    fallback_slippage, ..
                } => fallback_slippage,
                Slippage::MaxSlippage => get_uniswap_max_slippage(),
            }),
            auto_slippage: None,
            routing_preference: Some(UniswapRoutingPreferences::BEST_PRICE),
            protocols: vec![
                UniswapProtocol::V2,
                UniswapProtocol::V3,
                UniswapProtocol::V4,
            ],
            hooks_options: Some(UniswapV4HookOptions::V4_HOOKS_INCLUSIVE),
            spread_optimization: Some(UniswapSpreadOptimisation::EXECUTION),
            urgency: Some(UniswapUrgency::normal),
            permit_amount: Some(UniswapPermitAmount::EXACT),
        }
    }
}

// https://api-docs.uniswap.org/api-reference/swapping/create_protocol_swap
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UniswapSwapRequest {
    pub quote: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    // Use refresh_gas_price instead.
    // DEPRECATED
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_gas_info: Option<bool>,
    // If true, the gas price will be re-fetched from the network.
    // default:false
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_gas_price: Option<bool>,
    // If true, the transaction will be simulated. If the simulation results on an onchain error, endpoint will return an error.
    // default:false
    #[serde(skip_serializing_if = "Option::is_none")]
    pub simulate_transaction: Option<bool>,
    // the permit2 message object for the customer to sign to permit spending by the permit2 contract.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permit_data: Option<Value>,
    // Swap safety mode will automatically sweep the transaction for the native token and return it to the sender
    // wallet address.
    // This is to prevent accidental loss of funds in the event that the token amount is set in the transaction
    // value instead of as part of the calldata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_mode: Option<String>,
    // The unix timestamp in seconds at which the order will be reverted if not filled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<u64>,
    // The urgency impacts the estimated gas price of the transaction. The higher the urgency, the higher the gas price,
    // and the faster the transaction is likely to be selected from the mempool. The default value is urgent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urgency: Option<UniswapUrgency>,
}

impl UniswapSwapRequest {
    pub fn from_quote(quote: Value) -> Self {
        Self {
            quote,
            signature: None,
            include_gas_info: Some(false),
            refresh_gas_price: Some(false),
            simulate_transaction: Some(false),
            permit_data: None,
            safety_mode: None,
            deadline: None,
            urgency: Some(UniswapUrgency::normal),
        }
    }
}
