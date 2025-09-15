// src/broker/traits.rs
// Branch: 14.9.25

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use crate::types::Position;

/// Common trait for all broker implementations
#[async_trait]
pub trait BrokerTrait: Send + Sync {
    /// Get the broker name (e.g., "lightspeed", "paper", "interactive_brokers")
    fn broker_name(&self) -> &'static str;

    /// Check if broker is currently connected
    async fn is_connected(&self) -> bool;

    /// Connect to the broker
    async fn connect(&mut self) -> Result<()>;

    /// Disconnect from the broker
    async fn disconnect(&mut self) -> Result<()>;

    /// Get current positions from broker
    async fn get_positions(&self) -> Result<HashMap<String, Position>>;

    /// Place an order
    async fn place_order(
        &self,
        symbol: &str,
        side: &str,
        order_type: &str,
        quantity: u64,
        price: Option<f64>,
    ) -> Result<String>;

    /// Cancel an order
    async fn cancel_order(&self, order_id: &str) -> Result<()>;

    /// Calculate unrealized P&L for all positions
    async fn calculate_unrealized_pnl(&self) -> Result<f64>;

    /// Get account information (balance, buying power, etc.)
    async fn get_account_info(&self) -> Result<AccountInfo>;

    /// Record realized P&L (for live brokers)
    async fn record_realized_pnl(&self, symbol: &str, amount: f64, trade_id: &str) -> Result<()>;

    /// Sync positions with broker (for live brokers)
    async fn sync_positions(&self) -> Result<()>;
}

/// Account information structure
#[derive(Debug, Clone)]
pub struct AccountInfo {
    pub account_id: String,
    pub total_value: f64,
    pub available_cash: f64,
    pub buying_power: f64,
    pub day_trading_buying_power: Option<f64>,
    pub maintenance_margin: Option<f64>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Broker-specific position data
#[derive(Debug, Clone)]
pub struct BrokerPosition {
    pub broker_position_id: Option<String>,
    pub symbol: String,
    pub quantity: i64, // Can be negative for short positions
    pub market_value: f64,
    pub cost_basis: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Trade execution details from broker
#[derive(Debug, Clone)]
pub struct BrokerTrade {
    pub trade_id: String,
    pub order_id: Option<String>,
    pub symbol: String,
    pub side: String,
    pub quantity: u64,
    pub price: f64,
    pub commission: f64,
    pub fees: f64,
    pub execution_time: chrono::DateTime<chrono::Utc>,
    pub realized_pnl: Option<f64>,
}