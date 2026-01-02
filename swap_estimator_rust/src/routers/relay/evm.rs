use crate::error::{Error, EstimatorResult};
use crate::routers::estimate::{GenericEstimateRequest, GenericEstimateResponse};
use crate::routers::relay::relay::{get_amounts_from_quote, quote_relay_generic};
use crate::routers::relay::requests::RelayQuoteRequest;
use crate::routers::relay::responses::RelayEvmTxData;
use crate::routers::swap::{EvmSwapResponse, EvmTxData, GenericSwapRequest};
use crate::routers::{RouterType, Slippage};
use crate::utils::evm::{
    ERC20_APPROVE_CALLDATA_LEN, ERC20_APPROVE_SELECTOR, replace_amount_limit_in_tx,
};
use error_stack::{ResultExt, report};
use intents_models::network::client_rate_limit::Client;

pub async fn estimate_relay_evm(
    client: &Client,
    request: GenericEstimateRequest,
) -> EstimatorResult<GenericEstimateResponse> {
    let trade_type = request.trade_type;
    let quote_request = RelayQuoteRequest::from_generic_estimate_request(request, None, None)?;
    let quote_response = quote_relay_generic::<RelayEvmTxData>(client, quote_request).await?;

    let (amount_quote, amount_limit) = get_amounts_from_quote(&quote_response, trade_type)?;

    Ok(GenericEstimateResponse {
        amount_quote,
        amount_limit,
        router: RouterType::Relay,
        router_data: serde_json::to_value(quote_response).change_context(
            Error::AggregatorError("Error serializing Relay quote response".to_string()),
        )?,
    })
}

