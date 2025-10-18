use crate::routers::aftermath::AFTERMATH_BASE_API_URL;
use crate::{
    error::{Error, EstimatorResult},
    routers::{
        Slippage,
        aftermath::responses::AftermathQuoteResponse,
        estimate::{GenericEstimateRequest, GenericEstimateResponse, TradeType},
        swap::GenericSwapRequest,
    },
    utils::limit_amount::get_limit_amount_u64,
};
use error_stack::{ResultExt, report};
use intents_models::network::http::handle_reqwest_response;
use reqwest::Client;
use serde_json::{Value, json};

/// Quotes trade with Aftermath API
///
/// ### Arguments
///
/// * `generic_estimate_request` - Generic Sui estimate requestAPI
///
/// ### Returns
///
/// * Generic estimate response
/// * Response value
pub async fn quote_aftermath_swap(
    generic_estimate_request: GenericEstimateRequest,
) -> EstimatorResult<(GenericEstimateResponse, Value)> {
    let GenericEstimateRequest {
        trade_type,
        src_token,
        dest_token,
        amount_fixed,
        slippage,
        chain_id: _,
    } = generic_estimate_request;
    // subtracting 1.0 since Aftermath already adds 1% by default
    let slippage = match slippage {
        Slippage::Percent(slippage) => slippage,
        Slippage::AmountLimit(_) => {
            return Err(report!(Error::ModelsError).attach_printable(
                "Aftermath API route endpoint only supports slippage in percent form",
            ));
        }
    };
    let aftermath_slippage = get_aftermath_slippage(slippage);

    let body: Value = match generic_estimate_request.trade_type {
        TradeType::ExactIn => json!({
            "coinInType": &src_token,
            "coinInAmount": amount_fixed.to_string(),
            "coinOutType": &dest_token
        }),
        TradeType::ExactOut => json!({
            "coinInType": &src_token,
            "coinOutAmount": amount_fixed.to_string(),
            "coinOutType": &dest_token,
            "slippage": aftermath_slippage
        }),
    };

    let response = send_aftermath_request("/router/trade-route", &body).await?;
    let decoded_response: AftermathQuoteResponse = serde_json::from_value(response.clone())
        .change_context(Error::SerdeSerialize(
            "Failed to deserialize Aftermath quote response".to_string(),
        ))?;

    let amount_in: u64 = decoded_response
        .coin_in
        .amount
        .trim_end_matches("n")
        .parse::<u64>()
        .change_context(Error::ParseError)?;

    let amount_out: u64 = decoded_response
        .coin_out
        .amount
        .trim_end_matches("n")
        .parse::<u64>()
        .change_context(Error::ParseError)?;

    if trade_type == TradeType::ExactOut && (amount_out as u128) < amount_fixed {
        return Err(report!(Error::Unknown).attach_printable(format!(
            "Aftermath returned amount_out {amount_out} < amount_fixed {amount_fixed}"
        )));
    };

    let generic_response = match trade_type {
        TradeType::ExactIn => GenericEstimateResponse {
            amount_quote: amount_out as u128,
            amount_limit: get_limit_amount_u64(trade_type, amount_out, slippage) as u128,
        },
        TradeType::ExactOut => GenericEstimateResponse {
            amount_quote: amount_in as u128,
            // Aftermath exact OUT is in fact exact IN,
            // so we know exact amount of tokens we're going to spend
            amount_limit: amount_in as u128,
        },
    };

    Ok((generic_response, response))
}

