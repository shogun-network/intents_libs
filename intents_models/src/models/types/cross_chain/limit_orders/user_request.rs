use crate::constants::chains::ChainId;
use crate::error::{Error, ModelResult};
use crate::models::types::common::{
    CommonLimitOrderData, CommonLimitOrderUserRequestData, TransferDetails,
};
use crate::models::types::cross_chain::{
    CrossChainChainSpecificData, CrossChainGenericData, CrossChainLimitOrderGenericData,
    CrossChainLimitOrderIntentRequest,
};
use crate::models::types::user_types::IntentRequest;
use error_stack::{ResultExt, report};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, PickFirst, serde_as};
use sha2::Digest;

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Cross chain limit order intent request, received from the user
pub struct CrossChainLimitOrderUserIntentRequest {
    /// Contains the common data for the intent
    pub generic_data: CrossChainLimitOrderGenericRequestData,
    /// Contains chain-specific data
    pub chain_specific_data: CrossChainChainSpecificData,
    /// JSON string of additional execution details
    pub execution_details: String,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// A structure to hold generic data related to the cross chain limit order intent
pub struct CrossChainLimitOrderGenericRequestData {
    /// User address initiating the intent
    pub user: String,

    /// Source chain identifier (e.g., Ethereum, Solana)
    pub src_chain_id: ChainId,
    /// The token being spent in the operation (e.g., "ETH", "BTC")
    pub token_in: String,
    /// The amount of the input token to be used in the operation
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub amount_in: u128,
    /// Minimum amount of stablecoins that Tokens IN may be swapped for
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub min_stablecoins_amount: u128,

    /// Deadline for the operation, in Unix timestamp format, in SECONDS
    pub deadline: u64,
    /// SHA-256 hash of `execution_details` JSON String (hex format)
    pub execution_details_hash: String,
}

impl From<CrossChainLimitOrderGenericData> for CrossChainLimitOrderGenericRequestData {
    fn from(value: CrossChainLimitOrderGenericData) -> Self {
        Self {
            user: value.common_data.user,
            src_chain_id: value.common_data.src_chain_id,
            token_in: value.common_data.token_in,
            amount_in: value.amount_in,
            min_stablecoins_amount: value.common_data.min_stablecoins_amount,
            deadline: value.common_data.deadline,
            execution_details_hash: value.common_data.execution_details_hash,
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// A structure to hold execution details of cross chain limit order, provided by the user
pub struct CrossChainLimitOrderExecutionDetails {
    /// Destination chain identifier
    pub dest_chain_id: ChainId,
    /// Token to be received after the operation (e.g., "USDT", "DAI")
    pub token_out: String,
    /// The minimum amount of the output token to be received after the operation
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub amount_out_min: u128,
    /// Destination address for the operation (e.g., recipient address)
    pub destination_address: String,
    /// Requested array of extra transfers with fixed amounts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_transfers: Option<Vec<TransferDetails>>,

    /// Common limit order data to trigger "take profit" or "stop loss" execution
    #[serde(flatten)]
    pub common_limit_order_data: CommonLimitOrderUserRequestData,
}

impl CrossChainLimitOrderUserIntentRequest {
    pub fn try_into_into_intent_request(self) -> ModelResult<IntentRequest> {
        let mut hasher = sha2::Sha256::new();
        hasher.update(&self.execution_details);
        let result = hasher.finalize();
        let execution_details_hash = format!("0x{result:x}");

        if !execution_details_hash.eq_ignore_ascii_case(&self.generic_data.execution_details_hash) {
            tracing::error!(
                "genericData.executionDetailsHash {} doesn't match with executionDetails ({}) SHA-256 hash {}",
                &self.generic_data.execution_details_hash,
                &self.execution_details,
                &execution_details_hash
            );
            return Err(report!(Error::ValidationError)
                .attach_printable("Execution details hash does not match the provided hash."));
        }

        let execution_details: CrossChainLimitOrderExecutionDetails =
            serde_json::from_str(&self.execution_details)
                .change_context(Error::ValidationError)
                .attach_printable("Invalid execution_details object.")?;

        let generic_data = CrossChainLimitOrderGenericData {
            common_data: CrossChainGenericData {
                user: self.generic_data.user.clone(),
                src_chain_id: self.generic_data.src_chain_id,
                token_in: self.generic_data.token_in.clone(),
                min_stablecoins_amount: self.generic_data.min_stablecoins_amount,
                dest_chain_id: execution_details.dest_chain_id,
                token_out: execution_details.token_out.clone(),
                amount_out_min: execution_details.amount_out_min,
                destination_address: execution_details.destination_address.clone(),
                extra_transfers: execution_details.extra_transfers,
                deadline: self.generic_data.deadline,
                execution_details_hash: self.generic_data.execution_details_hash.clone(),
            },
            common_limit_order_data: CommonLimitOrderData {
                take_profit_min_out: execution_details
                    .common_limit_order_data
                    .take_profit_min_out,
                stop_loss: execution_details.common_limit_order_data.stop_loss,
                stop_loss_triggered: false,
            },
            amount_in: self.generic_data.amount_in,
        };

        Ok(IntentRequest::CrossChainLimitOrder(
            CrossChainLimitOrderIntentRequest {
                generic_data,
                chain_specific_data: self.chain_specific_data.clone(),
            },
        ))
    }
}
