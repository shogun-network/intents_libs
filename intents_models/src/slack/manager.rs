//! Slack messaging coordination and management.
//!
//! This module provides a manager that coordinates Slack messaging operations
//! by routing message requests from external callers to internal workers.
//! The manager acts as a broker between application code that wants to send
//! messages and the rate-limited worker implementation that interacts with
//! the Slack API.

use std::collections::HashMap;

use crate::error::{Error, ModelResult};
use error_stack::report;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;

use super::{actions::SlackAction, worker::SlackWorker};

#[derive(Debug)]
struct WorkerHandle {
    sender: Sender<String>,
    _task: JoinHandle<()>,
}

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
        tracing::info!("SlackManager started");

        // Workers indexed by channel id
        let mut workers: HashMap<String, WorkerHandle> = HashMap::new();

        while let Some(action) = self.input_channel.recv().await {
            match action {
                SlackAction::SendMessage { channel, text } => {
                    let worker = workers.entry(channel.clone()).or_insert_with(|| {
                        tracing::info!(
                            channel = %channel,
                            "Spawning SlackWorker for channel"
                        );

                        let (tx, rx) = tokio::sync::mpsc::channel::<String>(1024);

                        let worker = SlackWorker::new(self.token.clone(), channel.clone(), rx);

                        let task = tokio::spawn(async move {
                            worker.run().await;
                        });

                        WorkerHandle {
                            sender: tx,
                            _task: task,
                        }
                    });

                    if let Err(e) = worker.sender.send(text).await {
                        tracing::error!(
                            channel = %channel,
                            error = %e,
                            "Failed to send message to SlackWorker"
                        );
                    }
                }
            }
        }

        tracing::info!("SlackManager shutting down, input channel closed");

        // Drop all senders so workers can exit cleanly
        workers.clear();

        Err(report!(Error::ModuleStopped("SlackManager".to_string())))
    }
}
