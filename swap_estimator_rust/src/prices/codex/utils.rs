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
                CodexGetMetadataData, CodexGetPricesAndMetaData, CodexGetPricesData,
                CodexGraphqlResponse, CodexMetadataPayload, CodexPricePayload,
            },
        },
    },
};

const PRICE_AND_META_MAX_TOKENS: usize = 100;
const PRICE_OR_METADATA_ONLY_MAX_TOKENS: usize = 200;
const BATCH_SIZE: usize = 25;

fn validate_token_len(kind: &str, len: usize, max: usize) -> EstimatorResult<usize> {
    if len > max {
        return Err(report!(Error::CodexError(format!(
            "Can't request more than {max} tokens for {kind} (requested {len})"
        ))));
    }
    Ok(len.div_ceil(BATCH_SIZE))
}

fn build_inputs_object_generic(
    inputs: &[Value],
    max_tokens: usize,
    key_prefixes: &[&str],
    kind: &str,
) -> EstimatorResult<Value> {
    validate_token_len(kind, inputs.len(), max_tokens)?;
    let mut result: HashMap<String, Value> = HashMap::new();

    for (i, chunk) in inputs.chunks(BATCH_SIZE).enumerate() {
        let mut inputs = Vec::with_capacity(chunk.len());
        for input in chunk {
            inputs.push(input.to_owned());
        }

        for prefix in key_prefixes {
            let key = format!("{prefix}{i}");
            result.insert(key, Value::Array(inputs.clone()));
        }
    }

    let mut object_map: Map<String, Value> = Map::with_capacity(result.len());
    for (k, v) in result {
        object_map.insert(k, v);
    }

    Ok(Value::Object(object_map))
}

fn build_inputs_object_by_token_id(
    tokens: &[TokenId],
    max_tokens: usize,
    key_prefixes: &[&str],
    kind: &str,
) -> EstimatorResult<Value> {
    validate_token_len(kind, tokens.len(), max_tokens)?;
    let mut result: HashMap<String, Value> = HashMap::new();

    for (i, chunk) in tokens.chunks(BATCH_SIZE).enumerate() {
        let mut inputs = Vec::with_capacity(chunk.len());
        for token in chunk {
            let network = token.chain.to_codex_chain_number();
            inputs.push(serde_json::json!({
                "address": token.address,
                "networkId": network
            }));
        }

        for prefix in key_prefixes {
            let key = format!("{prefix}{i}");
            result.insert(key, Value::Array(inputs.clone()));
        }
    }

    let mut object_map: Map<String, Value> = Map::with_capacity(result.len());
    for (k, v) in result {
        object_map.insert(k, v);
    }

    Ok(Value::Object(object_map))
}

fn build_batched_body<F>(
    tokens_len: usize,
    max_tokens: usize,
    kind: &str,
    mut build_chunk: F,
) -> EstimatorResult<String>
where
    F: FnMut(usize) -> String,
{
    let chunks = validate_token_len(kind, tokens_len, max_tokens)?;
    let mut body = Vec::with_capacity(chunks);
    for i in 0..chunks {
        body.push(build_chunk(i));
    }
    Ok(body.join("\n"))
}

pub fn subscription_id(token: &TokenId) -> String {
    format!(
        "{}:{}",
        token.chain.to_codex_chain_number(),
        token.address.to_lowercase()
    )
}

pub fn default_decimals(chain: ChainId) -> u8 {
    match chain {
        ChainId::Solana | ChainId::Sui => 9,
        _ => 18,
    }
}

fn create_price_and_metadata_args(tokens_len: usize) -> EstimatorResult<String> {
    let args_needed = validate_token_len("price+metadata", tokens_len, PRICE_AND_META_MAX_TOKENS)?;

    let mut args = Vec::with_capacity(args_needed);
    for i in 0..args_needed {
        args.push(format!("$tokenInputs{i}: [TokenInput!]"));
        args.push(format!("$priceInputs{i}: [GetPriceInput!]"));
    }
    Ok(args.join("\n"))
}

fn create_price_and_metadata_body(tokens_len: usize) -> EstimatorResult<String> {
    build_batched_body(
        tokens_len,
        PRICE_AND_META_MAX_TOKENS,
        "price+metadata",
        |i| {
            format!(
                r#"meta{i}: tokens(ids: $tokenInputs{i}) {{
        address
        networkId
        decimals
        name
        symbol
    }}
    prices{i}: getTokenPrices(inputs: $priceInputs{i}) {{
        address
        networkId
        priceUsd
        timestamp
    }}"#
            )
        },
    )
}

fn create_price_and_metadata_inputs(tokens: &[TokenId]) -> EstimatorResult<Value> {
    build_inputs_object_by_token_id(
        tokens,
        PRICE_AND_META_MAX_TOKENS,
        &["tokenInputs", "priceInputs"],
        "price+metadata",
    )
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

    let results_needed = tokens_len.div_ceil(BATCH_SIZE);
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

    if prices.len() != tokens_len || meta.len() != tokens_len {
        return Err(report!(Error::CodexError(format!(
            "Expected {} tokens but received {} prices and {} metadata entries",
            tokens_len,
            prices.len(),
            meta.len()
        ))));
    }

    Ok(CodexGetPricesAndMetaData { prices, meta })
}

