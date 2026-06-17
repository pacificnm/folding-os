use crate::automation::{
    format_automation_command, write_failure, write_success, AutomationContext, MIGRATION_MARKER,
    OutputFormat,
};
use crate::inspect;
use crate::paths::AppliancePaths;

const USAGE: &str =
    "usage: foldingosctl <boot|config|fah|foldops|identity|inspect|provision|registry|storage|tools> <command> [arguments]";

#[derive(Debug)]
pub enum CliError {
    Usage,
    Failed(String),
    AlreadyReported,
}

impl CliError {
    pub fn message(&self) -> String {
        match self {
            Self::Usage => USAGE.to_string(),
            Self::Failed(message) => message.clone(),
            Self::AlreadyReported => String::new(),
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

    if args[0] == "inspect" {
        if args.len() < 2 {
            return Err(CliError::Usage);
        }
        let subcommand = args[1].clone();
        let extra = args[2..].to_vec();
        let command = format_automation_command(&["inspect", &subcommand]);
        let ctx = AutomationContext::new(format, command);
        let paths = AppliancePaths::default();
        return match inspect::run(&paths, &subcommand, &extra) {
            Ok(data) => publish_success(&ctx, data),
            Err(message) => publish_failure(&ctx, message),
        };
    }

    let command = infer_command_name(&args);
    let ctx = AutomationContext::new(format, command);
    publish_failure(
        &ctx,
        format!("command {} is not implemented in the Rust foldingosctl migration yet", ctx.command),
    )
}

fn publish_success(ctx: &AutomationContext, data: serde_json::Value) -> Result<(), CliError> {
    match ctx.format {
        OutputFormat::Json => {
            print!(
                "{}",
                write_success(ctx, data).map_err(|error| CliError::Failed(error.to_string()))?
            );
            Ok(())
        }
        OutputFormat::Human => {
            print_human_inspect_summary(&data);
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
            println!("foldingosctl Rust migration: phase 2");
            println!("marker: {MIGRATION_MARKER}");
            Ok(())
        }
        OutputFormat::Json => {
            let ctx = AutomationContext::new(OutputFormat::Json, "migration status");
            let data = serde_json::json!({
                "phase": 2,
                "marker": MIGRATION_MARKER,
                "implementation": "rust",
            });
            publish_success(&ctx, data)
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

fn print_human_inspect_summary(data: &serde_json::Value) {
    if let Some(node_id) = data.get("node_id").and_then(|value| value.as_str()) {
        println!(
            "node_id={} hostname={} role={} foldingos_version={} kernel={}",
            node_id,
            data.get("hostname").and_then(|value| value.as_str()).unwrap_or("-"),
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
    println!("{}", serde_json::to_string_pretty(data).unwrap_or_else(|_| "{}".into()));
}

pub fn print_human_error(error: &CliError) {
    if matches!(error, CliError::AlreadyReported) {
        return;
    }
    eprintln!("foldingosctl: {}", error.message());
}

pub fn exit_code_for_error(error: &CliError) -> i32 {
    match error {
        CliError::Usage => 2,
        _ => 1,
    }
}
