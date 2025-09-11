// src/broker/paper.rs
// Branch: 9.2.25.1

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};
use chrono::DateTime;
use uuid::Uuid;
use tokio::sync::RwLock;

use crate::types::{Position, PositionSide, PositionStatus};
use crate::database::Database;
use crate::data::DataModule;
use sqlx::Row;

/// Paper trading configuration loaded from environment
#[derive(Debug, Clone)]
pub struct PaperTradingConfig {
    pub initial_cash: f64,
    pub commission_per_share: f64,
    pub enable_commission: bool,
    pub session_id: String,
}

impl Default for PaperTradingConfig {
    fn default() -> Self {
        Self {
            initial_cash: 100000.0,
            commission_per_share: 0.005,
            enable_commission: true,
            session_id: "default-session".to_string(),
        }
    }
}

/// Paper trade record for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperTrade {
    pub id: String,
    pub session_id: String,
    pub symbol: String,
    pub side: String,
    pub quantity: i32,
    pub entry_price: f64,
    pub exit_price: Option<f64>,
    pub current_price: Option<f64>,
    pub commission: f64,
    pub pnl: Option<f64>,
    pub pnl_percent: Option<f64>,
    pub opened_at: i64,
    pub closed_at: Option<i64>,
    pub rule_id: Option<String>,
    pub rule_name: Option<String>,
    pub rule_trigger: Option<String>,
    pub stop_loss_price: Option<f64>,
    pub take_profit_price: Option<f64>,
    pub status: String,
    pub notes: Option<String>,
}

/// Paper trading session status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperSession {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub initial_cash: f64,
    pub current_cash: f64,
    pub total_pnl: f64,
    pub realized_pnl: f64,
    pub unrealized_pnl: f64,
    pub open_positions: i32,
    pub total_trades: i32,
    pub winning_trades: i32,
    pub losing_trades: i32,
    pub commission_paid: f64,
    pub max_drawdown: f64,
    pub created_at: i64,
    pub updated_at: i64,
    pub status: String,
}

/// Paper broker for simulated trading with real market data
#[derive(Debug)]
pub struct PaperBroker {
    config: PaperTradingConfig,
    database: Arc<Database>,
    data_module: Arc<DataModule>,
    open_positions: Arc<RwLock<HashMap<String, Position>>>,
    session: Arc<RwLock<PaperSession>>,
}

impl PaperBroker {
    /// Create new paper broker with configuration
    pub async fn new(
        config: PaperTradingConfig,
        database: Arc<Database>,
        data_module: Arc<DataModule>,
    ) -> Result<Self> {
        // Load or create paper trading session
        let session = Self::load_or_create_session(&config, &database).await?;
        
        // Load existing open positions
        let open_positions = Self::load_open_positions(&config.session_id, &database).await?;
        
        info!("📊 Paper broker initialized for session '{}' with ${:.2} cash", 
            session.name, session.current_cash);
        
        Ok(Self {
            config,
            database,
            data_module,
            open_positions: Arc::new(RwLock::new(open_positions)),
            session: Arc::new(RwLock::new(session)),
        })
    }

