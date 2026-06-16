mod agent;
mod alerts;
mod api;
mod config;
mod db;
mod deploy;
mod fah_projects;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use tower_http::services::{ServeDir, ServeFile};

use api::{router as api_router, spawn_alert_eval, AppState};
use config::Config;
use db::Db;

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

    let db = match Db::open(&config.db_path) {
        Ok(db) => Arc::new(db),
        Err(e) => {
            eprintln!("Failed to open database: {e}");
            std::process::exit(1);
        }
    };

    let state = AppState {
        db: db.clone(),
        config: config.clone(),
    };

    let web_root = config.web_root.clone();
    let index_path = web_root.join("index.html");

    let mut app = Router::new().nest("/api", api_router(state.clone()));

    if web_root.is_dir() {
        app = app.fallback_service(
            ServeDir::new(&web_root).not_found_service(ServeFile::new(&index_path)),
        );
    } else {
        tracing::warn!(path = %web_root.display(), "WEB_ROOT not found — dashboard static files unavailable");
    }

    spawn_alert_eval(state.clone());
    let alert_db = db.clone();
    let alert_config = config.alert_config.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            alerts::engine::run_alert_evaluation(&alert_db, &alert_config).await;
        }
    });

    let addr: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .expect("invalid HOST:PORT");

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        addr = %addr,
        db = %config.db_path.display(),
        "FoldOps supervisor listening"
    );
    if config.alert_config.enabled {
        tracing::info!(
            webhook = config.alert_config.webhook_url.is_some(),
            "[alerts] enabled"
        );
    }
    if config.deploy_enabled {
        tracing::info!(port = config.agent_http_port, "[deploy] agent push enabled");
    }
    if config.control_enabled {
        tracing::info!(port = config.agent_http_port, "[control] remote node control enabled");
    }

    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind failed");
    axum::serve(listener, app).await.expect("server error");
}
