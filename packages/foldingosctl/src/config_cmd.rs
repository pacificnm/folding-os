use crate::automation::OutputFormat;
use crate::config;
use crate::paths::AppliancePaths;

#[derive(Debug)]
pub enum ConfigCommandOutput {
    Json(serde_json::Value),
    EffectiveHuman(String),
    Silent,
}

pub fn run(
    paths: &AppliancePaths,
    subcommand: &str,
    args: &[String],
    format: OutputFormat,
) -> Result<ConfigCommandOutput, String> {
    match subcommand {
        "validate" => {
            let domain = args
                .first()
                .ok_or_else(|| "config validate requires a domain".to_string())?;
            if args.len() > 1 {
                return Err(format!("unknown config option {:?}", args[1]));
            }
            let data = config::validate_config(paths, domain)?;
            if format == OutputFormat::Json {
                return Ok(ConfigCommandOutput::Json(data));
            }
            if domain == "--all" {
                let valid = data["valid"].as_bool().unwrap_or(false);
                if !valid {
                    return Err("one or more configuration domains are invalid".into());
                }
            }
            Ok(ConfigCommandOutput::Silent)
        }
        "effective" => {
            let domain = args
                .first()
                .ok_or_else(|| "config effective requires a domain".to_string())?;
            if args.len() > 1 {
                return Err(format!("unknown config option {:?}", args[1]));
            }
            let (data, content) =
                config::print_effective_config(paths, domain, format != OutputFormat::Json)?;
            if format == OutputFormat::Json {
                Ok(ConfigCommandOutput::Json(data))
            } else {
                Ok(ConfigCommandOutput::EffectiveHuman(
                    content.unwrap_or_default(),
                ))
            }
        }
        "activate" => {
            if args.len() < 2 {
                return Err("config activate requires a domain and candidate path".into());
            }
            if args.len() > 2 {
                return Err(format!("unknown config option {:?}", args[2]));
            }
            config::activate_config(paths, &args[0], &args[1])?;
            Ok(ConfigCommandOutput::Silent)
        }
        other => Err(format!("unknown config subcommand {other:?}")),
    }
}