pub async fn prepare_swap_ptb_with_aftermath(
    generic_swap_request: GenericSwapRequest,
    routes_value: Value,
    serialized_tx_and_coin_id: Option<(Value, Value)>,
) -> EstimatorResult<Value> {
    let GenericSwapRequest {
        trade_type: _,
        dest_address,
        src_token: _,
        dest_token: _,
        spender,
        amount_fixed: _,
        slippage,
        chain_id: _,
    } = generic_swap_request;
    println!("Routes Value: {routes_value:#?}",);
    let slippage = match slippage {
        Slippage::Percent(slippage) => slippage,
        Slippage::AmountLimit(amount_limit) => {
            todo!();
        }
    };
    let aftermath_slippage = get_aftermath_slippage(slippage);

    let (body, uri_path) = match serialized_tx_and_coin_id {
        Some((serialized_tx, coin_id)) => (
            json!({
                "walletAddress": spender,
                "completeRoute": routes_value,
                "slippage": aftermath_slippage,
                "serializedTx": serialized_tx.to_string(),
                "coinInId": coin_id,
            }),
            "/router/transactions/add-trade".to_string(),
        ),
        None => {
            let mut body = json!({
                "walletAddress": spender,
                "completeRoute": routes_value,
                "slippage": aftermath_slippage,
            });
            if !spender.eq_ignore_ascii_case(&dest_address) {
                body["customRecipient"] = json!(dest_address);
            }
            (body, "/router/transactions/trade".to_string())
        }
    };

    send_aftermath_request(&uri_path, &body).await
}

pub async fn send_aftermath_request(uri_path: &str, body: &Value) -> EstimatorResult<Value> {
    let client = Client::new();
    let request = client
        .post(format!("{AFTERMATH_BASE_API_URL}{uri_path}"))
        .json(body);

    let response = request.send().await.change_context(Error::ReqwestError)?;

    let aftermath_response: Value = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;

    Ok(aftermath_response)
}

