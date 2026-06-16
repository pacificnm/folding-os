use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

use chrono::Utc;
use foldops_types::{
    Disk, Fah, FahSystemdStatus, IngestPayload, Maintenance, Memory, Network, NodeLogs, System,
};
use sysinfo::{Disks, Networks, System as SysInfo};

use crate::fah::get_newest_work_log_path;
use crate::fah::{collect_fah_status, FahLogState};
use crate::log_tail::read_log_tail_default;
use crate::temperatures::collect_temperatures;

const INGEST_LOG_LINES: usize = 100;

#[derive(Debug, Clone)]
pub struct FahStats {
    pub donor: Option<String>,
    pub team: Option<String>,
}

struct NetworkSample {
    rx_bytes: u64,
    tx_bytes: u64,
    at: Instant,
}

static LAST_NETWORK: Mutex<Option<NetworkSample>> = Mutex::new(None);

pub struct CollectPaths<'a> {
    pub fah_log_path: &'a Path,
    pub fah_db_path: &'a Path,
    pub fah_work_dir: &'a Path,
    pub fah_ws_host: &'a str,
    pub fah_ws_port: u16,
    pub fah_stats: FahStats,
}

pub async fn collect_snapshot(paths: CollectPaths<'_>) -> IngestPayload {
    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".into());

    let fah_systemd = get_fah_systemd_status().await;

    let fah_result = collect_fah_status(
        paths.fah_log_path,
        paths.fah_db_path,
        paths.fah_work_dir,
        paths.fah_ws_host,
        paths.fah_ws_port,
    )
    .await;

    if let Some(err) = &fah_result.db_error {
        if fah_systemd == FahSystemdStatus::Active {
            let from_log = fah_result.state.project.is_some()
                || fah_result.state.progress.is_some_and(|p| p > 0.0);
            if from_log {
                tracing::info!(hostname = %hostname, error = %err, "client.db error (project/progress from log until RUN)");
            } else {
                tracing::warn!(hostname = %hostname, error = %err, "client.db error");
            }
        }
    } else if let (Some(src), Some(ppd)) = (&fah_result.db_source, fah_result.state.ppd) {
        tracing::info!(hostname = %hostname, source = %src, ppd, "FAH metrics source");
    }

    let fah_log = fah_result.state;

    let (mem_total, mem_used, mem_free, cpu_usage, load_avg, uptime) =
        tokio::task::spawn_blocking(collect_system_metrics)
            .await
            .unwrap_or((0, 0, 0, 0.0, [0.0; 3], 0.0));

    let (disk_total, disk_used, disk_free) =
        tokio::task::spawn_blocking(collect_disk_metrics)
            .await
            .unwrap_or((0, 0, 0));

    let (rx_bytes, tx_bytes, rx_sec, tx_sec) =
        tokio::task::spawn_blocking(collect_network_rates)
            .await
            .unwrap_or((0, 0, None, None));

    let apt_updates = get_apt_updates_available().await;
    let reboot_required = tokio::fs::metadata("/var/run/reboot-required")
        .await
        .is_ok();
    let temps = collect_temperatures().await;

    let fah_tail = read_log_tail_default(paths.fah_log_path, INGEST_LOG_LINES).await;
    let work_path = get_newest_work_log_path(paths.fah_work_dir).await;
    let work_tail = match work_path.as_ref() {
        Some(p) => read_log_tail_default(p, INGEST_LOG_LINES).await,
        None => None,
    };

    let mem_percent = if mem_total > 0 {
        ((mem_used as f64 / mem_total as f64) * 1000.0).round() / 10.0
    } else {
        0.0
    };
    let disk_percent = if disk_total > 0 {
        ((disk_used as f64 / disk_total as f64) * 1000.0).round() / 10.0
    } else {
        0.0
    };

    IngestPayload {
        hostname,
        timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        system: System {
            uptime,
            loadAvg: load_avg,
            cpuUsage: cpu_usage,
            memory: Memory {
                total: mem_total as f64,
                used: mem_used as f64,
                free: mem_free as f64,
                percent: mem_percent,
            },
            disk: Disk {
                total: disk_total as f64,
                used: disk_used as f64,
                free: disk_free as f64,
                percent: disk_percent,
            },
            network: Network {
                rxBytes: rx_bytes,
                txBytes: tx_bytes,
                rxSec: rx_sec,
                txSec: tx_sec,
            },
            cpuTemp: temps.cpu_temp,
            chassisTemp: temps.chassis_temp,
        },
        fah: fah_log_to_payload(fah_log, fah_systemd, &paths.fah_stats),
        maintenance: Maintenance {
            aptUpdatesAvailable: apt_updates,
            rebootRequired: reboot_required,
        },
        logs: Some(NodeLogs {
            fah: fah_tail.as_ref().map(|t| t.lines.clone()).unwrap_or_default(),
            work: work_tail.as_ref().map(|t| t.lines.clone()).unwrap_or_default(),
            fahPath: fah_tail.map(|t| t.path),
            workPath: work_tail.map(|t| t.path),
        }),
    }
}

