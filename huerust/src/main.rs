mod security; mod clients; mod providers; mod models; mod infra; mod filters; mod routes; mod microservice; mod config; mod proxy; mod user_config; mod openapi; mod constants; mod app; mod http_client;

use dotenv::dotenv;
use tracing::info;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    // Try loading .env from multiple locations so it works regardless of CWD
    let dotenv_paths = ["../.env", ".env"];
    for path in &dotenv_paths {
        if std::path::Path::new(path).exists() {
            if dotenv::from_path(path).is_ok() {
                break;
            }
        }
    }
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "--encrypt" {
        if args.len() < 3 { println!("Usage: huerust --encrypt <url> [referer]"); return; }
        let cfg = config::AppConfig::from_env();
        let mut headers = std::collections::HashMap::new();
        if let Some(r) = args.get(3).cloned() { headers.insert("Referer".to_string(), r); }
        println!("{}", security::encrypt_url(&args[2], &cfg.secret, if headers.is_empty() { None } else { Some(headers) }));
        return;
    }
    tracing_subscriber::fmt::init();
    let cfg = config::AppConfig::from_env();
    if let Err(err) = cfg.validate_runtime() {
        eprintln!("Configuration error: {err}");
        std::process::exit(1);
    }
    let state = app::build_state(&cfg).await;
    app::spawn_background_tasks(&state);
    let router = app::build_router(state);
    let addr: SocketAddr = format!("{}:{}", cfg.host, cfg.port).parse().expect("Invalid RUST_HOST or RUST_PORT");
    info!("Huerust Engine Active on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    info!("Huerust Engine listening on {}", addr);
    axum::serve(listener, router).with_graceful_shutdown(shutdown_signal()).await.unwrap();
    info!("Huerust Engine shutdown complete");
}

async fn shutdown_signal() {
    let ctrl_c = async { tokio::signal::ctrl_c().await.expect("failed to install Ctrl+C handler"); };
    #[cfg(unix)]
    let terminate = async { tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("failed to install SIGTERM handler").recv().await; };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = terminate => {}, }
    info!("Shutdown signal received, starting graceful shutdown...");
}