    /// Load existing session or create new one
    async fn load_or_create_session(config: &PaperTradingConfig, db: &Database) -> Result<PaperSession> {
        // Try to load existing session using named field access to avoid tuple size limits
        let session_query = sqlx::query(
            "SELECT id, name, description, initial_cash, current_cash, total_pnl, realized_pnl, 
                    unrealized_pnl, open_positions, total_trades, winning_trades, losing_trades, 
                    commission_paid, max_drawdown, created_at, updated_at, status 
             FROM paper_sessions WHERE id = ?"
        )
        .bind(&config.session_id)
        .fetch_optional(db)
        .await?;
        
        match session_query {
            Some(row) => {
                let name: String = row.try_get("name")?;
                info!("📊 Loaded existing paper trading session: {}", name);
                Ok(PaperSession {
                    id: row.try_get("id")?,
                    name,
                    description: row.try_get("description")?,
                    initial_cash: row.try_get("initial_cash")?,
                    current_cash: row.try_get("current_cash")?,
                    total_pnl: row.try_get("total_pnl")?,
                    realized_pnl: row.try_get("realized_pnl")?,
                    unrealized_pnl: row.try_get("unrealized_pnl")?,
                    open_positions: row.try_get("open_positions")?,
                    total_trades: row.try_get("total_trades")?,
                    winning_trades: row.try_get("winning_trades")?,
                    losing_trades: row.try_get("losing_trades")?,
                    commission_paid: row.try_get("commission_paid")?,
                    max_drawdown: row.try_get("max_drawdown")?,
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                    status: row.try_get("status")?,
                })
            }
            None => {
                // Create new session
                let now = chrono::Utc::now().timestamp();
                let session = PaperSession {
                    id: config.session_id.clone(),
                    name: format!("Paper Trading Session {}", 
                        chrono::Utc::now().format("%Y-%m-%d %H:%M")),
                    description: Some("Auto-created paper trading session".to_string()),
                    initial_cash: config.initial_cash,
                    current_cash: config.initial_cash,
                    total_pnl: 0.0,
                    realized_pnl: 0.0,
                    unrealized_pnl: 0.0,
                    open_positions: 0,
                    total_trades: 0,
                    winning_trades: 0,
                    losing_trades: 0,
                    commission_paid: 0.0,
                    max_drawdown: 0.0,
                    created_at: now,
                    updated_at: now,
                    status: "ACTIVE".to_string(),
                };

                // Insert into database
                sqlx::query(
                    "INSERT INTO paper_sessions 
                     (id, name, description, initial_cash, current_cash, created_at, updated_at, status) 
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
                )
                .bind(&session.id)
                .bind(&session.name)
                .bind(&session.description)
                .bind(session.initial_cash)
                .bind(session.current_cash)
                .bind(session.created_at)
                .bind(session.updated_at)
                .bind(&session.status)
                .execute(db)
                .await?;

                info!("📊 Created new paper trading session: {}", session.name);
                Ok(session)
            }
        }
    }

    /// Load existing open positions from database
    async fn load_open_positions(session_id: &str, db: &Database) -> Result<HashMap<String, Position>> {
        let mut positions = HashMap::new();
        
        let rows = sqlx::query_as::<_, (String, String, i32, f64, Option<f64>, f64, i64)>(
            "SELECT id, symbol, quantity, entry_price, current_price, commission, opened_at
             FROM paper_trades WHERE session_id = ? AND status = 'OPEN'"
        )
        .bind(session_id)
        .fetch_all(db)
        .await?;

        for row in rows {
            let position = Position {
                id: row.0.clone(),
                symbol: row.1.clone(),
                side: if row.2 > 0 { PositionSide::Long } else { PositionSide::Short },
                quantity: row.2.abs() as u64,
                entry_price: row.3,
                current_price: row.4.unwrap_or(row.3),
                status: PositionStatus::Open,
                opened_at: DateTime::from_timestamp(row.6, 0).unwrap_or_default(),
                closed_at: None,
            };
            positions.insert(row.1, position);
        }

        if !positions.is_empty() {
            info!("📊 Loaded {} existing positions from paper trading session", positions.len());
        }

        Ok(positions)
    }

