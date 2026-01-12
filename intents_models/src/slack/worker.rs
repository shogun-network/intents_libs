//! Worker implementation for asynchronous Slack messaging.
//!
//! This module provides a worker that processes Slack message requests
//! asynchronously through a channel, implementing rate limiting to comply
//! with Slack API restrictions.

use std::num::NonZeroU32;
use std::time::Instant;

use crate::error::Error;
use crate::network::RateLimitWindow;
use crate::network::client_rate_limit::{Client, RateLimitedClient};
use crate::slack::api;
use tokio::sync::mpsc::Receiver;
use tokio::time::{Duration, sleep};

/// A worker that processes Slack message requests asynchronously.
///
/// `SlackWorker` receives message requests through a channel and sends them
/// to the Slack API while respecting rate limits. It's designed to run in
/// a separate task to avoid blocking the main application flow.
///
/// # Rate Limiting
///
/// The worker enforces a rate limit of approximately one message per second
/// (with a small buffer) to comply with Slack's API requirements.
#[derive(Debug)]
pub struct SlackWorker {
    client: Client,
    token: String,
    channel: String,
    receiver: Receiver<String>,
    /// Earliest instant at which we are allowed to send a message
    next_allowed_at: Instant,
    /// Base throttle for unknown retry (Slack ≈ 1 msg / sec / channel)
    base_throttle: Duration,
}

impl SlackWorker {
    pub fn new(token: String, channel: String, receiver: Receiver<String>) -> Self {
        Self {
            client: Client::RateLimited(RateLimitedClient::new(
                // 1 msg per second with burst of 3
                RateLimitWindow::PerSecond(NonZeroU32::new(1).expect("NonZeroU32::new(1) failed")), // Safe unwrap
                Some(NonZeroU32::new(3).expect("NonZeroU32::new(3) failed")), // Safe unwrap
            )),
            token,
            channel,
            receiver,
            next_allowed_at: Instant::now(),
            base_throttle: Duration::from_secs(1),
        }
    }

    /// Starts the worker processing loop.
    ///
    /// This method enters an asynchronous loop that:
    /// 1. Receives `SlackAction` requests from the channel
    /// 2. Processes each action by calling the appropriate Slack API
    /// 3. Handles rate limiting between requests
    /// 4. Terminates when the channel is closed
    ///
    pub async fn run(mut self) {
        tracing::info!(
            channel = %self.channel,
            "SlackWorker started."
        );

        while let Some(text) = self.receiver.recv().await {
            // Retry loop for the message
            let mut retry_attempts = 0;
            loop {
                let now = Instant::now();
                if now < self.next_allowed_at {
                    sleep(self.next_allowed_at - now).await;
                }

                match api::post_msg(&self.client, &self.token, &self.channel, &text).await {
                    Ok(_) => {
                        tracing::info!(
                            channel = %self.channel,
                            "Slack message sent successfully."
                        );
                        break;
                    }

                    Err(e) => {
                        match e.current_context() {
                            Error::RatelimitExceeded(Some(retry_after)) => {
                                tracing::warn!(
                                    channel = %self.channel,
                                    "Slack rate limit exceeded. Retry after {:?}",
                                    retry_after
                                );

                                // Update global window and retry same message
                                self.next_allowed_at = Instant::now() + *retry_after;
                            }

                            Error::RatelimitExceeded(None) => {
                                tracing::warn!(
                                    channel = %self.channel,
                                    "Slack rate limit exceeded without Retry-After",
                                );

                                // Conservative fallback
                                self.next_allowed_at = Instant::now() + self.base_throttle;
                            }

                            other => {
                                tracing::error!(
                                    channel = %self.channel,
                                    "Slack message failed with unexpected error: {:?}",
                                    other
                                );
                                retry_attempts += 1;
                                if retry_attempts >= 5 {
                                    tracing::error!(
                                        channel = %self.channel,
                                        "Slack message failed after {} attempts, giving up. Message: {}",
                                        retry_attempts,
                                        text
                                    );
                                    break;
                                }
                                // Exponential backoff fallback
                                self.next_allowed_at =
                                    Instant::now() + Duration::from_secs(retry_attempts);
                            }
                        }
                    }
                }
            }
        }

        tracing::info!(
            channel = %self.channel,
            "SlackWorker shutting down."
        );
    }
}
