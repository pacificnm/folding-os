mod collector;
mod config;
mod fah;
mod http;
mod ingest;
mod log_tail;
mod node_control;
mod temperatures;
mod update;

use std::sync::Arc;
use std::time::Duration;

use config::Config;
use ingest::IngestClient;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = match Config::from_env() {
        Ok(c) => Arc::new(c),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    tracing::info!(
        supervisor = %config.supervisor_url,
        interval_ms = config.interval_ms,
        "FoldOps agent starting"
    );

    let config_http = config.clone();
    tokio::spawn(async move {
        http::start_agent_http(config_http).await;
    });

    let client = IngestClient::new(config.clone());
    client.probe_supervisor().await;

    let interval = Duration::from_millis(config.interval_ms);

    loop {
        if let Err(e) = client.collect_and_post().await {
            tracing::error!(error = %e, "ingest error");
        }
        tokio::time::sleep(interval).await;
    }
}
