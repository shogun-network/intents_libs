use crate::error::ModelResult;

pub mod actions;
pub mod api;
pub mod client;
pub mod constants;
pub mod manager;
pub mod responses;
pub mod worker;

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
