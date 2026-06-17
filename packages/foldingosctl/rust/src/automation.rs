use serde::Serialize;
use serde_json::Value;

pub const SCHEMA_VERSION: i32 = 1;
pub const MIGRATION_MARKER: &str = "FOLDINGOSCTL_RUST_PHASE_5";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Clone)]
pub struct AutomationContext {
    pub format: OutputFormat,
    pub command: String,
}

impl AutomationContext {
    pub fn new(format: OutputFormat, command: impl Into<String>) -> Self {
        Self {
            format,
            command: command.into(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AutomationErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct AutomationSuccessDocument {
    pub schema_version: i32,
    pub ok: bool,
    pub command: String,
    pub data: Value,
}

#[derive(Debug, Serialize)]
pub struct AutomationFailureDocument {
    pub schema_version: i32,
    pub ok: bool,
    pub command: String,
    pub error: AutomationErrorBody,
}

pub fn write_success(ctx: &AutomationContext, data: Value) -> Result<String, serde_json::Error> {
    let document = AutomationSuccessDocument {
        schema_version: SCHEMA_VERSION,
        ok: true,
        command: ctx.command.clone(),
        data,
    };
    let mut content = serde_json::to_string_pretty(&document)?;
    content.push('\n');
    Ok(content)
}

pub fn write_failure(ctx: &AutomationContext, message: impl Into<String>) -> String {
    let message = message.into();
    let document = AutomationFailureDocument {
        schema_version: SCHEMA_VERSION,
        ok: false,
        command: ctx.command.clone(),
        error: classify_automation_error(&message),
    };
    let mut content =
        serde_json::to_string_pretty(&document).expect("automation failure document serializes");
    content.push('\n');
    content
}

pub fn classify_automation_error(message: &str) -> AutomationErrorBody {
    let lower = message.to_ascii_lowercase();
    let code = if lower.contains("requires supervisor role")
        || lower.contains("requires agent role")
        || lower.contains("requires agent or supervisor role")
    {
        "role_required"
    } else if lower.contains("automation policy") {
        "automation_denied"
    } else if lower.contains("permission denied") {
        "permission_denied"
    } else if lower.contains("unknown configuration domain")
        || lower.contains("unknown inspect subcommand")
        || lower.contains("missing value for")
    {
        "invalid_input"
    } else if lower.contains("not registered")
        || lower.contains("not in registry")
        || lower.contains("not implemented")
    {
        "not_found"
    } else {
        "internal"
    };

    AutomationErrorBody {
        code: code.into(),
        message: message.to_string(),
    }
}

pub fn strip_format_flag(args: &[String]) -> (Vec<String>, OutputFormat) {
    let mut format = OutputFormat::Human;
    let mut clean = Vec::new();
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--format" {
            if index + 1 >= args.len() {
                clean.push("--format".to_string());
                index += 1;
                continue;
            }
            match args[index + 1].as_str() {
                "json" => format = OutputFormat::Json,
                other => {
                    clean.push("--format".to_string());
                    clean.push(other.to_string());
                }
            }
            index += 2;
            continue;
        }
        clean.push(args[index].clone());
        index += 1;
    }
    (clean, format)
}

pub fn format_automation_command(parts: &[&str]) -> String {
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::{
        classify_automation_error, strip_format_flag, write_failure, write_success,
        AutomationContext, OutputFormat, SCHEMA_VERSION,
    };

    #[test]
    fn strip_format_flag_extracts_json() {
        let args = vec![
            "inspect".into(),
            "node".into(),
            "--format".into(),
            "json".into(),
        ];
        let (clean, format) = strip_format_flag(&args);
        assert_eq!(format, OutputFormat::Json);
        assert_eq!(clean, vec!["inspect".to_string(), "node".to_string()]);
    }

    #[test]
    fn success_envelope_matches_contract() {
        let ctx = AutomationContext::new(OutputFormat::Json, "inspect node");
        let content = write_success(&ctx, serde_json::json!({ "node_id": "test" }))
            .expect("serialize success");
        let document: serde_json::Value = serde_json::from_str(&content).expect("parse json");
        assert_eq!(document["schema_version"], SCHEMA_VERSION);
        assert_eq!(document["ok"], true);
        assert_eq!(document["command"], "inspect node");
        assert_eq!(document["data"]["node_id"], "test");
    }

    #[test]
    fn failure_envelope_classifies_not_implemented_as_not_found() {
        let ctx = AutomationContext::new(OutputFormat::Json, "inspect node");
        let content = write_failure(&ctx, "command inspect node is not implemented");
        let document: serde_json::Value = serde_json::from_str(&content).expect("parse json");
        assert_eq!(document["ok"], false);
        assert_eq!(document["error"]["code"], "not_found");
    }

    #[test]
    fn classifies_role_required() {
        let body = classify_automation_error("operation requires supervisor role, found \"agent\"");
        assert_eq!(body.code, "role_required");
    }

    #[test]
    fn classifies_automation_denied() {
        let body = classify_automation_error(
            "automation policy does not authorize provision assign for the foldops user",
        );
        assert_eq!(body.code, "automation_denied");
    }
}
