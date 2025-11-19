use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use governor::middleware::NoOpMiddleware;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter, clock::DefaultClock};

use thiserror::Error;

/// Errores posibles del cliente genérico
#[derive(Debug, Error)]
pub enum ApiClientError<E> {
    #[error("Insufficient capacity in rate limiter")]
    InsufficientCapacity,
    #[error("Request queue closed")]
    QueueClosed,
    #[error("Worker task cancelled")]
    WorkerClosed,
    #[error("{0}")]
    Custom(E),
}

/// Defines how many rate-limit "tokens" a request should consume.
pub trait RateLimitedRequest {
    /// Cost in "tokens" of this request.
    /// Default is 1.
    fn cost(&self) -> NonZeroU32 {
        // Safe: 1 is non-zero
        NonZeroU32::new(1).unwrap()
    }
}

/// Generic API request with a responder channel
pub struct ThrottlingApiRequest<Req, Resp, E> {
    pub req: Req,
    pub responder: oneshot::Sender<Result<Resp, ApiClientError<E>>>,
}

impl<Req, Resp, E> ThrottlingApiRequest<Req, Resp, E> {
    pub fn new(req: Req) -> (Self, oneshot::Receiver<Result<Resp, ApiClientError<E>>>) {
        let (responder, receiver) = tokio::sync::oneshot::channel();
        (ThrottlingApiRequest { req, responder }, receiver)
    }
}

pub enum RateLimitWindow {
    PerSecond(NonZeroU32),
    PerMinute(NonZeroU32),
    Custom { period: Duration },
}

impl RateLimitWindow {
    /// - `<n>s` → PerSecond(n)
    /// - `<n>m` → PerMinute(n)
    /// - `<n>h` → Custom { period = Duration::from_secs(n * 3600) }
    /// - `<n>d` → Custom { period = Duration::from_secs(n * 86400) }
    pub fn from_string(s: &str) -> Option<Self> {
        if s.is_empty() {
            return None;
        }

        let (num_str, unit) = s.split_at(s.len() - 1);
        let number: u32 = match num_str.parse() {
            Ok(n) if n > 0 => n,
            _ => return None,
        };
        let nonzero = match NonZeroU32::new(number) {
            Some(nz) => nz,
            None => return None,
        };

        match unit {
            "s" => Some(RateLimitWindow::PerSecond(nonzero)),
            "m" => Some(RateLimitWindow::PerMinute(nonzero)),
            "h" => {
                let secs = number as u64 * 3600;
                Some(RateLimitWindow::Custom {
                    period: Duration::from_secs(secs),
                })
            }
            "d" => {
                let secs = number as u64 * 86400;
                Some(RateLimitWindow::Custom {
                    period: Duration::from_secs(secs),
                })
            }
            _ => None,
        }
    }
}

/// Generic API client with throttling
pub struct ThrottledApiClient<Req, Resp, E> {
    pub sender: mpsc::Sender<ThrottlingApiRequest<Req, Resp, E>>,
    /// Background worker processing queued requests. Kept so that we can await a graceful shutdown and detect panics.
    handle: JoinHandle<()>,
}

