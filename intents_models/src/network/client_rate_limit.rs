use governor::middleware::NoOpMiddleware;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter, clock::DefaultClock};
use reqwest::{Client as ReqwestClient, Error as ReqwestError, Request, Response};
use std::num::NonZeroU32;
use std::sync::Arc;

use crate::network::RateLimitWindow;

#[derive(Debug, Clone)]
pub enum Client {
    RateLimited(RateLimitedClient),
    Unrestricted(ReqwestClient),
}

#[derive(Debug, Clone)]
pub struct RateLimitedClient {
    inner: ReqwestClient,
    limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock, NoOpMiddleware>>,
}

impl RateLimitedClient {
    pub fn new(limit: RateLimitWindow, burst: Option<NonZeroU32>) -> Self {
        let quota = {
            let mut quota = match limit {
                RateLimitWindow::PerSecond(allowed) => Quota::per_second(allowed),
                RateLimitWindow::PerMinute(allowed) => Quota::per_minute(allowed),
                RateLimitWindow::Custom { period } => Quota::with_period(period).unwrap(),
            };
            match burst {
                Some(b) => quota = quota.allow_burst(b),
                None => {}
            }
            quota
        };
        let limiter = Arc::new(RateLimiter::direct(quota));
        let inner = ReqwestClient::new();
        Self { inner, limiter }
    }

    /// Devuelve una referencia al cliente reqwest para funciones que esperan `&reqwest::Client`.
    pub fn inner_client(&self) -> &ReqwestClient {
        &self.inner
    }

    pub async fn execute(&self, req: Request) -> Result<Response, ReqwestError> {
        self.limiter.until_ready().await;
        self.inner.execute(req).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub async fn call_time(client: &Client) {
        let req: Request = Request::new(
            reqwest::Method::GET,
            "https://aisenseapi.com/services/v1/timestamp"
                .parse()
                .unwrap(),
        );
        let response = match client {
            Client::RateLimited(rate_limited_client) => {
                let resp = rate_limited_client.execute(req).await.unwrap();
                resp
            }
            Client::Unrestricted(unrestricted_client) => {
                let resp = unrestricted_client.execute(req).await.unwrap();
                resp
            }
        };
        let body = response.text().await.unwrap();
        println!("Response Body: {}", body);
    }

    #[tokio::test]
    async fn test_rate_limited_client() {
        let rate_limited_client = RateLimitedClient::new(
            RateLimitWindow::PerSecond(NonZeroU32::new(5).unwrap()),
            None,
        );
        let client = Client::RateLimited(rate_limited_client);

        for _ in 0..20 {
            call_time(&client).await;
        }
    }
}
