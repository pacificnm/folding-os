use crate::automation::{
    format_automation_command, write_failure, write_success, AutomationContext, MIGRATION_MARKER,
    OutputFormat,
};
use crate::boot_cmd::{self, boot_status};
use crate::config_cmd::{self, ConfigCommandOutput};
use crate::fah;
use crate::foldops;
use crate::identity::ensure_identity;
use crate::inspect;
use crate::paths::AppliancePaths;
use crate::provision;
use crate::registry_cmd::{self, RegistryOutput};
use crate::storage::expand_data;
use crate::tools;

const USAGE: &str =
    "usage: foldingosctl <boot|config|fah|foldops|identity|inspect|provision|registry|storage|tools> <command> [arguments]";

const PROVISION_JSON_COMMANDS: &[&str] = &[
    "list-enrollments",
    "assign",
    "list-allow-boot",
    "allow-boot",
];

#[derive(Debug)]
pub enum CliError {
    Usage,
    Failed(String),
    AlreadyReported,
    Exit(i32),
}

impl CliError {
    pub fn message(&self) -> String {
        match self {
            Self::Usage => USAGE.to_string(),
            Self::Failed(message) => message.clone(),
            Self::AlreadyReported => String::new(),
            Self::Exit(_) => String::new(),
        }
    }
}

pub fn dispatch(mut args: Vec<String>) -> Result<(), CliError> {
    let (clean_args, format) = crate::automation::strip_format_flag(&args);
    args = clean_args;

    if args.is_empty() {
        return Err(CliError::Usage);
    }

    if args.len() == 2 && args[0] == "migration" && args[1] == "status" {
        return print_migration_status(format);
    }

    let paths = AppliancePaths::default();

    if args[0] == "inspect" {
        return dispatch_json_group(
            format,
            &["inspect", &args.get(1).cloned().unwrap_or_default()],
            args.get(1).cloned().ok_or(CliError::Usage)?,
            args.get(2..).unwrap_or(&[]).to_vec(),
            |paths, subcommand, extra| inspect::run(paths, subcommand, extra),
            &paths,
        );
    }

    if args[0] == "provision" {
        if args.len() < 2 {
            return Err(CliError::Usage);
        }
        let subcommand = args[1].clone();
        let extra = args[2..].to_vec();
        if PROVISION_JSON_COMMANDS.contains(&subcommand.as_str()) {
            let command = format_automation_command(&["provision", &subcommand]);
            let ctx = AutomationContext::new(format, command);
            return match provision::run(&paths, &subcommand, &extra) {
                Ok(data) => publish_success(&ctx, data, &subcommand),
                Err(message) => publish_failure(&ctx, message),
            };
        }
        return match provision::run(&paths, &subcommand, &extra) {
            Ok(_) => Ok(()),
            Err(message) if subcommand == "apply-update" && message.contains("not schedulable") => {
                Err(CliError::Exit(1))
            }
            Err(message) => Err(CliError::Failed(message)),
        };
    }

    if args[0] == "registry" {
        if args.len() < 2 {
            return Err(CliError::Usage);
        }
        let subcommand = args[1].clone();
        let extra = args[2..].to_vec();
        let command = if subcommand == "show" && !extra.is_empty() {
            format_automation_command(&["registry", &subcommand, &extra[0]])
        } else {
            format_automation_command(&["registry", &subcommand])
        };
        let ctx = AutomationContext::new(format, command);
        return match registry_cmd::run(&paths, &subcommand, &extra) {
            Ok(RegistryOutput::Json(data)) => publish_success(&ctx, data, &subcommand),
            Ok(RegistryOutput::Silent) => Ok(()),
            Err(message) => publish_failure(&ctx, message),
        };
    }

    if args[0] == "config" {
        if args.len() < 2 {
            return Err(CliError::Usage);
        }
        let subcommand = args[1].clone();
        let extra = args[2..].to_vec();
        let command = match subcommand.as_str() {
            "validate" | "effective" if !extra.is_empty() => {
                format_automation_command(&["config", &subcommand, &extra[0]])
            }
            _ => format_automation_command(&["config", &subcommand]),
        };
        let ctx = AutomationContext::new(format, command);
        return match config_cmd::run(&paths, &subcommand, &extra, format) {
            Ok(ConfigCommandOutput::Json(data)) => publish_success(&ctx, data, &subcommand),
            Ok(ConfigCommandOutput::EffectiveHuman(content)) => {
                print!("{content}");
                Ok(())
            }
            Ok(ConfigCommandOutput::Silent) => Ok(()),
            Err(message) => publish_failure(&ctx, message),
        };
    }

    if args.len() == 2 && args[0] == "identity" && args[1] == "ensure" {
        return ensure_identity(&paths).map_err(CliError::Failed);
    }

    if args.len() == 2 && args[0] == "storage" && args[1] == "expand-data" {
        return expand_data(&paths).map_err(CliError::Failed);
    }

    if args.len() == 2 && args[0] == "boot" && args[1] == "status" {
        return boot_status(&paths).map_err(CliError::Failed);
    }

    if args.len() == 2 && args[0] == "boot" && args[1] == "refresh" {
        return boot_cmd::boot_refresh(&paths).map_err(CliError::Failed);
    }

    if args[0] == "fah" {
        if args.len() < 2 {
            return Err(CliError::Usage);
        }
        let subcommand = args[1].clone();
        let extra = args[2..].to_vec();
        return fah::run(&paths, &subcommand, &extra).map_err(CliError::Failed);
    }

    if args[0] == "foldops" || args[0] == "tools" {
        if args.len() < 2 {
            return Err(CliError::Usage);
        }
        let group = args[0].as_str();
        let subcommand = args[1].clone();
        let extra = args[2..].to_vec();
        if subcommand == "acquire" {
            if !extra.is_empty() {
                return Err(CliError::Failed(format!("unknown {group} option {:?}", extra[0])));
            }
            let command = format_automation_command(&[group, "acquire"]);
            let ctx = AutomationContext::new(format, command);
            return match if group == "foldops" {
                foldops::acquire_json(&paths)
            } else {
                tools::acquire_json(&paths)
            } {
                Ok(data) => publish_success(&ctx, data, &subcommand),
                Err(message) => publish_failure(&ctx, message),
            };
        }
        return match group {
            "foldops" => foldops::run(&paths, &subcommand, &extra).map_err(CliError::Failed),
            "tools" => tools::run(&paths, &subcommand, &extra).map_err(CliError::Failed),
            _ => Err(CliError::Usage),
        };
    }

    let command = infer_command_name(&args);
    let ctx = AutomationContext::new(format, command);
    publish_failure(
        &ctx,
        format!(
            "command {} is not implemented",
            ctx.command
        ),
    )
}

