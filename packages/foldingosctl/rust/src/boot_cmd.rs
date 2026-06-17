use std::fs;
use std::io::{BufRead, BufReader};
use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::identity::{candidate_network_interfaces, routable_ipv4_address};
use crate::paths::AppliancePaths;
use crate::process::write_console;
use crate::role::read_installation_role_for_display;

const ADMIN_SSH_USER: &str = "foldingos-admin";
const OS_RELEASE_PATH: &str = "/usr/lib/os-release";
const CONSOLE_CLEAR_SCREEN: &str = "\x1b[2J\x1b[3J\x1b[H";
const BOOT_STATUS_RETRY_ATTEMPTS: usize = 90;
const COMMISSIONING_DISPLAY_WIDTH: usize = 62;
const BOOT_SERVICE_WAIT_TIMEOUT: Duration = Duration::from_secs(180);
const BOOT_SERVICE_POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
struct CommissioningCheck {
    label: String,
    ready: bool,
}

pub fn boot_status(paths: &AppliancePaths) -> Result<(), String> {
    write_commissioning_display(paths, true)
}

fn write_commissioning_display(paths: &AppliancePaths, wait_for_services: bool) -> Result<(), String> {
    let mut pretty_name = os_release_value("PRETTY_NAME")?;
    if pretty_name.is_empty() {
        pretty_name = os_release_value("VERSION")?;
    }
    if pretty_name.is_empty() {
        pretty_name = "FoldingOS".into();
    }
    let mut version = os_release_value("VERSION_ID")?;
    if version.is_empty() {
        version = os_release_value("VERSION").unwrap_or_default();
    }

    let mut network_err = None;
    let mut address = None;
    for _ in 0..BOOT_STATUS_RETRY_ATTEMPTS {
        if let Some(found) = routable_ipv4_address() {
            address = Some(found);
            break;
        }
        network_err = Some(routable_ipv4_error());
        thread::sleep(Duration::from_secs(1));
    }
    let Some(address) = address else {
        let err = network_err.unwrap_or_else(|| "no routable IPv4 address available".into());
        let message = failure_display_message(&pretty_name, &err);
        clear_console()?;
        write_console(&message)?;
        eprintln!("{err}");
        return Ok(());
    };

    let role = read_installation_role_for_display(paths);
    let mut checks = evaluate_commissioning_checks(paths, &role);
    if wait_for_services && commissioning_checks_pending(&checks) {
        let deadline = std::time::Instant::now() + BOOT_SERVICE_WAIT_TIMEOUT;
        while std::time::Instant::now() < deadline {
            checks = evaluate_commissioning_checks(paths, &role);
            if !commissioning_checks_pending(&checks) {
                break;
            }
            thread::sleep(BOOT_SERVICE_POLL_INTERVAL);
        }
    }

    let message = format_commissioning_display(&pretty_name, &version, &role, &address, &checks);
    clear_console()?;
    write_console(&message)?;
    print_commissioning_status_summary(&checks);
    println!("Wrote FoldingOS commissioning display status.");
    Ok(())
}

fn routable_ipv4_error() -> String {
    let output = Command::new("networkctl")
        .args(["--no-legend", "--no-pager", "list"])
        .output();
    match output {
        Ok(output) if output.status.success() => {
            let listing = String::from_utf8_lossy(&output.stdout);
            if candidate_network_interfaces(&listing).is_err() {
                return "no wired network interface found".into();
            }
            "no routable IPv4 address available".into()
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if stderr.is_empty() {
                "networkctl list failed".into()
            } else {
                stderr
            }
        }
        Err(error) => error.to_string(),
    }
}

fn os_release_value(key: &str) -> Result<String, String> {
    let file = fs::File::open(OS_RELEASE_PATH)
        .map_err(|error| format!("read os-release: {error}"))?;
    let prefix = format!("{key}=");
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|error| error.to_string())?;
        let line = line.trim();
        if let Some(value) = line.strip_prefix(&prefix) {
            return Ok(value.trim_matches('"').to_string());
        }
    }
    Ok(String::new())
}

fn format_ready_display(pretty_name: &str, address: &str) -> String {
    format!(
        "{pretty_name} ready\nAddress: {address}\nSSH: {ADMIN_SSH_USER}@{address}\n"
    )
}

fn failure_display_message(pretty_name: &str, err: &str) -> String {
    format!("{pretty_name}\nNetwork: {err}\n")
}

