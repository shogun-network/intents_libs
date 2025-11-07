use std::collections::HashMap;

use error_stack::{ResultExt as _, report};
use intents_models::constants::chains::ChainId;
use serde_json::{Map, Value};

use crate::{
    error::{Error, EstimatorResult},
    prices::{
        TokenId,
        codex::{
            CodexChain as _,
            models::{
                CodexGetPricesAndMetaData, CodexGraphqlResponse, CodexMetadataPayload,
                CodexPricePayload,
            },
        },
    },
};

pub fn subscription_id(token: &TokenId) -> String {
    format!(
        "{}:{}",
        token.chain.to_codex_chain_number(),
        token.address.to_lowercase()
    )
}

pub fn default_decimals(token: &TokenId) -> u8 {
    match token.chain {
        ChainId::Solana | ChainId::Sui => 9,
        _ => 18,
    }
}

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
            let network = token.chain.to_codex_chain_number();
            inputs.push(serde_json::json!({
                "address": token.address,
                "networkId": network
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

pub fn assemble_price_and_metadata_results(
    tokens_len: usize,
    result: Value,
) -> EstimatorResult<CodexGetPricesAndMetaData> {
    if tokens_len > 100 {
        return Err(report!(Error::CodexError(
            "Can't request more than 200 results".to_string()
        )));
    }
    let payload = serde_json::from_value::<CodexGraphqlResponse<Value>>(result).change_context(
        Error::SerdeDeserialize(
            "Failed to deserialize Codex HTTP price GraphQL response".to_string(),
        ),
    )?;

    if let Some(errors) = payload.errors.as_ref() {
        if !errors.is_empty() {
            tracing::warn!(
                "Codex HTTP price batch response contained errors: {:?}",
                errors
            );
        }
    }

    let Some(data) = payload.data else {
        return Err(report!(Error::ResponseError)
            .attach_printable(format!("No data found in Codex HTTP price response")));
    };

    let mut prices: Vec<Option<CodexPricePayload>> = Vec::with_capacity(tokens_len);
    let mut meta: Vec<Option<CodexMetadataPayload>> = Vec::with_capacity(tokens_len);

    let results_needed = tokens_len.div_ceil(25);
    for i in 0..results_needed {
        let prices_key = format!("prices{i}");
        let meta_key = format!("meta{i}");
        if let Some(prices_value) = data.get(&prices_key) {
            let mut parsed_prices: Vec<Option<CodexPricePayload>> = serde_json::from_value(
                prices_value.clone(),
            )
            .change_context(Error::SerdeDeserialize(
                "Failed to deserialize Codex HTTP price GraphQL prices response".to_string(),
            ))?;
            prices.extend(parsed_prices.drain(..));
        }
        if let Some(meta_value) = data.get(&meta_key) {
            let mut parsed_meta: Vec<Option<CodexMetadataPayload>> = serde_json::from_value(
                meta_value.clone(),
            )
            .change_context(Error::SerdeDeserialize(
                "Failed to deserialize Codex HTTP price GraphQL metadata response".to_string(),
            ))?;
            meta.extend(parsed_meta.drain(..));
        }
    }

    Ok(CodexGetPricesAndMetaData { prices, meta })
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
