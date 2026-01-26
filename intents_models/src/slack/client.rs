//! Slack client for sending messages to Slack channels.
//!
//! This module provides a simple client interface for interacting with Slack.
//! It abstracts away the details of the message queue and worker implementation,
//! offering a clean API for sending messages to configured Slack channels.

use crate::error::{Error, ModelResult};
use error_stack::ResultExt;
use tokio::sync::mpsc::Sender;

use super::actions::SlackAction;

/// Client for sending messages to a Slack channel.
///
/// `SlackClient` provides a high-level interface for sending messages to a specific
/// Slack channel. It internally communicates with a worker through a message queue,
/// which handles rate limiting and actual API calls.
///
#[derive(Debug, Clone)]
pub struct SlackClient {
    /// Channel for sending commands to the Slack worker
    command_tx: Sender<SlackAction>,
    /// Target Slack channel for messages
    channel: String,
}

impl SlackClient {
    pub fn new(command_tx: Sender<SlackAction>, channel: String) -> Self {
        Self {
            command_tx,
            channel,
        }
    }

    /// Sends a message to the configured Slack channel.
    ///
    /// This method asynchronously sends a message to the Slack channel
    /// specified during the client's creation. The actual sending is handled
    /// by a worker process, allowing this method to return quickly.
    ///
    /// # Arguments
    ///
    /// * `text` - The message text to send
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the message was successfully queued for delivery
    /// * `Err(...)` - If the message could not be queued
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be sent to the worker,
    /// typically because the worker has been shut down.
    pub async fn send_message(&self, text: String) -> ModelResult<()> {
        let action = SlackAction::SendMessage {
            channel: self.channel.clone(),
            text,
        };
        self.command_tx
            .send(action)
            .await
            .change_context(Error::ClientMessageError(format!(
                "Failed to send message to Slack channel: {}",
                self.channel
            )))
    }
}
