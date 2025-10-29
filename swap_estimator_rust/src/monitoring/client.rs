use std::collections::{HashMap, HashSet};

use error_stack::{ResultExt, report};
use intents_models::constants::chains::ChainId;
use tokio::sync::{mpsc::Sender, oneshot};

use crate::{
    error::{Error, EstimatorResult},
    monitoring::messages::MonitorRequest,
    prices::{TokenId, TokenPrice, estimating::OrderEstimationData},
};

#[derive(Debug, Clone)]
pub struct MonitorClient {
    client: Sender<MonitorRequest>,
}

impl MonitorClient {
    pub fn new(client: Sender<MonitorRequest>) -> Self {
        Self { client }
    }

    pub async fn get_coins_data(
        &self,
        token_ids: HashSet<TokenId>,
    ) -> EstimatorResult<HashMap<TokenId, TokenPrice>> {
        let (resp_sender, resp_receiver) = oneshot::channel();
        self.client
            .send(MonitorRequest::GetCoinsData {
                token_ids,
                resp: resp_sender,
            })
            .await
            .change_context(Error::ResponseError)
            .attach_printable("Failed to send result of get coins data")?;
        match resp_receiver.await {
            Ok(Ok(data)) => Ok(data),
            Ok(Err(e)) => {
                tracing::error!("Error in monitoring service response: {e}");
                Err(e.clone())
                    .change_context(Error::ResponseError)
                    .attach_printable_lazy(|| format!("Failed to get coins data: {e}"))
            }
            Err(_) => {
                tracing::error!("Failed to receive response from monitoring service");
                Err(report!(Error::ResponseError)
                    .attach_printable("Failed to receive response from monitoring service"))
            }
        }
    }

    pub async fn check_swap_feasibility(
        &self,
        order_id: String,
        src_chain: ChainId,
        dst_chain: ChainId,
        token_in: String,
        token_out: String,
        amount_in: u128,
        amount_out: u128,
        feasibility_margin_in: f64,
        feasibility_margin_out: f64,
        extra_expenses: HashMap<TokenId, u128>,
    ) -> EstimatorResult<()> {
        self.client
            .send(MonitorRequest::CheckSwapFeasibility {
                order_id,
                src_chain,
                dst_chain,
                token_in,
                token_out,
                amount_in,
                amount_out,
                feasibility_margin_in,
                feasibility_margin_out,
                extra_expenses,
            })
            .await
            .change_context(Error::ResponseError)
            .attach_printable("Failed to send result of check swap feasibility")
    }

    pub async fn remove_check_swap_feasibility(&self, order_id: String) -> EstimatorResult<()> {
        self.client
            .send(MonitorRequest::RemoveCheckSwapFeasibility { order_id })
            .await
            .change_context(Error::ResponseError)
            .attach_printable("Failed to send result of remove check swap feasibility")
    }

    pub async fn estimate_orders_amount_out(
        &self,
        orders: Vec<OrderEstimationData>,
    ) -> EstimatorResult<HashMap<String, u128>> {
        let (resp_sender, resp_receiver) = oneshot::channel();
        self.client
            .send(MonitorRequest::EstimateOrdersAmountOut {
                orders,
                resp: resp_sender,
            })
            .await
            .change_context(Error::ResponseError)
            .attach_printable("Failed to send result of estimate orders amount out")?;
        match resp_receiver.await {
            Ok(Ok(data)) => Ok(data),
            Ok(Err(e)) => {
                tracing::error!("Error in monitoring service response: {e}");
                Err(e.clone())
                    .change_context(Error::ResponseError)
                    .attach_printable_lazy(|| format!("Failed to estimate orders amount out: {e}"))
            }
            Err(_) => {
                tracing::error!("Failed to receive response from monitoring service");
                Err(report!(Error::ResponseError)
                    .attach_printable("Failed to receive response from monitoring service"))
            }
        }
    }
}