fn get_aftermath_slippage(slippage: f64) -> f64 {
    // subtracting 1.0 since Aftermath already adds 1% by default
    (slippage - 1.0) / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use intents_models::constants::chains::ChainId;

    const TEST_TX: &'static str = "{\"version\":1,\"sender\":\"0xd422530e3f19bdd09baccfdaf8754ff9b5db01df825a96a581a1236c9b8edf84\",\"expiration\":null,\"gasConfig\":{\"owner\":\"0xd422530e3f19bdd09baccfdaf8754ff9b5db01df825a96a581a1236c9b8edf84\"},\"inputs\":[{\"kind\":\"Input\",\"index\":0,\"value\":{\"Object\":{\"ImmOrOwned\":{\"objectId\":\"0x3f11d40f61d9f20b5488a6d0aa71bcf0a9f0079c4f2d6405c1b72c0c021a79eb\",\"version\":594927392,\"digest\":\"AsbHrWsqkFmH8efJ9CdXVqshG71CAaZtemJKvaaBHWSJ\"}}},\"type\":\"object\"},{\"kind\":\"Input\",\"index\":1,\"value\":{\"Object\":{\"ImmOrOwned\":{\"objectId\":\"0x6e0f3725a853330bbd870f1c9b559f91bacaa24c2a99a6b41af39cd5cb40881f\",\"version\":594927392,\"digest\":\"BTtBV34KjXN2gDaFkaH9sXNNGGWmfkhQmDckCHDYR7NM\"}}},\"type\":\"object\"},{\"kind\":\"Input\",\"index\":2,\"value\":{\"Pure\":[15,94,0,0,0,0,0,0]},\"type\":\"pure\"}],\"transactions\":[{\"kind\":\"MergeCoins\",\"destination\":{\"kind\":\"Input\",\"index\":0,\"value\":{\"Object\":{\"ImmOrOwned\":{\"objectId\":\"0x3f11d40f61d9f20b5488a6d0aa71bcf0a9f0079c4f2d6405c1b72c0c021a79eb\",\"version\":594927392,\"digest\":\"AsbHrWsqkFmH8efJ9CdXVqshG71CAaZtemJKvaaBHWSJ\"}}},\"type\":\"object\"},\"sources\":[{\"kind\":\"Input\",\"index\":1,\"value\":{\"Object\":{\"ImmOrOwned\":{\"objectId\":\"0x6e0f3725a853330bbd870f1c9b559f91bacaa24c2a99a6b41af39cd5cb40881f\",\"version\":594927392,\"digest\":\"BTtBV34KjXN2gDaFkaH9sXNNGGWmfkhQmDckCHDYR7NM\"}}},\"type\":\"object\"}]},{\"kind\":\"SplitCoins\",\"coin\":{\"kind\":\"Input\",\"index\":0,\"value\":{\"Object\":{\"ImmOrOwned\":{\"objectId\":\"0x3f11d40f61d9f20b5488a6d0aa71bcf0a9f0079c4f2d6405c1b72c0c021a79eb\",\"version\":594927392,\"digest\":\"AsbHrWsqkFmH8efJ9CdXVqshG71CAaZtemJKvaaBHWSJ\"}}},\"type\":\"object\"},\"amounts\":[{\"kind\":\"Input\",\"index\":2,\"value\":{\"Pure\":[15,94,0,0,0,0,0,0]},\"type\":\"pure\"}]}]}";

    #[tokio::test]
    async fn test_quote_aftermath_exact_in() {
        let request = GenericEstimateRequest {
            trade_type: TradeType::ExactIn,
            chain_id: ChainId::Sui,
            src_token:
                "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC"
                    .to_string(),
            dest_token:
                "0x0000000000000000000000000000000000000000000000000000000000000002::sui::SUI"
                    .to_string(),
            amount_fixed: 1_000_000, // 1 USDC
            slippage: Slippage::Percent(1.0),
        };

        let (_, routes) = quote_aftermath_swap(request)
            .await
            .expect("Should not fail");

        let routes: AftermathQuoteResponse = serde_json::from_value(routes).unwrap();
        let amount_in: u64 = routes.coin_in.amount.trim_end_matches("n").parse().unwrap();
        assert_eq!(amount_in, 1_000_000);
    }

    #[tokio::test]
    async fn test_quote_aftermath_exact_out() {
        let request = GenericEstimateRequest {
            trade_type: TradeType::ExactOut,
            chain_id: ChainId::Sui,
            src_token:
                "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC"
                    .to_string(),
            dest_token:
                "0x0000000000000000000000000000000000000000000000000000000000000002::sui::SUI"
                    .to_string(),
            amount_fixed: 1_000_000_000, // 1 SUI
            slippage: Slippage::Percent(1.0),
        };
        let (_, routes) = quote_aftermath_swap(request)
            .await
            .expect("Should not fail");

        let routes: AftermathQuoteResponse = serde_json::from_value(routes).unwrap();
        let amount_out: u64 = routes
            .coin_out
            .amount
            .trim_end_matches("n")
            .parse()
            .unwrap();
        assert!(amount_out >= 1_000_000_000);
        assert!(amount_out < 1_020_000_000);
    }

    #[tokio::test]
    async fn test_prepare_swap_ptb_with_aftermath_exact_in() {
        let swap_request = GenericSwapRequest {
            trade_type: TradeType::ExactIn,
            chain_id: ChainId::Sui,
            spender: "0xd422530e3f19bdd09baccfdaf8754ff9b5db01df825a96a581a1236c9b8edf84"
                .to_string(),
            amount_fixed: 10_000, // 0.01 USDC

            src_token:
                "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC"
                    .to_string(),
            dest_token:
                "0x0000000000000000000000000000000000000000000000000000000000000002::sui::SUI"
                    .to_string(),
            slippage: Slippage::Percent(2.0),
            dest_address: "0xd422530e3f19bdd09baccfdaf8754ff9b5db01df825a96a581a1236c9b8edf84"
                .to_string(),
        };

        let quote_request = GenericEstimateRequest::from(swap_request.clone());
        let (_, routes) = quote_aftermath_swap(quote_request)
            .await
            .expect("Should not fail");

        let res = prepare_swap_ptb_with_aftermath(swap_request, routes, None)
            .await
            .expect("Should not fail");

        assert!(res.get("coinOutId").is_none());
    }

    #[tokio::test]
    async fn test_prepare_swap_ptb_with_aftermath_exact_out() {
        let swap_request = GenericSwapRequest {
            trade_type: TradeType::ExactOut,
            chain_id: ChainId::Sui,
            spender: "0xd422530e3f19bdd09baccfdaf8754ff9b5db01df825a96a581a1236c9b8edf84"
                .to_string(),
            amount_fixed: 10_000_000, // 0.01 SUI

            src_token:
                "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC"
                    .to_string(),
            dest_token:
                "0x0000000000000000000000000000000000000000000000000000000000000002::sui::SUI"
                    .to_string(),
            slippage: Slippage::Percent(2.0),
            dest_address: "0xd422530e3f19bdd09baccfdaf8754ff9b5db01df825a96a581a1236c9b8edf84"
                .to_string(),
        };

        let quote_request = GenericEstimateRequest::from(swap_request.clone());
        let (_, routes) = quote_aftermath_swap(quote_request)
            .await
            .expect("Should not fail");

        let res = prepare_swap_ptb_with_aftermath(swap_request, routes, None)
            .await
            .expect("Should not fail");

        assert!(res.get("coinOutId").is_none());
    }

    #[tokio::test]
    async fn test_prepare_swap_ptb_with_aftermath_exact_out_with_destination_address() {
        let swap_request = GenericSwapRequest {
            trade_type: TradeType::ExactOut,
            chain_id: ChainId::Sui,
            spender: "0xd422530e3f19bdd09baccfdaf8754ff9b5db01df825a96a581a1236c9b8edf84"
                .to_string(),
            amount_fixed: 10_000_000, // 0.01 SUI

            src_token:
                "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC"
                    .to_string(),
            dest_token:
                "0x0000000000000000000000000000000000000000000000000000000000000002::sui::SUI"
                    .to_string(),
            slippage: Slippage::Percent(2.0),
            dest_address: "0xd929d817e0ef0338b25254fec67ef6f42a65e248fb2bfaf1d81d1d0aa4d74e67"
                .to_string(),
        };
        let quote_request = GenericEstimateRequest::from(swap_request.clone());
        let (_, routes) = quote_aftermath_swap(quote_request)
            .await
            .expect("Should not fail");

        let serialized_tx = serde_json::from_str(TEST_TX).unwrap();

        let coin_id = json!({
            "$kind": "NestedResult",
            "NestedResult": [1,0]
        });

        let res =
            prepare_swap_ptb_with_aftermath(swap_request, routes, Some((serialized_tx, coin_id)))
                .await
                .expect("Should not fail");

        assert!(res.get("coinOutId").is_some());
    }
    #[tokio::test]
    async fn test_prepare_swap_ptb_with_aftermath_exact_in_with_ptb() {
        let src_token =
            "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC"
                .to_string();
        let amount_in = 10_000; // 0.01 USDC

        let swap_request = GenericSwapRequest {
            trade_type: TradeType::ExactOut,
            chain_id: ChainId::Sui,
            spender: "0xd422530e3f19bdd09baccfdaf8754ff9b5db01df825a96a581a1236c9b8edf84"
                .to_string(),
            amount_fixed: amount_in,

            src_token,
            dest_token:
                "0x0000000000000000000000000000000000000000000000000000000000000002::sui::SUI"
                    .to_string(),
            slippage: Slippage::Percent(2.0),
            dest_address: "0xd929d817e0ef0338b25254fec67ef6f42a65e248fb2bfaf1d81d1d0aa4d74e67"
                .to_string(),
        };

        let quote_request = GenericEstimateRequest::from(swap_request.clone());
        let (_, routes) = quote_aftermath_swap(quote_request)
            .await
            .expect("Should not fail");

        let serialized_tx = serde_json::from_str(TEST_TX).unwrap();

        let coin_id = json!({
            "$kind": "NestedResult",
            "NestedResult": [1,0]
        });

        let res =
            prepare_swap_ptb_with_aftermath(swap_request, routes, Some((serialized_tx, coin_id)))
                .await
                .expect("Should not fail");

        assert!(res.get("coinOutId").is_some());
    }
}
