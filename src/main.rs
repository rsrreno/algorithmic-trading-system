// src/main.rs
use anyhow::Result;
use std::sync::Arc;
use tokio::signal;
use tracing::{info, error};

mod config;
mod database;
mod engine;
mod data;
mod broker;
mod rules;
mod web;
mod types;
mod metrics;
mod market;

use config::Config;
use engine::TradingEngine;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing with file logging
    let log_dir = std::path::Path::new("/app/logs");
    let file_appender = tracing_appender::rolling::daily(log_dir, "trading-system.log");
    let (non_blocking_file, _guard) = tracing_appender::non_blocking(file_appender);
    
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};
    
    // Create a multi-writer that writes to both stdout and file
    let stdout_layer = fmt::layer().with_writer(std::io::stdout);
    let file_layer = fmt::layer().with_writer(non_blocking_file).with_ansi(false);
    
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(stdout_layer)
        .with(file_layer)
        .init();
    
    // Keep the guard alive for the duration of the program
    std::mem::forget(_guard);

    info!("Starting Algorithmic Trading System v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config = Config::load()?;
    info!("Configuration loaded successfully");

    // Initialize database
    let db = database::init(&config.database_url).await?;
    info!("Database initialized");

    // Run migrations
    database::migrate(&db).await?;
    info!("Database migrations completed");

    // Initialize metrics
    let _metrics_handle = metrics::init(&config.metrics_address)?;
    info!("Metrics server started on {}", config.metrics_address);

    // Create trading engine
    let mut engine = TradingEngine::new(config.clone(), Arc::new(db)).await?;
    info!("Trading engine initialized");

    // Start rules engine if enabled in configuration
    if config.is_rules_engine_enabled() {
        info!("🚀 Starting rules engine with initial cash: ${:.2}", config.paper_trading_config.initial_cash);
        if let Err(e) = engine.start_rules_engine(config.paper_trading_config.initial_cash).await {
            error!("Failed to start rules engine: {}", e);
        }
    } else {
        info!("📊 Rules engine disabled in configuration");
    }
    
    let engine = Arc::new(engine);

    // Start the trading engine
    let engine_handle = {
        let engine = Arc::clone(&engine);
        tokio::spawn(async move {
            if let Err(e) = engine.run().await {
                error!("Trading engine error: {}", e);
            }
        })
    };

    // Start web server for rules management
    let bind_address_clone = config.bind_address.clone();
    let metrics_address_clone = config.metrics_address.clone();
    let web_handle = {
        let engine = Arc::clone(&engine);
        tokio::spawn(async move {
            if let Err(e) = web::start_server(bind_address_clone, engine).await {
                error!("Web server error: {}", e);
            }
        })
    };

    info!("All services started successfully");
    info!("Web UI available at: http://{}", config.bind_address);
    info!("Metrics available at: http://{}", metrics_address_clone);

    // Wait for shutdown signal
    match signal::ctrl_c().await {
        Ok(()) => {
            info!("Received shutdown signal, stopping services...");
        }
        Err(err) => {
            error!("Unable to listen for shutdown signal: {}", err);
        }
    }

    // Graceful shutdown
    engine.shutdown().await?;
    
    // Cancel running tasks
    engine_handle.abort();
    web_handle.abort();
    
    // Wait a moment for cleanup
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    
    info!("Trading system shut down successfully");
    Ok(())
}