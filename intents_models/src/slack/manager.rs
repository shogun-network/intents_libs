//! Slack messaging coordination and management.
//!
//! This module provides a manager that coordinates Slack messaging operations
//! by routing message requests from external callers to internal workers.
//! The manager acts as a broker between application code that wants to send
//! messages and the rate-limited worker implementation that interacts with
//! the Slack API.

use crate::error::{Error, ModelResult};
use error_stack::report;
use tokio::sync::mpsc::Receiver;

use super::{actions::SlackAction, worker::SlackWorker};

/// Manager for coordinating Slack messaging operations.
///
/// `SlackManager` is responsible for:
/// 1. Receiving message requests from application code
/// 2. Forwarding them to a rate-limited worker for processing
/// 3. Managing the lifecycle of the worker
///
/// The manager serves as a middleman between code that wants to send messages
/// and the actual implementation that handles rate limiting and API calls.
#[derive(Debug)]
pub struct SlackManager {
    /// Slack API authentication token
    token: String,
    /// Channel receiver for incoming action requests from external code
    input_channel: Receiver<SlackAction>,
}

impl SlackManager {
    pub fn new(token: String, input_channel: Receiver<SlackAction>) -> Self {
        SlackManager {
            token,
            input_channel,
        }
    }

    /// Starts the manager's main processing loop.
    ///
    /// This method:
    /// 1. Creates an internal worker to handle Slack API calls
    /// 2. Forwards incoming action requests to the worker
    /// 3. Runs until the input channel is closed
    /// 4. Ensures all pending messages are processed before shutting down
    pub async fn run(mut self) -> ModelResult<()> {
        // Create channel for internal communication with worker
        let (worker_sender, worker_receiver) = tokio::sync::mpsc::channel(1000);
        // Start the message worker in a separate task
        let worker = SlackWorker::new(self.token.clone(), worker_receiver);

        // Spawn the worker in a separate task
        let worker_handle = tokio::spawn(async move {
            worker.run().await;
        });

        // Receive msgs at external channel and handle them
        while let Some(action) = self.input_channel.recv().await {
            match action {
                SlackAction::SendMessage { channel, text } => {
                    // Send message to internal channel
                    if let Err(e) = worker_sender
                        .send(SlackAction::SendMessage { channel, text })
                        .await
                    {
                        tracing::error!("Failed to send message to internal channel: {e}");
                    }
                }
            }
        }

        // Input channel is closed, wait for worker to finish processing remaining messages
        let _ = worker_handle.await;

        Err(report!(Error::ModuleStopped("SlackManager".to_string())))
    }
}