fn create_get_prices_args(tokens_len: usize) -> EstimatorResult<String> {
    let args_needed =
        validate_token_len("price-only", tokens_len, PRICE_OR_METADATA_ONLY_MAX_TOKENS)?;
    let mut args = Vec::with_capacity(args_needed);
    for i in 0..args_needed {
        args.push(format!("$inputs{i}: [GetPriceInput!]"));
    }
    Ok(args.join("\n"))
}

fn create_get_prices_body(tokens_len: usize) -> EstimatorResult<String> {
    build_batched_body(
        tokens_len,
        PRICE_OR_METADATA_ONLY_MAX_TOKENS,
        "price-only",
        |i| {
            format!(
                r#"prices{i}: getTokenPrices(inputs: $inputs{i}) {{
        address
        networkId
        priceUsd
        timestamp
        poolAddress
        confidence
    }}"#
            )
        },
    )
}

fn create_get_prices_inputs(tokens: &[TokenId]) -> EstimatorResult<Value> {
    build_inputs_object_by_token_id(
        tokens,
        PRICE_OR_METADATA_ONLY_MAX_TOKENS,
        &["inputs"],
        "price-only",
    )
}

fn create_get_historical_prices_inputs(
    tokens_and_dates: &[(TokenId, u64)],
) -> EstimatorResult<Value> {
    let inputs: Vec<Value> = tokens_and_dates
        .iter()
        .map(|(token, date)| {
            serde_json::json!({
                "address": token.address,
                "networkId": token.chain.to_codex_chain_number(),
                "timestamp": date,
            })
        })
        .collect();
    build_inputs_object_generic(
        &inputs,
        PRICE_OR_METADATA_ONLY_MAX_TOKENS,
        &["inputs"],
        "historical-price-only",
    )
}

pub fn combine_get_prices_query(tokens: &[TokenId]) -> EstimatorResult<Value> {
    let args = create_get_prices_args(tokens.len())?;
    let body = create_get_prices_body(tokens.len())?;
    let inputs = create_get_prices_inputs(tokens)?;
    let query = format!(
        r#"query GetTokenPrice(
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

pub fn combine_get_historical_prices_query(tokens: &[(TokenId, u64)]) -> EstimatorResult<Value> {
    let args = create_get_prices_args(tokens.len())?;
    let body = create_get_prices_body(tokens.len())?;
    let inputs = create_get_historical_prices_inputs(tokens)?;
    let query = format!(
        r#"query GetTokenPrice(
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

pub fn assemble_get_prices_results(
    tokens_len: usize,
    result: Value,
) -> EstimatorResult<CodexGetPricesData> {
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

    let results_needed = tokens_len.div_ceil(BATCH_SIZE);
    for i in 0..results_needed {
        let prices_key = format!("prices{i}");
        if let Some(prices_value) = data.get(&prices_key) {
            let mut parsed_prices: Vec<Option<CodexPricePayload>> = serde_json::from_value(
                prices_value.clone(),
            )
            .change_context(Error::SerdeDeserialize(
                "Failed to deserialize Codex HTTP price GraphQL prices response".to_string(),
            ))?;
            prices.extend(parsed_prices.drain(..));
        }
    }

    Ok(CodexGetPricesData { prices })
}

fn create_get_metadata_args(tokens_len: usize) -> EstimatorResult<String> {
    let args_needed = validate_token_len(
        "metadata-only",
        tokens_len,
        PRICE_OR_METADATA_ONLY_MAX_TOKENS,
    )?;
    let mut args = Vec::with_capacity(args_needed);
    for i in 0..args_needed {
        args.push(format!("$inputs{i}: [TokenInput!]"));
    }
    Ok(args.join("\n"))
}

fn create_get_metadata_body(tokens_len: usize) -> EstimatorResult<String> {
    build_batched_body(
        tokens_len,
        PRICE_OR_METADATA_ONLY_MAX_TOKENS,
        "metadata-only",
        |i| {
            format!(
                r#"meta{i}: tokens(ids: $inputs{i}) {{
        address
        networkId
        name
        symbol
        decimals
    }}"#
            )
        },
    )
}

fn create_get_metadata_inputs(tokens: &[TokenId]) -> EstimatorResult<Value> {
    build_inputs_object_by_token_id(
        tokens,
        PRICE_OR_METADATA_ONLY_MAX_TOKENS,
        &["inputs"],
        "metadata-only",
    )
}

pub fn combine_get_metadata_query(tokens: &[TokenId]) -> EstimatorResult<Value> {
    let args = create_get_metadata_args(tokens.len())?;
    let body = create_get_metadata_body(tokens.len())?;
    let inputs = create_get_metadata_inputs(tokens)?;
    let query = format!(
        r#"query GetTokenMetadata(
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

pub fn assemble_get_metadata_results(
    tokens_len: usize,
    result: Value,
) -> EstimatorResult<CodexGetMetadataData> {
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

    let mut meta: Vec<Option<CodexMetadataPayload>> = Vec::with_capacity(tokens_len);

    let results_needed = tokens_len.div_ceil(BATCH_SIZE);
    for i in 0..results_needed {
        let meta_key = format!("meta{i}");
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

    Ok(CodexGetMetadataData { meta })
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