impl<Req, Resp, E> ThrottledApiClient<Req, Resp, E>
where
    Req: RateLimitedRequest + Send + 'static,
    Resp: Send + 'static,
    E: Send + 'static,
{
    /// Creates a new throttled API client.
    ///
    /// This client enqueues incoming requests in an internal bounded queue and processes
    /// them in a background task, respecting a global rate limit defined by `limit_per_sec`
    /// and `burst`.
    ///
    /// - `limit_per_sec`: maximum sustained number of requests allowed per second.
    /// - `burst`: maximum number of requests that can be executed in a short burst
    ///   before the rate limiter starts delaying new requests.
    /// - `queue_capacity`: maximum number of pending requests that can be buffered
    ///   in the internal MPSC channel. Once this capacity is reached, calls to
    ///   [`ThrottledApiClient::send`] will wait until there is free space.
    /// - `handler_fn`: asynchronous function that processes each `Req` and returns
    ///   either a successful `Resp` or a custom error `E`.
    ///
    /// Each enqueued request is:
    /// 1. Throttled by the shared rate limiter.
    /// 2. Processed in its own Tokio task using `handler_fn`.
    pub fn new<F, Fut>(
        limit: RateLimitWindow,
        burst: NonZeroU32,
        queue_capacity: usize,
        handler_fn: F,
    ) -> Self
    where
        F: Fn(Req) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<Resp, E>> + Send + 'static,
    {
        // Build the rate limiter
        let quota = match limit {
            RateLimitWindow::PerSecond(allowed) => Quota::per_second(allowed).allow_burst(burst),
            RateLimitWindow::PerMinute(allowed) => Quota::per_minute(allowed).allow_burst(burst),
            RateLimitWindow::Custom { period } => {
                Quota::with_period(period).unwrap().allow_burst(burst)
            }
        };
        let limiter = Arc::new(RateLimiter::<
            NotKeyed,
            InMemoryState,
            DefaultClock,
            NoOpMiddleware,
        >::direct(quota));

        let (tx, mut rx) = mpsc::channel::<ThrottlingApiRequest<Req, Resp, E>>(queue_capacity);

        let limiter_clone = Arc::clone(&limiter);
        let handler_fn = Arc::new(handler_fn);

        let handle = tokio::spawn(async move {
            while let Some(api_req) = rx.recv().await {
                // Wait for rate-limit permit
                if let Err(_) = limiter_clone.until_n_ready(api_req.req.cost()).await {
                    let _ = api_req
                        .responder
                        .send(Err(ApiClientError::InsufficientCapacity));
                    continue;
                };

                let handler_fn = Arc::clone(&handler_fn);
                let req = api_req.req;
                let responder = api_req.responder;

                // Execute the concrete request
                tokio::spawn(async move {
                    let result = handler_fn(req).await.map_err(|e| ApiClientError::Custom(e));
                    let _ = responder.send(result);
                });
            }
        });

        ThrottledApiClient { sender: tx, handle }
    }

    pub async fn send(&self, req: Req) -> Result<Resp, ApiClientError<E>> {
        let (resp_tx, resp_rx) = oneshot::channel();
        let api_req = ThrottlingApiRequest {
            req,
            responder: resp_tx,
        };
        self.sender
            .send(api_req)
            .await
            .map_err(|_| ApiClientError::QueueClosed)?;
        resp_rx.await.map_err(|_| ApiClientError::WorkerClosed)?
    }

    pub async fn shutdown(self) -> Result<(), tokio::task::JoinError> {
        drop(self.sender);
        self.handle.await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    // Simple handler that just echoes the request as response
    async fn echo_handler(req: u32) -> Result<u32, ()> {
        Ok(req)
    }
    impl RateLimitedRequest for u32 {
        // Opcional: puedes sobreescribir `cost` si quieres un coste distinto
        // fn cost(&self) -> NonZeroU32 { NonZeroU32::new(1).unwrap() }
    }

    #[tokio::test]
    async fn test_basic_request_success() {
        let client = ThrottledApiClient::new(
            RateLimitWindow::PerSecond(NonZeroU32::new(10).unwrap()), // 10 req/s
            NonZeroU32::new(10).unwrap(),                             // burst 10
            10,                                                       // queue capacity
            echo_handler,
        );

        let result = client.send(42).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);

        client.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_rate_limit_is_respected() {
        // 2 req/s, burst 1 ⇒ second request has to wait ~0.5s at least
        let client = ThrottledApiClient::new(
            RateLimitWindow::PerSecond(NonZeroU32::new(2).unwrap()),
            NonZeroU32::new(1).unwrap(),
            10,
            echo_handler,
        );

        let start = Instant::now();
        let client = Arc::new(client);

        let h1 = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.send(1).await }
        });
        let h2 = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.send(2).await }
        });

        let r1 = h1.await.unwrap();
        let r2 = h2.await.unwrap();

        assert!(r1.is_ok());
        assert!(r2.is_ok());

        // With 2 req/s, sending 2 concurrent requests should take
        // at least ~500ms; we use a wide margin to avoid flakes.
        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_millis(400),
            "Expected at least ~400ms, got {elapsed:?}"
        );
    }
}
