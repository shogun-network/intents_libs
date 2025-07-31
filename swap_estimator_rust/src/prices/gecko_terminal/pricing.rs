use crate::{
    error::{Error, EstimatorResult},
    prices::{
        PriceProvider, TokenId, TokenPrice,
        defillama::DefiLlamaChain as _,
        gecko_terminal::{
            GECKO_TERMINAL_API_URL,
            responses::{
                GeckoTerminalOkResponseType, GeckoTerminalResponse, GeckoTerminalTokensInfo,
            },
        },
    },
};
use error_stack::{ResultExt as _, report};
use intents_models::{constants::chains::ChainId, network::http::handle_reqwest_response};
use reqwest::Client;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct GeckoTerminalProvider {
    client: Client,
}

impl GeckoTerminalProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl PriceProvider for GeckoTerminalProvider {
    async fn get_tokens_price(
        &self,
        tokens: HashSet<TokenId>,
    ) -> EstimatorResult<HashMap<TokenId, TokenPrice>> {
        todo!();
    }
}

// pub async fn gecko_terminal_get_tokens_prices(
//     client: &Client,
//     chain_id: ChainId,
//     tokens_address: Vec<String>,
// ) -> EstimatorResult<GeckoTerminalTokensPriceResponse> {
//     let url = format!(
//         "{}/simple/networks/{}/token_price/{}",
//         GECKO_TERMINAL_API_URL,
//         chain_id.to_defillama_chain_name(),
//         tokens_address.join(",")
//     );

//     let response = client
//         .get(&url)
//         .send()
//         .await
//         .change_context(Error::ReqwestError)
//         .attach_printable("Error in gecko terminal request")?;

//     let tokens_response: GeckoTerminalTokensPriceResponse = handle_reqwest_response(response)
//         .await
//         .change_context(Error::ModelsError)?;

//     Ok(tokens_response)
// }

pub async fn gecko_terminal_get_tokens_info(
    client: &Client,
    chain_id: ChainId,
    tokens_address: Vec<String>,
) -> EstimatorResult<GeckoTerminalTokensInfo> {
    let url = format!(
        "{}/networks/{}/token_price/{}",
        GECKO_TERMINAL_API_URL,
        chain_id.to_defillama_chain_name(),
        tokens_address.join(",")
    );

    let response = client
        .get(&url)
        .send()
        .await
        .change_context(Error::ReqwestError)
        .attach_printable("Error in gecko terminal request")?;

    let tokens_response: GeckoTerminalResponse = handle_reqwest_response(response)
        .await
        .change_context(Error::ModelsError)?;

    if let GeckoTerminalOkResponseType::TokensInfo(tokens_info) =
        handle_gecko_terminal_response(tokens_response)?
    {
        // TODO: Revisar que parte es un vector
        Ok(tokens_info)
    } else {
        tracing::error!("Unexpected response in gecko terminal request");
        Err(report!(Error::ResponseError)
            .attach_printable("Unexpected response in gecko terminal request"))
    }
}

fn handle_gecko_terminal_response(
    response: GeckoTerminalResponse,
) -> EstimatorResult<GeckoTerminalOkResponseType> {
    match response {
        GeckoTerminalResponse::Ok(gecko_terminal_ok_response) => {
            Ok(gecko_terminal_ok_response.data)
        }
        GeckoTerminalResponse::Error(gecko_terminal_error_response) => {
            tracing::error!(
                "Error in gecko terminal request: {:?}",
                gecko_terminal_error_response.errors
            );
            Err(report!(Error::ResponseError).attach_printable("Error"))
        }
    }
}
