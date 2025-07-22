use std::collections::{HashMap, HashSet};

use intents_models::constants::chains::ChainId;

use crate::{
    error::EstimatorResult,
    prices::defillama::pricing::{DefiLlamaChain as _, DefiLlamaTokensResponse, get_tokens_data},
    utils::number_conversion::{f64_to_u128, u128_to_f64},
};

#[derive(Debug, Clone)]
pub struct OrderEstimationData {
    pub order_id: String,
    pub src_chain: ChainId,
    pub dst_chain: ChainId,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: u128,
}

pub fn estimate_order_amount_out(
    order_data: &OrderEstimationData,
    tokens_info: &DefiLlamaTokensResponse,
) -> EstimatorResult<Option<u128>> {
    let src_token_data = tokens_info.coins.get(
        &order_data
            .src_chain
            .to_defillama_format(&order_data.token_in),
    );
    let dst_token_data = tokens_info.coins.get(
        &order_data
            .dst_chain
            .to_defillama_format(&order_data.token_out),
    );

    if let (Some(src_data), Some(dst_data)) = (src_token_data, dst_token_data) {
        let src_price = src_data.price;
        let dst_price = dst_data.price;

        let amount_in_decimal = u128_to_f64(order_data.amount_in, src_data.decimals);
        let amount_out_decimal = amount_in_decimal * (src_price / dst_price);
        let amount_out = f64_to_u128(amount_out_decimal, dst_data.decimals)?;
        Ok(Some(amount_out))
    } else {
        Ok(None)
    }
}

pub async fn estimate_orders_amount_out(
    orders: Vec<OrderEstimationData>,
) -> EstimatorResult<HashMap<String, u128>> {
    let mut result = HashMap::new();

    // Get all tokens info in one request
    let tokens_to_request = orders
        .iter()
        .flat_map(|order| {
            vec![
                (order.src_chain, order.token_in.clone()),
                (order.dst_chain, order.token_out.clone()),
            ]
        })
        .collect::<HashSet<_>>();

    let tokens_info = get_tokens_data(tokens_to_request).await?;

    for order in orders {
        match estimate_order_amount_out(&order, &tokens_info) {
            Ok(Some(amount_out)) => {
                result.insert(order.order_id, amount_out);
            }
            Ok(None) => {
                tracing::warn!(
                    "Token data not found for order {}: src_chain: {}, src_token: {}, dst_chain: {}, dst_token: {}",
                    order.order_id,
                    order.src_chain,
                    order.token_in,
                    order.dst_chain,
                    order.token_out
                );
            }
            Err(e) => {
                tracing::error!(
                    "Error estimating amount out for order {}: {}",
                    order.order_id,
                    e
                );
            }
        }
    }

    Ok(result)
}