fn fah_log_to_payload(state: FahLogState, systemd: FahSystemdStatus, stats: &FahStats) -> Fah {
    Fah {
        systemdStatus: systemd,
        project: state.project,
        run: state.run,
        clone: state.clone,
        gen: state.gen,
        progress: state.progress,
        ppd: state.ppd,
        tpf: state.tpf,
        recentErrors: state.recent_errors,
        statsDonor: stats.donor.clone(),
        statsTeam: stats.team.clone(),
    }
}

fn collect_system_metrics() -> (u64, u64, u64, f64, [f64; 3], f64) {
    let mut sys = SysInfo::new();
    sys.refresh_memory();
    sys.refresh_cpu_usage();
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    sys.refresh_cpu_usage();

    let load = SysInfo::load_average();
    let load_avg = [load.one, load.five, load.fifteen];
    let cpu_usage = ((sys.global_cpu_usage() as f64) * 10.0).round() / 10.0;
    let uptime = SysInfo::uptime() as f64;

    (sys.total_memory(), sys.used_memory(), sys.free_memory(), cpu_usage, load_avg, uptime)
}

fn collect_disk_metrics() -> (u64, u64, u64) {
    let disks = Disks::new_with_refreshed_list();
    let root = disks
        .iter()
        .find(|d| d.mount_point().to_string_lossy() == "/")
        .or_else(|| disks.iter().next());

    match root {
        Some(d) => {
            let total = d.total_space();
            let free = d.available_space();
            let used = total.saturating_sub(free);
            (total, used, free)
        }
        None => (0, 0, 0),
    }
}

fn collect_network_rates() -> (u64, u64, Option<f64>, Option<f64>) {
    let networks = Networks::new_with_refreshed_list();
    let primary = networks
        .iter()
        .find(|(name, _)| !name.starts_with("lo"))
        .map(|(_, data)| data)
        .or_else(|| networks.iter().next().map(|(_, d)| d));

    let rx_bytes = primary.map(|n| n.total_received()).unwrap_or(0);
    let tx_bytes = primary.map(|n| n.total_transmitted()).unwrap_or(0);
    let now = Instant::now();

    let mut guard = LAST_NETWORK.lock().unwrap_or_else(|e| e.into_inner());
    let (rx_sec, tx_sec) = if let Some(prev) = guard.as_ref() {
        let elapsed = now.duration_since(prev.at).as_secs_f64();
        if elapsed > 0.0 {
            (
                Some(((rx_bytes.saturating_sub(prev.rx_bytes)) as f64 / elapsed).max(0.0)),
                Some(((tx_bytes.saturating_sub(prev.tx_bytes)) as f64 / elapsed).max(0.0)),
            )
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    *guard = Some(NetworkSample {
        rx_bytes,
        tx_bytes,
        at: now,
    });

    (rx_bytes, tx_bytes, rx_sec, tx_sec)
}

async fn get_fah_systemd_status() -> FahSystemdStatus {
    let output = tokio::process::Command::new("systemctl")
        .args(["is-active", "fah-client"])
        .output()
        .await;

    match output {
        Ok(o) if o.status.success() => match String::from_utf8_lossy(&o.stdout).trim() {
            "active" => FahSystemdStatus::Active,
            "inactive" => FahSystemdStatus::Inactive,
            "failed" => FahSystemdStatus::Failed,
            _ => FahSystemdStatus::Unknown,
        },
        Ok(o) => match String::from_utf8_lossy(&o.stdout).trim() {
            "inactive" => FahSystemdStatus::Inactive,
            "failed" => FahSystemdStatus::Failed,
            _ => FahSystemdStatus::Unknown,
        },
        Err(_) => FahSystemdStatus::Unknown,
    }
}

async fn get_apt_updates_available() -> u32 {
    let output = tokio::process::Command::new("bash")
        .args([
            "-c",
            "apt list --upgradable 2>/dev/null | grep -c upgradable || true",
        ])
        .output()
        .await;

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .trim()
            .parse()
            .unwrap_or(0),
        _ => 0,
    }
}
