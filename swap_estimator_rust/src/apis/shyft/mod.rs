use error_stack::{ResultExt as _, report};
use intents_models::network::http::handle_reqwest_response;
use serde_json::json;

use crate::{
    apis::shyft::responses::{PumpPoolData, ShyftResponse, ShyftResponseData},
    error::{Error, EstimatorResult},
};

pub mod responses;

pub async fn get_pump_fun_pools_by_liquidity_pair(
    api_key: &str,
    mint_a: &str,
    mint_b: &str,
) -> EstimatorResult<Vec<PumpPoolData>> {
    let query = r#"
        query MyQuery($mints: [String!]) {
          pump_fun_amm_Pool(
            where: {
              base_mint:  { _in: $mints }
              quote_mint: { _in: $mints }
            }
          ) {
            base_mint
            creator
            index
            lp_mint
            lp_supply
            pool_base_token_account
            pool_bump
            pool_quote_token_account
            quote_mint
            pubkey
          }
        }
    "#;

    let body = json!({
        "query": query,
        "operationName": "MyQuery",
        "variables": { "mints": [mint_a, mint_b] }
    });

    let response = reqwest::Client::new()
        .post(format!(
            "https://programs.shyft.to/v0/graphql/accounts?api_key={api_key}&network=mainnet-beta",
        ))
        .json(&body)
        .send()
        .await
        .change_context(Error::ReqwestError)
        .attach_printable("Failed to fetch pump fun pools")?;

    let data: ShyftResponse = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;

    let response = handle_shyft_response(data)?;

    if let ShyftResponseData::PumpPoolData { pump_fun_amm_Pool } = response {
        Ok(pump_fun_amm_Pool)
    } else {
        Err(report!(Error::ResponseError)
            .attach_printable("Unexpected response type from Shyft API"))
    }
}

fn handle_shyft_response(response: ShyftResponse) -> EstimatorResult<ShyftResponseData> {
    match response {
        ShyftResponse::Error { error } => Err(report!(Error::ResponseError)
            .attach_printable(format!("Error from Shyft API: {}", error))),
        ShyftResponse::Unknown(val) => {
            tracing::error!(
                "Unknown response from Shyft API: {}",
                serde_json::to_string_pretty(&val)
                    .ok()
                    .unwrap_or("Failed to serialize response".to_string())
            );
            // println!(
            //     "Unknown response from Liquidswap: {}",
            //     serde_json::to_string_pretty(&val).unwrap()
            // );
            Err(report!(Error::ResponseError).attach_printable("Unknown response from Shyft API"))
        }
        ShyftResponse::Data { data } => Ok(data),
    }
}

#[cfg(test)]
mod tests {
    use intents_models::constants::chains::WRAPPED_NATIVE_TOKEN_SOLANA_ADDRESS;

    use super::*;

    #[tokio::test]
    async fn test_get_pump_fun_pools_by_liquidity_pair() {
        let api_key = std::env::var("SHYFT_API_KEY").expect("SHYFT_API_KEY must be set");
        let base_mint = "Si8Y3nfRcHLGpjWdJw5bpgmBvzKGLRovjBijGGcpump";
        let quote_mint = WRAPPED_NATIVE_TOKEN_SOLANA_ADDRESS;

        let result = get_pump_fun_pools_by_liquidity_pair(&api_key, base_mint, quote_mint).await;
        println!("Result: {:#?}", result);
        assert!(result.is_ok());
        let pools = result.unwrap();
        println!("Pools: {:#?}", pools);
    }
}