fn evaluate_commissioning_checks(paths: &AppliancePaths, role: &str) -> Vec<CommissioningCheck> {
    let mut checks = vec![
        CommissioningCheck {
            label: "Network online".into(),
            ready: true,
        },
        check_systemd_unit("SSH administrator provisioned", "foldingos-ssh-provision.service"),
        check_installation_role(role),
        check_foldops_packages(paths),
        check_foldops_provisioned(paths),
    ];
    if role == "supervisor" {
        checks.extend([
            check_systemd_unit(
                "FoldOps HTTPS (port 3443)",
                "foldingos-foldops-serve-https.service",
            ),
            check_systemd_unit(
                "FoldOps supervisor (loopback)",
                "foldingos-foldops-supervisor.service",
            ),
            check_systemd_unit("Provisioning control plane", "foldingos-provision.service"),
        ]);
    }
    checks.extend([
        check_systemd_unit("FoldOps agent", "foldingos-foldops-agent.service"),
        check_systemd_unit("Folding@home client", "folding-at-home.service"),
    ]);
    checks
}

fn commissioning_checks_pending(checks: &[CommissioningCheck]) -> bool {
    checks.iter().any(|check| !check.ready)
}

fn check_systemd_unit(label: &str, unit: &str) -> CommissioningCheck {
    CommissioningCheck {
        label: label.into(),
        ready: systemd_unit_is_active(unit),
    }
}

fn check_installation_role(role: &str) -> CommissioningCheck {
    CommissioningCheck {
        label: "Installation role active".into(),
        ready: role == "supervisor" || role == "agent",
    }
}

fn check_foldops_packages(paths: &AppliancePaths) -> CommissioningCheck {
    CommissioningCheck {
        label: "FoldOps packages acquired".into(),
        ready: paths.foldops_current_link().exists(),
    }
}

fn check_foldops_provisioned(paths: &AppliancePaths) -> CommissioningCheck {
    CommissioningCheck {
        label: "FoldOps provisioned".into(),
        ready: paths.foldops_provisioned_marker.exists(),
    }
}

fn systemd_unit_is_active(unit: &str) -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", unit])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn format_commissioning_display(
    pretty_name: &str,
    version: &str,
    role: &str,
    address: &str,
    checks: &[CommissioningCheck],
) -> String {
    let ready_lines = format_ready_display(pretty_name, address);
    let all_ready = !commissioning_checks_pending(checks);
    let status_line = if all_ready {
        "System ready"
    } else {
        "Some services are still starting"
    };

    let mut output = String::new();
    output.push_str(&render_commissioning_box(pretty_name, status_line));
    output.push('\n');
    output.push_str(&ready_lines);
    output.push('\n');
    if !version.is_empty() {
        output.push_str(&format!("Version       : {version}\n"));
    }
    if !role.is_empty() && role != "unknown" {
        output.push_str(&format!("Role          : {role}\n"));
    }
    output.push('\n');
    for check in checks {
        output.push_str(&format_commissioning_check_line(check));
        output.push('\n');
    }
    output.push_str("\nhttps://folding-os.com\n");
    output
}

fn render_commissioning_box(title: &str, status_line: &str) -> String {
    let inner_width = COMMISSIONING_DISPLAY_WIDTH - 2;
    let lines = [
        center_display_text(title, inner_width),
        center_display_text(status_line, inner_width),
    ];
    let border = "═".repeat(inner_width);
    let mut output = format!("╔{border}╗\n");
    for line in lines {
        output.push_str(&format!("║{line}║\n"));
    }
    output.push_str(&format!("╚{border}╝"));
    output
}

fn center_display_text(text: &str, width: usize) -> String {
    let text_len = text.chars().count();
    if text_len >= width {
        return truncate_display_chars(text, width);
    }
    let padding = width - text_len;
    let left = padding / 2;
    let right = padding - left;
    format!("{}{}{}", " ".repeat(left), text, " ".repeat(right))
}

fn truncate_display_chars(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    text.chars().take(width).collect()
}

fn format_commissioning_check_line(check: &CommissioningCheck) -> String {
    let marker = if check.ready { "✓" } else { "✗" };
    format!("{marker} {}", check.label)
}

fn print_commissioning_status_summary(checks: &[CommissioningCheck]) {
    println!("Commissioning service status:");
    for check in checks {
        let state = if check.ready { "ready" } else { "pending" };
        println!("  {}: {state}", check.label);
    }
}

fn clear_console() -> Result<(), String> {
    write_console(CONSOLE_CLEAR_SCREEN)
}
