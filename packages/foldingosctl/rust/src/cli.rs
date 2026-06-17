use crate::automation::{
    format_automation_command, write_failure, write_success, AutomationContext, MIGRATION_MARKER,
    OutputFormat,
};

const USAGE: &str =
    "usage: foldingosctl <boot|config|fah|foldops|identity|inspect|provision|registry|storage|tools> <command> [arguments]";

#[derive(Debug)]
pub enum CliError {
    Usage,
    NotImplemented { command: String },
    AlreadyReported,
}

impl CliError {
    pub fn message(&self) -> String {
        match self {
            Self::Usage => USAGE.to_string(),
            Self::NotImplemented { command } => format!(
                "command {command} is not implemented in the Rust foldingosctl migration yet"
            ),
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

    let command = infer_command_name(&args);
    let ctx = AutomationContext::new(format, command);
    let message = CliError::NotImplemented {
        command: ctx.command.clone(),
    }
    .message();

    if ctx.format == OutputFormat::Json {
        print!("{}", write_failure(&ctx, message));
        return Err(CliError::AlreadyReported);
    }

    Err(CliError::NotImplemented {
        command: ctx.command,
    })
}

fn print_migration_status(format: OutputFormat) -> Result<(), CliError> {
    match format {
        OutputFormat::Human => {
            println!("foldingosctl Rust migration: phase 1");
            println!("marker: {MIGRATION_MARKER}");
            Ok(())
        }
        OutputFormat::Json => {
            let ctx = AutomationContext::new(OutputFormat::Json, "migration status");
            let data = serde_json::json!({
                "phase": 1,
                "marker": MIGRATION_MARKER,
                "implementation": "rust",
            });
            print!(
                "{}",
                write_success(&ctx, data).map_err(|_| CliError::AlreadyReported)?
            );
            Ok(())
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
