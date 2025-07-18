use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum MonitorRequest {}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum MonitorAlert {}