pub async fn swap_relay_evm(
    client: &Client,
    generic_swap_request: GenericSwapRequest,
) -> EstimatorResult<EvmSwapResponse> {
    let trade_type = generic_swap_request.trade_type;
    let estimate_request = GenericEstimateRequest::from(generic_swap_request.clone());
    let quote_request = RelayQuoteRequest::from_generic_estimate_request(
        estimate_request,
        Some(generic_swap_request.spender.clone()),
        Some(generic_swap_request.dest_address.clone()),
    )?;
    let quote_response = quote_relay_generic::<RelayEvmTxData>(client, quote_request).await?;

    let (amount_quote, amount_limit) = get_amounts_from_quote(&quote_response, trade_type)?;

    let mut relay_txs: Vec<RelayEvmTxData> = vec![];
    let mut maybe_approval_tx: Option<RelayEvmTxData> = None;
    for step in quote_response.steps.iter() {
        for item in step.items.iter() {
            if step.id.eq("approve") && maybe_approval_tx.is_none() {
                maybe_approval_tx = Some(item.data.clone());
            } else {
                relay_txs.push(item.data.clone());
            }
        }
    }

    if relay_txs.is_empty() {
        return Err(report!(Error::ResponseError)
            .attach_printable("No swap transaction found in Relay response"));
    }

    let swap_tx = relay_txs
        .pop()
        .ok_or(report!(Error::LogicError("No swap tx".to_string())))?;

    let mut pre_transactions: Vec<EvmTxData> = vec![];
    let mut approve_address: Option<String> = None;

    if let Some(maybe_approval_tx) = maybe_approval_tx {
        let is_approval_calldata = maybe_approval_tx.data.starts_with(ERC20_APPROVE_SELECTOR)
            && maybe_approval_tx.data.len() == ERC20_APPROVE_CALLDATA_LEN;
        if is_approval_calldata
            && maybe_approval_tx
                .to
                .eq_ignore_ascii_case(&generic_swap_request.src_token)
        {
            let spender = format!("0x{}", &maybe_approval_tx.data[34..74]);
            if !spender.eq_ignore_ascii_case(&swap_tx.to) {
                // If they ask us to approve to different address - then we set `approve_address`
                approve_address = Some(spender);
            }
        } else {
            // If this is not "Approve token IN" transaction - we count it as pre_transaction
            pre_transactions.push(maybe_approval_tx.to_evm_tx_data()?);
        }
    }

    // Handling unlikely but possible other transactions
    for relay_tx in relay_txs {
        pre_transactions.push(relay_tx.to_evm_tx_data()?);
    }

    let mut swap_tx = swap_tx.to_evm_tx_data()?;

    if let Slippage::AmountLimit {
        amount_limit: requested_amount_limit,
        ..
    } = generic_swap_request.slippage
    {
        swap_tx.tx_data = replace_amount_limit_in_tx(
            swap_tx.tx_data,
            trade_type,
            amount_quote,
            amount_limit,
            requested_amount_limit,
        )?;
    }

    Ok(EvmSwapResponse {
        amount_quote,
        amount_limit,
        pre_transactions: if pre_transactions.is_empty() {
            None
        } else {
            Some(pre_transactions)
        },
        tx_to: swap_tx.tx_to,
        tx_data: swap_tx.tx_data,
        tx_value: swap_tx.tx_value,
        approve_address,
        // Relay sends tokens to receiver
        require_transfer: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::routers::{Slippage, estimate::TradeType};
    use intents_models::constants::chains::ChainId;

    #[tokio::test]
    async fn test_estimate_relay_evm_exact_in() {
        dotenv::dotenv().ok();
        let client = Client::Unrestricted(reqwest::Client::new());

        let request = GenericEstimateRequest {
            trade_type: TradeType::ExactIn,
            chain_id: ChainId::Base,
            src_token: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string(),
            dest_token: "0x4200000000000000000000000000000000000006".to_string(),
            amount_fixed: 100000000,
            slippage: Slippage::Percent(2.0),
        };
        let result = estimate_relay_evm(&client, request).await;
        assert!(
            result.is_ok(),
            "Expected a successful estimate swap response"
        );
        let response = result.unwrap();
        println!("Response: {response:?}");
        assert!(
            response.amount_quote > 0,
            "Expected a non-zero amount quote"
        );
    }

    #[tokio::test]
    async fn test_relay_swap_exact_in() {
        dotenv::dotenv().ok();
        let client = Client::Unrestricted(reqwest::Client::new());

        let chain_id = ChainId::Base;
        let src_token = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string();
        let dest_token = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string();
        let swap_request = GenericSwapRequest {
            trade_type: TradeType::ExactIn,
            chain_id,
            spender: "0x9ecDC9aF2a8254DdE8bbce8778eFAe695044cC9F".to_string(),
            dest_address: "0x4E28f22DE1DBDe92310db2779217a74607691038".to_string(),
            src_token,
            dest_token,
            amount_fixed: 10_000_000_000u128,
            slippage: Slippage::Percent(2.0),
        };

        let swap_result = swap_relay_evm(&client, swap_request).await;
        assert!(swap_result.is_ok());
        let result = swap_result.unwrap();
        assert!(result.approve_address.is_none());
        assert!(!result.require_transfer);
        assert!(result.pre_transactions.is_none());
    }

    #[tokio::test]
    async fn test_relay_swap_exact_out() {
        dotenv::dotenv().ok();
        let client = Client::Unrestricted(reqwest::Client::new());

        let chain_id = ChainId::Base;
        let src_token = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string();
        let dest_token = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string();
        let request = GenericSwapRequest {
            trade_type: TradeType::ExactOut,
            chain_id,
            spender: "0x9ecDC9aF2a8254DdE8bbce8778eFAe695044cC9F".to_string(),
            dest_address: "0x4E28f22DE1DBDe92310db2779217a74607691038".to_string(),
            src_token,
            dest_token,
            amount_fixed: 10_000_000_000_000_000u128,
            slippage: Slippage::Percent(2.0),
        };
        let swap_result = swap_relay_evm(&client, request).await;
        assert!(swap_result.is_ok());
        let swap_result = swap_result.unwrap();
        assert!(swap_result.approve_address.is_none());
        assert!(!swap_result.require_transfer);
        assert!(swap_result.pre_transactions.is_none());
        assert!(swap_result.amount_quote < 1_000_000_000)
    }

    #[tokio::test]
    async fn test_relay_swap_exact_in_with_quote() {
        dotenv::dotenv().ok();
        let client = Client::Unrestricted(reqwest::Client::new());

        let chain_id = ChainId::Base;
        let src_token = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string();
        let dest_token = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string();
        let swap_request = GenericSwapRequest {
            trade_type: TradeType::ExactIn,
            chain_id,
            spender: "0x9ecDC9aF2a8254DdE8bbce8778eFAe695044cC9F".to_string(),
            dest_address: "0x4E28f22DE1DBDe92310db2779217a74607691038".to_string(),
            src_token,
            dest_token,
            amount_fixed: 10_000_000_000u128,
            slippage: Slippage::Percent(2.0),
        };

        let swap_result = swap_relay_evm(&client, swap_request).await;
        assert!(swap_result.is_ok());
        let result = swap_result.unwrap();
        assert!(result.approve_address.is_none());
        assert!(!result.require_transfer);
        assert!(result.pre_transactions.is_none());
    }

    #[tokio::test]
    async fn test_relay_swap_exact_in_with_quote_amount_limit() {
        dotenv::dotenv().ok();
        let chain_id = ChainId::Base;
        let client = Client::Unrestricted(reqwest::Client::new());

        let src_token = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string();
        let dest_token = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string();
        let swap_request = GenericSwapRequest {
            trade_type: TradeType::ExactIn,
            chain_id,
            spender: "0x9ecDC9aF2a8254DdE8bbce8778eFAe695044cC9F".to_string(),
            dest_address: "0x4E28f22DE1DBDe92310db2779217a74607691038".to_string(),
            src_token,
            dest_token,
            amount_fixed: 1_000_000_000_000_000_000u128,
            slippage: Slippage::AmountLimit {
                amount_limit: 1_123,
                fallback_slippage: 2.0,
            },
        };
        let requested_amount_limit_hex = format!("{:064x}", 1_123);

        let swap_result = swap_relay_evm(&client, swap_request).await;
        assert!(swap_result.is_ok());
        let swap_response = swap_result.unwrap();
        assert!(swap_response.tx_data.contains(&requested_amount_limit_hex));
    }
}
