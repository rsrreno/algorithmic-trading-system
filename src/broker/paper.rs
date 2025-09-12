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
        
        let rows = sqlx::query_as::<_, (String, String, String, i32, f64, Option<f64>, f64, i64)>(
            "SELECT id, symbol, side, quantity, entry_price, current_price, commission, opened_at
             FROM paper_trades WHERE session_id = ?"
        )
        .bind(session_id)
        .fetch_all(db)
        .await?;

        // Group trades by symbol and consolidate positions
        let mut symbol_trades: HashMap<String, Vec<(String, String, i32, f64, Option<f64>, i64)>> = HashMap::new();
        
        for row in rows {
            let symbol = row.1.clone();
            let trade_data = (row.0.clone(), row.2.clone(), row.3, row.4, row.5, row.7); // id, side, quantity, entry_price, current_price, opened_at
            symbol_trades.entry(symbol).or_insert_with(Vec::new).push(trade_data);
        }

        // Consolidate positions by symbol - simple sum calculation
        for (symbol, trades) in symbol_trades {
            let mut net_quantity = 0i64;
            let mut total_cost = 0.0;
            let mut total_shares_bought = 0i64;
            let mut realized_pnl = 0.0;
            
            let mut latest_opened_at = 0i64;
            let mut current_price = None;

            // Simple sum: BUY adds, SELL subtracts
            for (trade_id, side, quantity, entry_price, curr_price, opened_at) in &trades {
                latest_opened_at = latest_opened_at.max(*opened_at);
                if curr_price.is_some() {
                    current_price = *curr_price;
                }

                match side.as_str() {
                    "BUY" | "LONG" => {
                        // Add to position
                        net_quantity += *quantity as i64;
                        total_cost += (*quantity as f64) * entry_price;
                        total_shares_bought += *quantity as i64;
                    },
                    "SELL" => {
                        // Subtract from position
                        net_quantity -= *quantity as i64;
                        
                        // Calculate realized P&L: (sale_price - average_cost_basis) * quantity_sold
                        // For now, use a simple FIFO approach where we assume avg cost basis from buys
                        let current_avg_cost = if total_shares_bought > 0 {
                            total_cost / total_shares_bought as f64
                        } else {
                            *entry_price // fallback to sale price if no prior buys
                        };
                        realized_pnl += (*entry_price - current_avg_cost) * (*quantity as f64);
                    },
                    _ => {}
                }
            }

            // Skip if no net position
            if net_quantity <= 0 {
                continue;
            }

            // Calculate weighted average cost basis
            let cost_basis = if total_shares_bought > 0 {
                total_cost / total_shares_bought as f64
            } else {
                0.0
            };

            let final_quantity = net_quantity as u64;
            let final_side = PositionSide::Long;

            let position = Position {
                id: format!("consolidated_{}", symbol),
                symbol: symbol.clone(),
                quantity: final_quantity,
                avg_cost_basis: cost_basis, // Weighted average cost basis
                total_cost: final_quantity as f64 * cost_basis,
                current_price: current_price.unwrap_or(cost_basis),
                realized_pnl: realized_pnl, // P&L from sales
                opened_at: DateTime::from_timestamp(latest_opened_at, 0).unwrap_or_default(),
                closed_at: None,
                status: PositionStatus::Open,
                session_id: session_id.to_string(),
            };

            let side_str = match final_side {
                PositionSide::Long => "LONG",
                PositionSide::Short => "SHORT",
            };

            tracing::debug!("📊 Consolidated {} position for {}: {} shares @ ${:.2} avg (from {} trades)", 
                side_str, symbol, position.quantity, position.avg_cost_basis, trades.len());
            
            positions.insert(symbol, position);
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

        // Store trade in database
        self.store_paper_trade(&paper_trade).await?;

        // Process the position using new weighted average logic
        match side {
            PositionSide::Long => {
                self.process_buy_order(symbol, quantity as u64, current_price, trade_id.clone()).await?;
            }
            PositionSide::Short => {
                let realized_pnl = self.process_sell_order(symbol, quantity as u64, current_price).await?;
                info!("🎯 SELL order completed: {} shares of {} @ ${:.2}, Realized P&L: ${:.2}", 
                      quantity, symbol, current_price, realized_pnl);
            }
        }
        
        // Auto-subscribe to WebSocket for new position symbol
        if let Err(e) = self.data_module.websocket_subscribe(vec![symbol.to_string()]).await {
            warn!("Failed to subscribe to new position symbol {}: {}", symbol, e);
        } else {
            info!("📡 Auto-subscribed to new position symbol: {}", symbol);
        }

        // Update session cash and statistics
        self.update_session_after_trade(side, trade_value, commission).await?;

        info!("📊 Paper trade executed: {} {} shares of {} at ${:.2} (Commission: ${:.2})", 
            format!("{:?}", side).to_uppercase(), quantity, symbol, current_price, commission);

        // Refresh positions from database to reflect consolidated trades
        self.refresh_positions_from_db().await?;

        Ok(trade_id)
    }

    /// Execute a trade with proper SELL vs SELL_SHORT logic
    pub async fn execute_trade_with_side(
        &self,
        symbol: &str,
        side_str: &str, // "BUY", "SELL", or "SELL_SHORT"
        quantity: u32,
        rule_id: Option<String>,
        rule_name: Option<String>,
    ) -> Result<String> {
        let side_upper = side_str.to_uppercase();
        
        match side_upper.as_str() {
            "BUY" => {
                // Simple buy - create long position
                self.execute_trade(symbol, PositionSide::Long, quantity, rule_id, rule_name).await
            },
            "SELL" => {
                // Sell existing long shares - reduce or close long position
                self.execute_sell(symbol, quantity, rule_id, rule_name).await
            },
            "SELL_SHORT" => {
                // Create new short position (only if no long position exists)
                self.execute_short(symbol, quantity, rule_id, rule_name).await
            },
            _ => Err(anyhow::anyhow!("Invalid order side: {}. Valid sides: BUY, SELL, SELL_SHORT", side_str))
        }
    }

    /// Execute a sell order - reduces existing long position or rejects if insufficient shares
    async fn execute_sell(
        &self,
        symbol: &str,
        quantity: u32,
        rule_id: Option<String>,
        rule_name: Option<String>,
    ) -> Result<String> {
        // Get current market price
        let current_price = self.get_current_price(symbol).await?;
        
        // Process sell order directly with database positions
        let realized_pnl = self.process_database_sell_order(symbol, quantity as u64, current_price).await?;
        
        // Create trade record in paper_trades table for historical tracking
        let trade_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        let commission = if self.config.enable_commission {
            quantity as f64 * self.config.commission_per_share
        } else {
            0.0
        };
        
        let paper_trade = PaperTrade {
            id: trade_id.clone(),
            session_id: self.config.session_id.clone(),
            symbol: symbol.to_string(),
            side: "SELL".to_string(),
            quantity: quantity as i32,
            entry_price: current_price,
            exit_price: Some(current_price),
            current_price: Some(current_price),
            commission,
            pnl: Some(realized_pnl),
            pnl_percent: None,
            opened_at: now,
            closed_at: Some(now),
            rule_id: rule_id.clone(),
            rule_name: rule_name.clone(),
            rule_trigger: None,
            stop_loss_price: None,
            take_profit_price: None,
            status: "CLOSED".to_string(),
            notes: Some(format!("Sell order: Realized P&L ${:.2}", realized_pnl)),
        };

        // Store in database for historical tracking
        self.store_paper_trade(&paper_trade).await?;

        // Update session cash and P&L
        let trade_value = current_price * quantity as f64;
        let net_proceeds = trade_value - commission;
        
        {
            let mut session = self.session.write().await;
            session.current_cash += net_proceeds;
            session.realized_pnl += realized_pnl;
            session.total_trades += 1;
            session.updated_at = now;
        }

        info!("💰 Sold {} shares of {} @ ${:.2}. Realized P&L: ${:.2}, Net proceeds: ${:.2}", 
            quantity, symbol, current_price, realized_pnl, net_proceeds);

        Ok(trade_id)
    }

    /// Execute a short order - creates new short position (only if no long position exists)
    async fn execute_short(
        &self,
        symbol: &str,
        quantity: u32,
        rule_id: Option<String>,
        rule_name: Option<String>,
    ) -> Result<String> {
        // Check that no long position exists
        let positions = self.open_positions.read().await;
        let current_position = positions.get(symbol);
        
        if let Some(pos) = current_position {
            if pos.quantity > 0 { // All positions are treated as long in new model
                let quantity = pos.quantity; // Copy the quantity before dropping
                drop(positions);
                return Err(anyhow::anyhow!(
                    "Cannot SELL_SHORT {}: Long position of {} shares exists. Use SELL to reduce long position first.",
                    symbol, quantity
                ));
            }
        }
        drop(positions);

        // Execute as short position (negative quantity)
        self.execute_trade(symbol, PositionSide::Short, quantity, rule_id, rule_name).await
    }

    /// Close a position (simulate selling)
    pub async fn close_position(&self, symbol: &str, _rule_id: Option<String>) -> Result<f64> {
        let current_price = self.get_current_price(symbol).await?;
        
        // Get position
        let mut positions = self.open_positions.write().await;
        let position = positions.remove(symbol)
            .ok_or_else(|| anyhow!("No open position found for {}", symbol))?;

        // Calculate P&L (assuming long positions only in the new model)
        let pnl = (current_price - position.avg_cost_basis) * position.quantity as f64;

        let pnl_percent = (pnl / (position.avg_cost_basis * position.quantity as f64)) * 100.0;

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

        // Auto-unsubscribe from WebSocket since position is closed
        if let Err(e) = self.data_module.websocket_unsubscribe(vec![symbol.to_string()]).await {
            warn!("Failed to unsubscribe from closed position symbol {}: {}", symbol, e);
        } else {
            info!("📡 Auto-unsubscribed from closed position symbol: {}", symbol);
        }

        info!("📊 Paper position closed: {} {} shares at ${:.2}, P&L: ${:.2} ({:.1}%)", 
            symbol, position.quantity, current_price, net_pnl, pnl_percent);

        Ok(net_pnl)
    }

    /// Get realized P&L for a symbol from all closed trades
    async fn get_realized_pnl_for_symbol(&self, symbol: &str) -> Result<f64> {
        let rows = sqlx::query(
            "SELECT pnl FROM paper_trades 
             WHERE symbol = ? AND status = 'CLOSED' AND session_id = ? AND pnl IS NOT NULL"
        )
        .bind(symbol)
        .bind(&self.config.session_id)
        .fetch_all(&*self.database)
        .await?;

        let mut total_realized_pnl = 0.0;
        
        for row in rows {
            let trade_pnl: f64 = row.get("pnl");
            total_realized_pnl += trade_pnl;
        }
        
        // Round to nearest penny
        Ok((total_realized_pnl * 100.0).round() / 100.0)
    }

    /// Get positions closed today for display
    async fn get_positions_closed_today(&self) -> Result<HashMap<String, Position>> {
        let today_start = chrono::Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();
            
        let rows = sqlx::query(
            "SELECT symbol, SUM(quantity) as total_quantity, AVG(entry_price) as avg_entry_price, 
                    MAX(exit_price) as last_exit_price, SUM(pnl) as total_pnl, MAX(closed_at) as last_closed_at
             FROM paper_trades 
             WHERE status = 'CLOSED' AND session_id = ? AND closed_at >= ?
             GROUP BY symbol"
        )
        .bind(&self.config.session_id)
        .bind(today_start)
        .fetch_all(&*self.database)
        .await?;

        let mut closed_positions = HashMap::new();
        
        for row in rows {
            let symbol: String = row.get("symbol");
            let total_quantity: i64 = row.get("total_quantity");
            let avg_entry_price: f64 = row.get("avg_entry_price");
            let last_exit_price: Option<f64> = row.get("last_exit_price");
            let total_pnl: Option<f64> = row.get("total_pnl");
            let last_closed_at: i64 = row.get("last_closed_at");
            
            let position = Position {
                id: format!("closed_{}_{}", symbol, last_closed_at),
                symbol: symbol.clone(),
                quantity: 0, // Show as 0 since position is closed
                avg_cost_basis: avg_entry_price,
                total_cost: 0.0, // No cost for closed position
                current_price: last_exit_price.unwrap_or(avg_entry_price),
                realized_pnl: total_pnl.unwrap_or(0.0),
                opened_at: DateTime::from_timestamp(last_closed_at, 0).unwrap_or_default(),
                closed_at: Some(DateTime::from_timestamp(last_closed_at, 0).unwrap_or_default()),
                status: PositionStatus::Closed,
                session_id: self.config.session_id.clone(),
            };
            
            closed_positions.insert(symbol, position);
        }
        
        Ok(closed_positions)
    }

    /// Get current positions with updated market prices and auto-manage WebSocket subscriptions
    /// Now includes positions closed today for complete daily view
    pub async fn get_positions(&self) -> Result<HashMap<String, Position>> {
        // Load all positions from the positions table
        let rows = sqlx::query_as::<_, (String, String, i64, f64, f64, f64, f64, i64, Option<i64>, String, String)>(
            "SELECT id, symbol, quantity, avg_cost_basis, total_cost, current_price, realized_pnl, 
                    opened_at, closed_at, status, session_id 
             FROM positions 
             WHERE session_id = ? 
             AND (status = 'OPEN' OR (status = 'CLOSED' AND DATE(closed_at, 'unixepoch') = DATE('now')))"
        )
        .bind(&self.config.session_id)
        .fetch_all(&*self.database)
        .await?;

        let mut all_positions = HashMap::new();
        let mut open_symbols = Vec::new();
        
        for row in rows {
            let (id, symbol, quantity, avg_cost_basis, total_cost, current_price, realized_pnl, 
                 opened_at, closed_at, status, session_id) = row;

            // Create position from database record
            let position_status = status.parse::<PositionStatus>()
                .unwrap_or(PositionStatus::Open);
            
            let mut position = Position {
                id: id.clone(),
                symbol: symbol.clone(),
                quantity: quantity as u64,
                avg_cost_basis,
                total_cost,
                current_price,
                realized_pnl,
                opened_at: DateTime::from_timestamp(opened_at, 0).unwrap_or_default(),
                closed_at: closed_at.map(|ts| DateTime::from_timestamp(ts, 0).unwrap_or_default()),
                status: position_status.clone(),
                session_id,
            };

            // Track open positions for WebSocket subscription
            if position_status == PositionStatus::Open && position.quantity > 0 {
                open_symbols.push(symbol.clone());
                
                // Update current_price with live market data for open positions
                match self.get_current_price(&symbol).await {
                    Ok(live_price) => {
                        position.current_price = live_price;
                        
                        // Update price in database for future reference
                        let _ = sqlx::query("UPDATE positions SET current_price = ? WHERE id = ?")
                            .bind(live_price)
                            .bind(&position.id)
                            .execute(&*self.database)
                            .await;
                    }
                    Err(e) => {
                        warn!("Could not get current price for {}: {}, using stored price", symbol, e);
                    }
                }
            }

            all_positions.insert(id.clone(), position);
        }

        // Sync in-memory positions with database positions
        {
            let mut memory_positions = self.open_positions.write().await;
            memory_positions.clear();
            
            // Only add open positions to memory
            for (symbol, position) in &all_positions {
                if position.status == PositionStatus::Open && position.quantity > 0 {
                    memory_positions.insert(symbol.clone(), position.clone());
                }
            }
        }
        
        // Auto-manage WebSocket subscriptions for open position symbols
        if !open_symbols.is_empty() {
            if let Err(e) = self.data_module.websocket_subscribe(open_symbols.clone()).await {
                warn!("Failed to subscribe to position symbols {:?}: {}", open_symbols, e);
            } else {
                info!("📡 Auto-subscribed to {} position symbols: {:?}", open_symbols.len(), open_symbols);
            }
        }
        
        info!("📊 Loaded {} positions from database ({} open, {} closed today)", 
              all_positions.len(), open_symbols.len(), 
              all_positions.len() - open_symbols.len());
        
        Ok(all_positions)
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
        
        // Try to get current market price from snapshot API
        match self.data_module.get_current_market_price(symbol).await {
            Ok(current_price) => {
                info!("📊 Retrieved current market price for {}: ${:.2}", symbol, current_price);
                Ok(current_price)
            }
            Err(e) => {
                warn!("Could not get current price for {}: {}, falling back to previous day", symbol, e);
                // Fallback to previous day close price
                match self.data_module.get_previous_day_cached(symbol).await {
                    Ok(close_data) => Ok(close_data.close.unwrap_or(100.0)),
                    Err(e2) => {
                        warn!("Could not get fallback price for {}: {}, using default", symbol, e2);
                        Ok(100.0) // Final fallback price
                    }
                }
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
                    let pnl = (current_price - position.avg_cost_basis) * position.quantity as f64;
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

    /// Get complete portfolio snapshot with real-time position values
    pub async fn get_portfolio_snapshot(&self) -> Result<crate::types::PortfolioState> {
        // Get current session data
        let session = self.session.read().await;
        let positions_map = self.open_positions.read().await;
        
        // Calculate real-time position values
        let mut total_position_value = 0.0;
        let mut total_exposure = 0.0;
        let mut calculated_positions = std::collections::HashMap::new();
        
        for (symbol, position) in positions_map.iter() {
            match self.get_current_price(symbol).await {
                Ok(current_price) => {
                    let position_value = current_price * position.quantity as f64;
                    let unrealized_pnl = (current_price - position.avg_cost_basis) * position.quantity as f64;
                    
                    total_position_value += position_value;
                    total_exposure += position_value;
                    
                    // Create Position struct for the response
                    calculated_positions.insert(symbol.clone(), crate::types::Position {
                        id: format!("{}_{}", symbol, position.opened_at.timestamp()),
                        symbol: symbol.clone(),
                        quantity: position.quantity,
                        avg_cost_basis: position.avg_cost_basis,
                        total_cost: position.total_cost,
                        current_price,
                        realized_pnl: position.realized_pnl,
                        opened_at: position.opened_at,
                        closed_at: position.closed_at,
                        status: position.status.clone(),
                        session_id: position.session_id.clone(),
                    });
                }
                Err(e) => {
                    warn!("Could not get current price for {}: {}", symbol, e);
                    // Use average cost basis as fallback
                    let position_value = position.avg_cost_basis * position.quantity as f64;
                    total_position_value += position_value;
                    total_exposure += position_value;
                    
                    calculated_positions.insert(symbol.clone(), crate::types::Position {
                        id: format!("{}_{}", symbol, position.opened_at.timestamp()),
                        symbol: symbol.clone(),
                        quantity: position.quantity,
                        avg_cost_basis: position.avg_cost_basis,
                        total_cost: position.total_cost,
                        current_price: position.avg_cost_basis, // Fallback
                        realized_pnl: position.realized_pnl,
                        opened_at: position.opened_at,
                        closed_at: position.closed_at,
                        status: position.status.clone(),
                        session_id: position.session_id.clone(),
                    });
                }
            }
        }
        
        let portfolio_state = crate::types::PortfolioState {
            total_value: session.current_cash + total_position_value,
            available_cash: session.current_cash,
            total_exposure,
            max_positions: 20, // Default max positions for paper trading
            current_position_count: calculated_positions.len() as u32, // Use actual consolidated positions count
            positions: calculated_positions,
            last_updated: chrono::Utc::now(),
        };
        
        tracing::info!("📊 Real portfolio snapshot: ${:.2} total (${:.2} cash + ${:.2} positions), {} positions", 
            portfolio_state.total_value, portfolio_state.available_cash, total_position_value, 
            portfolio_state.current_position_count);
        
        Ok(portfolio_state)
    }

    /// Check if broker is in paper trading mode
    pub fn is_paper_mode(&self) -> bool {
        true // Always true for PaperBroker
    }

    /// Refresh position cache from database (used after database reset)
    pub async fn refresh_position_cache(&self) -> Result<()> {
        tracing::info!("🔄 Refreshing paper broker position and session cache from database");
        
        // Reload positions from database
        let new_positions = Self::load_open_positions(&self.config.session_id, &self.database).await?;
        
        // Update the cached positions
        {
            let mut positions = self.open_positions.write().await;
            *positions = new_positions;
        }
        
        // Reload session data from database
        let updated_session = Self::load_or_create_session(&self.config, &self.database).await?;
        
        // Update the cached session
        {
            let mut session = self.session.write().await;
            *session = updated_session;
        }
        
        let position_count = self.open_positions.read().await.len();
        let current_cash = self.session.read().await.current_cash;
        
        tracing::info!("✅ Paper broker cache refreshed: {} positions, ${:.2} cash", 
            position_count, current_cash);
        
        Ok(())
    }

    /// Refresh positions from database after trade execution
    async fn refresh_positions_from_db(&self) -> Result<()> {
        let new_positions = Self::load_open_positions(&self.config.session_id, &self.database).await?;
        
        {
            let mut positions = self.open_positions.write().await;
            *positions = new_positions;
        }
        
        Ok(())
    }

    /// Process a buy order with proper weighted average cost basis calculation
    async fn process_buy_order(&self, symbol: &str, quantity: u64, price: f64, trade_id: String) -> Result<()> {
        let mut positions_guard = self.open_positions.write().await;
        
        if let Some(existing_position) = positions_guard.get_mut(symbol) {
            // Add to existing position with weighted average cost basis
            existing_position.add_shares(quantity, price);
            info!("📊 Added {} shares of {} @ ${:.5} to existing position. New basis: ${:.5}, Total: {} shares, Cost: ${:.2}",
                  quantity, symbol, price, existing_position.avg_cost_basis, 
                  existing_position.quantity, existing_position.total_cost);
        } else {
            // Create new position
            let new_position = Position::new(
                trade_id,
                symbol.to_string(),
                quantity,
                price,
                self.config.session_id.clone(),
            );
            
            info!("🎯 Created new position: {} shares of {} @ ${:.5}, Cost: ${:.2}",
                  quantity, symbol, price, new_position.total_cost);
            
            positions_guard.insert(symbol.to_string(), new_position);
        }
        
        drop(positions_guard);

        // Store position in database
        self.store_position_in_database(symbol).await?;
        
        // Auto-subscribe to WebSocket for position updates
        if let Err(e) = self.data_module.websocket_subscribe(vec![symbol.to_string()]).await {
            warn!("Failed to subscribe to position symbol {}: {}", symbol, e);
        }

        Ok(())
    }

    /// Process a sell order with proper realized P&L calculation
    async fn process_sell_order(&self, symbol: &str, quantity: u64, price: f64) -> Result<f64> {
        let mut positions_guard = self.open_positions.write().await;
        
        let position = positions_guard.get_mut(symbol)
            .ok_or_else(|| anyhow!("No open position found for {}", symbol))?;

        if position.quantity < quantity {
            return Err(anyhow!("Cannot sell {} shares: only {} shares held", quantity, position.quantity));
        }

        // Calculate realized P&L and update position
        let realized_pnl = position.remove_shares(quantity, price);
        
        info!("💰 Sold {} shares of {} @ ${:.5}. Realized P&L: ${:.2}, Position: {} shares remaining @ ${:.5} basis",
              quantity, symbol, price, realized_pnl, position.quantity, position.avg_cost_basis);

        // If position is closed, unsubscribe from WebSocket
        if position.quantity == 0 {
            drop(positions_guard);
            if let Err(e) = self.data_module.websocket_unsubscribe(vec![symbol.to_string()]).await {
                warn!("Failed to unsubscribe from closed position symbol {}: {}", symbol, e);
            }
            info!("🔴 Position closed for {}", symbol);
        } else {
            drop(positions_guard);
        }

        // Update position in database
        self.store_position_in_database(symbol).await?;
        
        Ok(realized_pnl)
    }

    /// Process a sell order directly with database positions (handles multiple position records)
    async fn process_database_sell_order(&self, symbol: &str, quantity: u64, price: f64) -> Result<f64> {
        // Get all open positions for this symbol from database
        let rows = sqlx::query_as::<_, (String, i64, f64, f64, f64)>(
            "SELECT id, quantity, avg_cost_basis, total_cost, realized_pnl 
             FROM positions 
             WHERE symbol = ? AND session_id = ? AND status = 'OPEN' AND quantity > 0
             ORDER BY opened_at ASC" // FIFO order
        )
        .bind(symbol)
        .bind(&self.config.session_id)
        .fetch_all(&*self.database)
        .await?;

        if rows.is_empty() {
            return Err(anyhow!("No open position found for {}", symbol));
        }

        // Calculate total shares available
        let total_shares: u64 = rows.iter().map(|(_, qty, _, _, _)| *qty as u64).sum();
        
        if quantity > total_shares {
            return Err(anyhow!("Cannot sell {} shares of {}: only {} shares held", 
                              quantity, symbol, total_shares));
        }

        let mut remaining_to_sell = quantity;
        let mut total_realized_pnl = 0.0;

        // Process sell against positions in FIFO order
        for (id, pos_quantity, avg_cost_basis, total_cost, current_realized_pnl) in rows {
            if remaining_to_sell == 0 {
                break;
            }

            let pos_quantity = pos_quantity as u64;
            let shares_to_sell_from_position = remaining_to_sell.min(pos_quantity);
            
            // Calculate realized P&L for this portion
            let realized_pnl = shares_to_sell_from_position as f64 * (price - avg_cost_basis);
            total_realized_pnl += realized_pnl;
            
            let new_quantity = pos_quantity - shares_to_sell_from_position;
            let new_total_cost = new_quantity as f64 * avg_cost_basis;
            let new_realized_pnl = current_realized_pnl + realized_pnl;

            info!("💰 Selling {} shares from position {} @ ${:.5}. Position P&L: ${:.2}", 
                  shares_to_sell_from_position, id, price, realized_pnl);

            if new_quantity == 0 {
                // Close this position completely
                sqlx::query(
                    "UPDATE positions 
                     SET quantity = 0, total_cost = 0.0, realized_pnl = ?, 
                         status = 'CLOSED', closed_at = ?
                     WHERE id = ?"
                )
                .bind(new_realized_pnl)
                .bind(chrono::Utc::now().timestamp())
                .bind(&id)
                .execute(&*self.database)
                .await?;
                
                info!("🔴 Position {} closed completely", id);
            } else {
                // Update position with reduced quantity
                sqlx::query(
                    "UPDATE positions 
                     SET quantity = ?, total_cost = ?, realized_pnl = ?, current_price = ?
                     WHERE id = ?"
                )
                .bind(new_quantity as i64)
                .bind(new_total_cost)
                .bind(new_realized_pnl)
                .bind(price)
                .bind(&id)
                .execute(&*self.database)
                .await?;
                
                info!("📊 Position {} updated: {} shares remaining @ ${:.5} basis", 
                      id, new_quantity, avg_cost_basis);
            }

            remaining_to_sell -= shares_to_sell_from_position;
        }

        // Check if we need to unsubscribe from WebSocket (no open positions left)
        let open_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM positions 
             WHERE symbol = ? AND session_id = ? AND status = 'OPEN' AND quantity > 0"
        )
        .bind(symbol)
        .bind(&self.config.session_id)
        .fetch_one(&*self.database)
        .await?;

        if open_count == 0 {
            if let Err(e) = self.data_module.websocket_unsubscribe(vec![symbol.to_string()]).await {
                warn!("Failed to unsubscribe from closed position symbol {}: {}", symbol, e);
            }
            info!("🔴 All positions closed for {}, unsubscribed from WebSocket", symbol);
        }

        Ok(total_realized_pnl)
    }

    /// Store position data in the positions table
    async fn store_position_in_database(&self, symbol: &str) -> Result<()> {
        let positions_guard = self.open_positions.read().await;
        
        if let Some(position) = positions_guard.get(symbol) {
            // Insert or update position in database
            sqlx::query(
                "INSERT OR REPLACE INTO positions (
                    id, symbol, quantity, avg_cost_basis, total_cost, current_price,
                    realized_pnl, opened_at, closed_at, status, session_id
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&position.id)
            .bind(&position.symbol)
            .bind(position.quantity as i64)
            .bind(position.avg_cost_basis)
            .bind(position.total_cost)
            .bind(position.current_price)
            .bind(position.realized_pnl)
            .bind(position.opened_at.timestamp())
            .bind(position.closed_at.map(|dt| dt.timestamp()))
            .bind(position.status.to_string())
            .bind(&position.session_id)
            .execute(&*self.database)
            .await?;
            
            info!("💾 Stored position {} in database", symbol);
        }
        
        Ok(())
    }
}