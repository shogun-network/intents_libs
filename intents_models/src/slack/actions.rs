/// Represents actions that can be performed with the Slack API.
///
/// This enum defines the various operations that the Slack subsystem
/// can perform. Currently it only supports sending messages, but could
/// be extended to support other Slack API operations like updating messages,
/// adding reactions, listening to events...
#[derive(Debug)]
pub enum SlackAction {
    /// Sends a text message to a Slack channel.
    ///
    /// # Fields
    ///
    /// * `channel` - The target Slack channel
    /// * `text` - The message content to send
    SendMessage { channel: String, text: String },
}
