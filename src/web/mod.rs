use anyhow::Result;
use axum::{routing::get, Router};
use std::sync::Arc;
use crate::engine::TradingEngine;

pub async fn start_server(bind_address: String, _engine: Arc<TradingEngine>) -> Result<()> {
    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health));

    let listener = tokio::net::TcpListener::bind(&bind_address).await?;
    
    tracing::info!("Web server listening on {}", bind_address);
    
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn root() -> &'static str {
    "Trading System Web Interface - Coming Soon"
}

async fn health() -> &'static str {
    "OK"
}