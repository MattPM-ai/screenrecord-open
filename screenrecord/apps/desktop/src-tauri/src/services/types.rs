use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub name: String,
    pub running: bool,
    pub pid: Option<u32>,
    pub port: Option<u16>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllServicesStatus {
    pub mongodb: ServiceStatus,
    pub influxdb: ServiceStatus,
    pub collector: ServiceStatus,
    pub report: ServiceStatus,
    pub chat_agent: ServiceStatus,
    pub frontend: ServiceStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceProgress {
    pub service: String,
    pub status: String, // "starting", "ready", "failed", "skipped"
    pub message: Option<String>,
}