fn dispatch_json_group(
    format: OutputFormat,
    command_parts: &[&str],
    subcommand: String,
    extra: Vec<String>,
    run: fn(&AppliancePaths, &str, &[String]) -> Result<serde_json::Value, String>,
    paths: &AppliancePaths,
) -> Result<(), CliError> {
    if subcommand.is_empty() {
        return Err(CliError::Usage);
    }
    if !extra.is_empty() {
        return Err(CliError::Failed(format!("unknown inspect option {:?}", extra[0])));
    }
    let command = format_automation_command(command_parts);
    let ctx = AutomationContext::new(format, command);
    match run(paths, &subcommand, &extra) {
        Ok(data) => publish_success(&ctx, data, &subcommand),
        Err(message) => publish_failure(&ctx, message),
    }
}

fn publish_success(
    ctx: &AutomationContext,
    data: serde_json::Value,
    subcommand: &str,
) -> Result<(), CliError> {
    match ctx.format {
        OutputFormat::Json => {
            print!(
                "{}",
                write_success(ctx, data).map_err(|error| CliError::Failed(error.to_string()))?
            );
            Ok(())
        }
        OutputFormat::Human => {
            print_human_summary(&data, subcommand);
            Ok(())
        }
    }
}

fn publish_failure(ctx: &AutomationContext, message: String) -> Result<(), CliError> {
    if ctx.format == OutputFormat::Json {
        print!("{}", write_failure(ctx, message));
        return Err(CliError::AlreadyReported);
    }
    Err(CliError::Failed(message))
}

fn print_migration_status(format: OutputFormat) -> Result<(), CliError> {
    match format {
        OutputFormat::Human => {
            println!("foldingosctl Rust migration: complete");
            println!("marker: {MIGRATION_MARKER}");
            Ok(())
        }
        OutputFormat::Json => {
            let ctx = AutomationContext::new(OutputFormat::Json, "migration status");
            let data = serde_json::json!({
                "phase": 6,
                "complete": true,
                "marker": MIGRATION_MARKER,
                "implementation": "rust",
            });
            publish_success(&ctx, data, "status")
        }
    }
}

fn infer_command_name(args: &[String]) -> String {
    match args {
        [group, command, ..] => format_automation_command(&[group.as_str(), command.as_str()]),
        [command] => command.clone(),
        _ => "foldingosctl".into(),
    }
}

fn print_human_summary(data: &serde_json::Value, subcommand: &str) {
    if let Some(node_id) = data.get("node_id").and_then(|value| value.as_str()) {
        println!(
            "node_id={} hostname={} role={} foldingos_version={} kernel={}",
            node_id,
            data.get("hostname")
                .and_then(|value| value.as_str())
                .unwrap_or("-"),
            data.get("installation_role")
                .and_then(|value| value.as_str())
                .unwrap_or("-"),
            data.get("foldingos_version")
                .and_then(|value| value.as_str())
                .unwrap_or("-"),
            data.get("kernel_version")
                .and_then(|value| value.as_str())
                .unwrap_or("-"),
        );
        if let Some(address) = data.get("primary_ipv4").and_then(|value| value.as_str()) {
            println!("primary_ipv4={address}");
        }
        if let Some(macs) = data.get("mac_addresses").and_then(|value| value.as_array()) {
            let joined = macs
                .iter()
                .filter_map(|value| value.as_str())
                .collect::<Vec<_>>()
                .join(",");
            println!("mac_addresses={joined}");
        }
        return;
    }
    if subcommand == "assign" {
        if let Some(count) = data.get("updated_count").and_then(|value| value.as_i64()) {
            println!("Assigned updates to {count} enrolled agent(s).");
            return;
        }
    }
    if subcommand == "allow-boot" {
        if let Some(mac) = data.get("mac_address").and_then(|value| value.as_str()) {
            println!("Allowed MAC {mac} for network boot.");
            return;
        }
    }
    if subcommand == "validate" || subcommand == "effective" {
        if data.get("valid").and_then(|value| value.as_bool()) == Some(true) {
            return;
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(data).unwrap_or_else(|_| "{}".into())
    );
}

pub fn print_human_error(error: &CliError) {
    if matches!(error, CliError::AlreadyReported | CliError::Exit(_)) {
        return;
    }
    eprintln!("foldingosctl: {}", error.message());
}

pub fn exit_code_for_error(error: &CliError) -> i32 {
    match error {
        CliError::Usage => 2,
        CliError::Exit(code) => *code,
        _ => 1,
    }
}
