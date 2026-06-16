#![allow(non_snake_case)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Memory {
    pub total: f64,
    pub used: f64,
    pub free: f64,
    pub percent: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Disk {
    pub total: f64,
    pub used: f64,
    pub free: f64,
    pub percent: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Network {
    pub rxBytes: u64,
    pub txBytes: u64,
    pub rxSec: Option<f64>,
    pub txSec: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct System {
    pub uptime: f64,
    pub loadAvg: [f64; 3],
    pub cpuUsage: f64,
    pub memory: Memory,
    pub disk: Disk,
    pub network: Network,
    pub cpuTemp: Option<f64>,
    pub chassisTemp: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FahSystemdStatus {
    Active,
    Inactive,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fah {
    pub systemdStatus: FahSystemdStatus,
    pub project: Option<String>,
    pub run: Option<f64>,
    pub clone: Option<f64>,
    pub gen: Option<f64>,
    pub progress: Option<f64>,
    pub ppd: Option<f64>,
    pub tpf: Option<String>,
    pub recentErrors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statsDonor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statsTeam: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Maintenance {
    pub aptUpdatesAvailable: u32,
    pub rebootRequired: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeLogs {
    pub fah: Vec<String>,
    pub work: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fahPath: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workPath: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IngestPayload {
    pub hostname: String,
    pub timestamp: String,
    pub system: System,
    pub fah: Fah,
    pub maintenance: Maintenance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logs: Option<NodeLogs>,
}