    /// Execute a simulated trade with instant fill at current market price
    pub async fn execute_trade(
        &self,
        symbol: &str,
        side: PositionSide,
        quantity: u32,
        rule_id: Option<String>,
        rule_name: Option<String>,
    ) -> Result<String> {
        // Get current market price from real data
        let current_price = self.get_current_price(symbol).await?;
        
        // Calculate commission
        let commission = if self.config.enable_commission {
            quantity as f64 * self.config.commission_per_share
        } else {
            0.0
        };

        // Calculate total trade value
        let trade_value = current_price * quantity as f64;
        let total_cost = trade_value + commission;

        // Check if we have enough cash for buy orders
        let session = self.session.read().await;
        if side == PositionSide::Long && session.current_cash < total_cost {
            return Err(anyhow!(
                "Insufficient cash for trade: Required ${:.2}, Available ${:.2}",
                total_cost, session.current_cash
            ));
        }
        drop(session);

        // Create trade record
        let trade_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();

        let paper_trade = PaperTrade {
            id: trade_id.clone(),
            session_id: self.config.session_id.clone(),
            symbol: symbol.to_string(),
            side: format!("{:?}", side).to_uppercase(),
            quantity: if side == PositionSide::Long { quantity as i32 } else { -(quantity as i32) },
            entry_price: current_price,
            exit_price: None,
            current_price: Some(current_price),
            commission,
            pnl: None,
            pnl_percent: None,
            opened_at: now,
            closed_at: None,
            rule_id: rule_id.clone(),
            rule_name: rule_name.clone(),
            rule_trigger: None,
            stop_loss_price: None,
            take_profit_price: None,
            status: "OPEN".to_string(),
            notes: Some(format!("Paper trade executed via rules engine")),
        };

        // Store in database
        self.store_paper_trade(&paper_trade).await?;

        // Create position object
        let position = Position {
            id: trade_id.clone(),
            symbol: symbol.to_string(),
            side,
            quantity: quantity as u64,
            entry_price: current_price,
            current_price,
            status: PositionStatus::Open,
            opened_at: DateTime::from_timestamp(now, 0).unwrap_or_default(),
            closed_at: None,
        };

        // Update in-memory positions
        {
            let mut positions = self.open_positions.write().await;
            positions.insert(symbol.to_string(), position);
        }

        // Update session cash and statistics
        self.update_session_after_trade(side, trade_value, commission).await?;

        info!("📊 Paper trade executed: {} {} shares of {} at ${:.2} (Commission: ${:.2})", 
            format!("{:?}", side).to_uppercase(), quantity, symbol, current_price, commission);

        Ok(trade_id)
    }

    /// Close a position (simulate selling)
    pub async fn close_position(&self, symbol: &str, _rule_id: Option<String>) -> Result<f64> {
        let current_price = self.get_current_price(symbol).await?;
        
        // Get position
        let mut positions = self.open_positions.write().await;
        let position = positions.remove(symbol)
            .ok_or_else(|| anyhow!("No open position found for {}", symbol))?;

        // Calculate P&L
        let pnl = match position.side {
            PositionSide::Long => (current_price - position.entry_price) * position.quantity as f64,
            PositionSide::Short => (position.entry_price - current_price) * position.quantity as f64,
        };

        let pnl_percent = (pnl / (position.entry_price * position.quantity as f64)) * 100.0;

        // Calculate commission on close
        let close_commission = if self.config.enable_commission {
            position.quantity as f64 * self.config.commission_per_share
        } else {
            0.0
        };

        let net_pnl = pnl - close_commission; // Commission tracking handled separately in paper trades table

        // Update trade record in database
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "UPDATE paper_trades SET 
                exit_price = ?, current_price = ?, pnl = ?, pnl_percent = ?, 
                closed_at = ?, status = 'CLOSED',
                commission = commission + ?
             WHERE id = ?"
        )
        .bind(current_price)
        .bind(current_price)
        .bind(net_pnl)
        .bind(pnl_percent)
        .bind(now)
        .bind(close_commission)
        .bind(&position.id)
        .execute(&*self.database)
        .await?;

        // Update session statistics
        self.update_session_after_close(net_pnl, close_commission).await?;

        info!("📊 Paper position closed: {} {} shares at ${:.2}, P&L: ${:.2} ({:.1}%)", 
            symbol, position.quantity, current_price, net_pnl, pnl_percent);

