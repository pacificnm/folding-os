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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activeClientVersion: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expectedClientVersion: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clientInstalled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clientVerified: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acquisitionFailures: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acquisitionNextAttemptUnix: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acquisitionLastFailureReason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logPath: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logReadable: Option<bool>,
    pub project: Option<String>,
    pub run: Option<f64>,
    pub clone: Option<f64>,
    pub gen: Option<f64>,
    pub progress: Option<f64>,
    pub ppd: Option<f64>,
    pub tpf: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foldingState: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unitState: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foldingDetail: Option<String>,
    pub recentErrors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statsDonor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statsTeam: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configUsername: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configTeam: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configPasskeyConfigured: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configCpus: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effectiveCpus: Option<i64>,
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
pub struct HostHardwareCpu {
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    pub physicalCores: u32,
    pub logicalThreads: u32,
    pub architecture: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostHardwareMemoryModule {
    pub sizeBytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speedMts: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locator: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostHardwareMemory {
    pub totalBytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moduleSlotsDetected: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modules: Option<Vec<HostHardwareMemoryModule>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostHardwareNamedBlock {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sku: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typeCode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostHardwareStorage {
    pub name: String,
    pub sizeBytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotational: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostHardwareNetwork {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macAddress: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operstate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speedMbps: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pciAddress: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostHardwarePciDevice {
    pub address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendorId: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deviceId: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classId: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostHardwareProfile {
    pub cpu: HostHardwareCpu,
    pub memory: HostHardwareMemory,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<HostHardwareNamedBlock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<HostHardwareNamedBlock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chassis: Option<HostHardwareNamedBlock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bios: Option<HostHardwareNamedBlock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<Vec<HostHardwareStorage>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<Vec<HostHardwareNetwork>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pciDevices: Option<Vec<HostHardwarePciDevice>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IngestPayload {
    pub hostname: String,
    pub timestamp: String,
    pub system: System,
    pub fah: Fah,
    pub maintenance: Maintenance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nodeId: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installationRole: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foldingosVersion: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primaryIpv4: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logs: Option<NodeLogs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hardware: Option<HostHardwareProfile>,
}
