use tokio::sync::mpsc::{Receiver, Sender};

use crate::monitoring::messages::{MonitorAlert, MonitorRequest};

#[derive(Debug)]
pub struct MonitorManager {
    pub receiver: Receiver<MonitorRequest>,
    pub sender: Sender<MonitorAlert>,
}

impl MonitorManager {
    pub fn new(receiver: Receiver<MonitorRequest>, sender: Sender<MonitorAlert>) -> Self {
        Self { receiver, sender }
    }

    pub async fn run(&mut self) {
        while let Some(request) = self.receiver.recv().await {
            tracing::debug!("Received monitor request: {:?}", request);
        }
    }
}
