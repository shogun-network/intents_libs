use crate::error::{Error, EstimatorResult};
use crate::routers::RouterType;
use crate::routers::estimate::{GenericEstimateRequest, GenericEstimateResponse, TradeType};
use crate::routers::relay::relay::{get_amounts_from_quote, quote_relay_generic};
use crate::routers::relay::requests::RelayQuoteRequest;
use crate::routers::relay::responses::RelayEvmTxData;
use crate::routers::swap::{EvmSwapResponse, EvmTxData, GenericSwapRequest};
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
    spender: String,
) -> EstimatorResult<EvmSwapResponse> {
    let trade_type = generic_swap_request.trade_type;
    let estimate_request = GenericEstimateRequest::from(generic_swap_request.clone());
    let quote_request = RelayQuoteRequest::from_generic_estimate_request(
        estimate_request,
        Some(spender),
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
        let is_approval_calldata =
            maybe_approval_tx.data.starts_with("0x095ea7b3") && maybe_approval_tx.data.len() == 138;
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

    let swap_tx = swap_tx.to_evm_tx_data()?;

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
