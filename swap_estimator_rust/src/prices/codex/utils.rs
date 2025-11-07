use std::collections::HashMap;

use error_stack::report;
use serde_json::{Map, Value};

use crate::{
    error::{Error, EstimatorResult},
    prices::TokenId,
};

const BATCH_SIZE: usize = 25;

fn create_price_and_metadata_args(tokens_len: usize) -> EstimatorResult<String> {
    if tokens_len > 100 {
        return Err(report!(Error::CodexError(
            "Can't request more than 200 results".to_string()
        )));
    }
    let mut args = Vec::new();
    let args_needed = tokens_len.div_ceil(25);
    for i in 0..args_needed {
        args.push(format!("$tokenInputs{i}: [TokenInput!]"));
        args.push(format!("$priceInputs{i}: [GetPriceInput!]"));
    }
    Ok(args.join("\n"))
}

fn create_price_and_metadata_body(tokens_len: usize) -> EstimatorResult<String> {
    if tokens_len > 100 {
        return Err(report!(Error::CodexError(
            "Can't request more than 200 results".to_string()
        )));
    }
    let mut body = Vec::new();
    let args_needed = tokens_len.div_ceil(25);
    for i in 0..args_needed {
        body.push(format!(
            r#"meta{i}: tokens(ids: $tokenInputs{i}) {{
        address
        networkId
        decimals
        name
        symbol
        }}"#
        ));
        body.push(format!(
            r#"prices{i}: getTokenPrices(inputs: $priceInputs{i}) {{
        address
        networkId
        priceUsd
        timestamp
        }}"#
        ));
    }
    Ok(body.join("\n"))
}

fn create_price_and_metadata_inputs(tokens: &[TokenId]) -> EstimatorResult<Value> {
    if tokens.len() > 100 {
        return Err(report!(Error::CodexError(
            "Can't request more than 200 results".to_string()
        )));
    }
    let mut result = HashMap::new();
    for (i, chunk) in tokens.chunks(BATCH_SIZE).enumerate() {
        let mut inputs = Vec::new();
        for token in chunk {
            inputs.push(serde_json::json!({
                "address": token.address,
                "networkId": token.chain
            }));
        }
        let token_inputs_key = format!("tokenInputs{i}");
        let price_inputs_key = format!("priceInputs{i}");
        result.insert(token_inputs_key, Value::Array(inputs.clone()));
        result.insert(price_inputs_key, Value::Array(inputs));
    }

    let mut object_map: Map<String, Value> = Map::with_capacity(result.len());
    for (k, v) in result {
        object_map.insert(k, v);
    }
    Ok(Value::Object(object_map))
}

pub fn combine_price_and_metadata_query(tokens: &[TokenId]) -> EstimatorResult<Value> {
    let args = create_price_and_metadata_args(tokens.len())?;
    let body = create_price_and_metadata_body(tokens.len())?;
    let inputs = create_price_and_metadata_inputs(tokens)?;
    let query = format!(
        r#"query TokensWithPrices(
    {args}
) {{
    {body}
}}
        "#
    );
    Ok(serde_json::json!({
        "query": query,
        "variables": inputs
    }))
}

#[cfg(test)]
mod tests {
    use intents_models::constants::chains::ChainId;

    use super::*;

    #[test]
    fn test_create_price_and_metadata_args() {
        let args = create_price_and_metadata_args(51).unwrap();
        println!("Args:\n {}", args);
    }

    #[test]
    fn test_create_price_and_metadata_body() {
        let body = create_price_and_metadata_body(51).unwrap();
        println!("Body:\n {}", body);
    }

    #[test]
    fn test_create_price_and_metadata_inputs() {
        let tokens = vec![
            TokenId {
                address: "0xTokenAddress1".to_string(),
                chain: ChainId::Ethereum,
            },
            TokenId {
                address: "0xTokenAddress2".to_string(),
                chain: ChainId::Bsc,
            },
            TokenId {
                address: "0xTokenAddress3".to_string(),
                chain: ChainId::ArbitrumOne,
            },
        ];
        let inputs = create_price_and_metadata_inputs(&tokens).unwrap();
        println!("Inputs:\n {}", inputs);
    }

    #[test]
    fn test_combine_price_and_metadata_query() {
        let tokens = vec![
            TokenId {
                address: "0xTokenAddress1".to_string(),
                chain: ChainId::Ethereum,
            },
            TokenId {
                address: "0xTokenAddress2".to_string(),
                chain: ChainId::Bsc,
            },
            TokenId {
                address: "0xTokenAddress3".to_string(),
                chain: ChainId::ArbitrumOne,
            },
        ];
        let result = combine_price_and_metadata_query(&tokens).unwrap();
        println!("Result:\n {:#?}", result);
    }
}