        Ok(net_pnl)
    }

    /// Get current positions
    pub async fn get_positions(&self) -> Result<HashMap<String, Position>> {
        let positions = self.open_positions.read().await;
        Ok(positions.clone())
    }

    /// Get current session status
    pub async fn get_session_status(&self) -> Result<PaperSession> {
        let session = self.session.read().await;
        Ok(session.clone())
    }

    /// Get current market price for a symbol
    async fn get_current_price(&self, symbol: &str) -> Result<f64> {
        // Try to get real-time price from WebSocket cache first
        if let Ok(Some(realtime_data)) = self.data_module.get_realtime_data(symbol).await {
            // Use the built-in current price method which handles bid/ask or last_price
            return Ok(realtime_data.get_current_price());
        }
        
        // Fallback to previous day close price using correct method
        match self.data_module.get_previous_day_cached(symbol).await {
            Ok(close_data) => Ok(close_data.close.unwrap_or(100.0)),
            Err(e) => {
                warn!("Could not get price for {}: {}, using fallback", symbol, e);
                Ok(100.0) // Fallback price from environment
            }
        }
    }

    /// Store paper trade in database
    async fn store_paper_trade(&self, trade: &PaperTrade) -> Result<()> {
        sqlx::query(
            "INSERT INTO paper_trades 
             (id, session_id, symbol, side, quantity, entry_price, current_price, commission, 
              opened_at, rule_id, rule_name, status, notes)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&trade.id)
        .bind(&trade.session_id)
        .bind(&trade.symbol)
        .bind(&trade.side)
        .bind(trade.quantity)
        .bind(trade.entry_price)
        .bind(trade.current_price)
        .bind(trade.commission)
        .bind(trade.opened_at)
        .bind(&trade.rule_id)
        .bind(&trade.rule_name)
        .bind(&trade.status)
        .bind(&trade.notes)
        .execute(&*self.database)
        .await?;

        Ok(())
    }

    /// Update session after opening a trade
    async fn update_session_after_trade(&self, side: PositionSide, trade_value: f64, commission: f64) -> Result<()> {
        let mut session = self.session.write().await;
        
        // Update cash (subtract for buys, add for sells)
        match side {
            PositionSide::Long => {
                session.current_cash -= trade_value + commission;
            }
            PositionSide::Short => {
                session.current_cash += trade_value - commission;
            }
        }

        session.open_positions += 1;
        session.total_trades += 1;
        session.commission_paid += commission;
        session.updated_at = chrono::Utc::now().timestamp();

        // Update database
        sqlx::query(
            "UPDATE paper_sessions SET 
                current_cash = ?, open_positions = ?, total_trades = ?, 
                commission_paid = ?, updated_at = ?
             WHERE id = ?"
        )
        .bind(session.current_cash)
        .bind(session.open_positions)
        .bind(session.total_trades)
        .bind(session.commission_paid)
        .bind(session.updated_at)
        .bind(&session.id)
        .execute(&*self.database)
        .await?;

        Ok(())
    }

    /// Update session after closing a position
    async fn update_session_after_close(&self, net_pnl: f64, close_commission: f64) -> Result<()> {
        let mut session = self.session.write().await;
        
        session.current_cash += net_pnl; // Net P&L already includes commissions
        session.open_positions -= 1;
        session.realized_pnl += net_pnl;
        session.total_pnl = session.realized_pnl + session.unrealized_pnl;
        session.commission_paid += close_commission;
        
        if net_pnl > 0.0 {
            session.winning_trades += 1;
        } else {
            session.losing_trades += 1;
        }

        session.updated_at = chrono::Utc::now().timestamp();

        // Update database
        sqlx::query(
            "UPDATE paper_sessions SET 
                current_cash = ?, open_positions = ?, realized_pnl = ?, total_pnl = ?,
                commission_paid = ?, winning_trades = ?, losing_trades = ?, updated_at = ?
             WHERE id = ?"
        )
        .bind(session.current_cash)
        .bind(session.open_positions)
        .bind(session.realized_pnl)
        .bind(session.total_pnl)
        .bind(session.commission_paid)
        .bind(session.winning_trades)
        .bind(session.losing_trades)
        .bind(session.updated_at)
        .bind(&session.id)
        .execute(&*self.database)
        .await?;

        Ok(())
    }

    /// Calculate unrealized P&L for all open positions
    pub async fn calculate_unrealized_pnl(&self) -> Result<f64> {
        let positions = self.open_positions.read().await;
        let mut total_unrealized = 0.0;

        for (symbol, position) in positions.iter() {
            match self.get_current_price(symbol).await {
                Ok(current_price) => {
                    let pnl = match position.side {
                        PositionSide::Long => (current_price - position.entry_price) * position.quantity as f64,
                        PositionSide::Short => (position.entry_price - current_price) * position.quantity as f64,
                    };
                    total_unrealized += pnl;
                }
                Err(e) => {
                    warn!("Could not get current price for {}: {}", symbol, e);
                }
            }
        }

        // Update session unrealized P&L
        {
            let mut session = self.session.write().await;
            session.unrealized_pnl = total_unrealized;
            session.total_pnl = session.realized_pnl + session.unrealized_pnl;
        }

        Ok(total_unrealized)
    }

    /// Check if broker is in paper trading mode
    pub fn is_paper_mode(&self) -> bool {
        true // Always true for PaperBroker
    }
}