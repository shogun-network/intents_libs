//! Worker implementation for asynchronous Slack messaging.
//!
//! This module provides a worker that processes Slack message requests
//! asynchronously through a channel, implementing rate limiting to comply
//! with Slack API restrictions.

use crate::slack::api;
use tokio::sync::mpsc::Receiver;
use tokio::time::{Duration, sleep};

use super::actions::SlackAction;

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
    /// Slack API authentication token
    token: String,
    /// Channel receiver for incoming Slack action requests
    receiver: Receiver<SlackAction>,
}

impl SlackWorker {
    pub fn new(token: String, receiver: Receiver<SlackAction>) -> Self {
        SlackWorker { token, receiver }
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
        tracing::info!("Slack Worker started");

        // Slack's rate limit is around 1 message per second
        const RATE_LIMIT: Duration = Duration::from_millis(1050);

        while let Some(action) = self.receiver.recv().await {
            match action {
                SlackAction::SendMessage { channel, text } => {
                    // Call Slack API to send the message
                    match api::post_msg(&self.token, &channel, &text).await {
                        Ok(_) => tracing::info!("Slack Message sent successfully"),
                        Err(e) => tracing::error!("Failed to send  slack message: {e}"),
                    }

                    // Respect rate limits
                    sleep(RATE_LIMIT).await;
                }
            }
        }

        tracing::info!("SlackWorker shutting down - channel closed");
    }
}
