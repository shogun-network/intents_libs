use crate::{
    error::ModelResult,
    slack::{client::SlackClient, manager::SlackManager},
};

pub mod actions;
pub mod api;
pub mod client;
pub mod constants;
pub mod manager;
pub mod responses;
pub mod worker;

#[derive(Debug, Clone)]
pub struct SlackConfig {
    pub token: String,
    pub info_channel: Option<String>,
    pub errors_channel: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SlackClients {
    pub info: Option<client::SlackClient>,
    pub errors: Option<client::SlackClient>,
}

impl SlackClients {
    pub fn new(info: Option<client::SlackClient>, errors: Option<client::SlackClient>) -> Self {
        Self { info, errors }
    }

    /// Sends an informational message to the info channel.
    pub async fn send_info(&self, text: String) -> ModelResult<()> {
        if let Some(info_client) = &self.info {
            return info_client.send_message(text).await;
        }
        Ok(())
    }

    /// Sends an error message to the errors channel.
    pub async fn send_error(&self, text: String) -> ModelResult<()> {
        if let Some(errors_client) = &self.errors {
            return errors_client.send_message(text).await;
        }
        Ok(())
    }
}

pub fn initialize_slack_messages(
    slack_config: Option<SlackConfig>,
) -> (SlackClients, Option<SlackManager>) {
    {
        match slack_config {
            Some(slack_config) => {
                let (slack_action_sender, slack_action_receiver) = tokio::sync::mpsc::channel(1000);

                let slack_manager = SlackManager::new(slack_config.token, slack_action_receiver);

                let slack_info = slack_config.info_channel.as_ref().map(|info_channel| {
                    SlackClient::new(slack_action_sender.clone(), info_channel.clone())
                });
                let slack_errors = slack_config.errors_channel.as_ref().map(|errors_channel| {
                    SlackClient::new(slack_action_sender.clone(), errors_channel.clone())
                });

                if slack_info.is_none() && slack_errors.is_none() {
                    tracing::warn!(
                        "Slack config provided but no channels configured. SlackManager will not be started."
                    );
                    return (SlackClients::new(None, None), None);
                }

                (
                    SlackClients::new(slack_info, slack_errors),
                    Some(slack_manager),
                )
            }
            None => {
                tracing::info!("Slack config is not set, skipping SlackManager initialization");
                (SlackClients::new(None, None), None)
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::{actions::SlackAction, client::SlackClient, manager::SlackManager, *};

    pub fn get_mock_slack() -> (SlackManager, SlackClient) {
        let (sender, receiver) = tokio::sync::mpsc::channel::<SlackAction>(1000);
        let manager = manager::SlackManager::new("mock_token".to_string(), receiver);
        let client = SlackClient::new(sender, "mock_channel".to_string());
        (manager, client)
    }

    pub fn get_mock_slack_clients() -> SlackClients {
        let (_, info_client) = get_mock_slack();
        let (_, errors_client) = get_mock_slack();
        SlackClients::new(Some(info_client), Some(errors_client))
    }
}
