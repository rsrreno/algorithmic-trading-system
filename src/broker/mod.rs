// src/broker/mod.rs
// Branch: 14.9.25

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

pub mod lightspeed;
pub mod paper;
pub mod traits;

use crate::config::{LightspeedConfig, TradingMode};
use crate::types::{Position, PositionSide};
use crate::database::Database;
use crate::data::DataModule;
pub use lightspeed::{LightspeedBroker, BrokerEvent};
pub use paper::PaperBroker;
pub use traits::{BrokerTrait, AccountInfo, BrokerPosition, BrokerTrade};

#[derive(Debug)]
pub struct BrokerModule {
    lightspeed: Option<Arc<LightspeedBroker>>,
    paper_broker: Option<Arc<PaperBroker>>,
    lightspeed_enabled: bool,
    paper_enabled: bool,
    trading_mode: TradingMode,
    database: Arc<Database>,
}

impl BrokerModule {
    pub fn new(trading_mode: TradingMode, database: Arc<Database>) -> Self {
        Self {
            lightspeed: None,
            paper_broker: None,
            lightspeed_enabled: false,
            paper_enabled: false,
            trading_mode,
            database,
        }
    }

    pub async fn try_initialize_lightspeed(
        &mut self, 
        config: LightspeedConfig,
    ) -> Result<()> {
        match self.initialize_lightspeed_internal(config).await {
            Ok(_) => {
                self.lightspeed_enabled = true;
                tracing::info!("✅ LightSpeed broker initialized successfully");
                Ok(())
            }
            Err(e) => {
                self.lightspeed_enabled = false;
                tracing::warn!("⚠️ LightSpeed broker initialization failed: {} - continuing without broker", e);
                // Return Ok to not crash the system
                Ok(())
            }
        }
    }

    pub async fn try_initialize_paper_broker(
        &mut self,
        config: crate::config::PaperTradingConfig,
        database: Arc<Database>,
        data_module: Arc<DataModule>,
    ) -> Result<()> {
        match self.initialize_paper_broker_internal(config, database, data_module).await {
            Ok(_) => {
                self.paper_enabled = true;
                tracing::info!("✅ Paper broker initialized successfully");
                Ok(())
            }
            Err(e) => {
                self.paper_enabled = false;
                tracing::warn!("⚠️ Paper broker initialization failed: {} - continuing without paper trading", e);
                Ok(())
            }
        }
    }

    async fn initialize_lightspeed_internal(
        &mut self, 
        config: LightspeedConfig,
    ) -> Result<()> {
        let mut broker = LightspeedBroker::new(
            config.clone(),
            config.client_id.clone(),
            config.account_id.clone(),
        );
        broker.connect().await?;
        self.lightspeed = Some(Arc::new(broker));
        Ok(())
    }

    async fn initialize_paper_broker_internal(
        &mut self,
        config: crate::config::PaperTradingConfig,
        database: Arc<Database>,
        data_module: Arc<DataModule>,
    ) -> Result<()> {
        let paper_config = paper::PaperTradingConfig {
            initial_cash: config.initial_cash,
            commission_per_share: config.commission_per_share,
            enable_commission: config.enable_commission,
            session_id: config.session_id,
        };
        
        let broker = PaperBroker::new(paper_config, database, data_module).await?;
        self.paper_broker = Some(Arc::new(broker));
        self.paper_enabled = true;
        Ok(())
    }

    pub async fn place_stock_order(
        &self,
        symbol: &str,
        side: &str, // BUY, SELL, SELL_SHORT
        order_type: &str, // MARKET, LIMIT
        quantity: u64,
        price: Option<f64>,
    ) -> Result<String> {
        match self.trading_mode {
            TradingMode::Paper => {
                if let Some(paper_broker) = &self.paper_broker {
                    // Execute paper trade with proper SELL vs SELL_SHORT logic
                    tracing::info!("📝 Executing paper trade: {} {} shares of {}", side, quantity, symbol);
                    paper_broker.execute_trade_with_side(
                        symbol, 
                        side, // Pass string directly - method will handle BUY/SELL/SELL_SHORT logic
                        quantity as u32, 
                        None, // rule_id - can be added later for rule tracking
                        None  // rule_name - can be added later for rule tracking
                    ).await
                } else {
                    anyhow::bail!("Paper broker not available - check configuration")
                }
            }
            TradingMode::Live => {
                if let Some(broker) = &self.lightspeed {
                    let order = broker.create_stock_order(symbol, side, order_type, quantity, price);
                    broker.place_order(order).await
                } else {
                    anyhow::bail!("LightSpeed broker not available - check configuration and connection")
                }
            }
            TradingMode::Simulation => {
                // For simulation mode, use paper broker but with different logging
                if let Some(paper_broker) = &self.paper_broker {
                    let position_side = match side.to_uppercase().as_str() {
                        "BUY" => PositionSide::Long,
                        "SELL" | "SELL_SHORT" => PositionSide::Short,
                        _ => return Err(anyhow::anyhow!("Invalid order side: {}", side)),
                    };
                    
                    tracing::info!("🎮 SIMULATION MODE: Executing trade for {}", symbol);
                    paper_broker.execute_trade(
                        symbol, 
                        position_side, 
                        quantity as u32, 
                        None, // rule_id - can be added later for rule tracking
                        None  // rule_name - can be added later for rule tracking
                    ).await
                } else {
                    anyhow::bail!("Paper broker not available for simulation mode")
                }
            }
        }
    }

    pub async fn cancel_order(&self, client_order_id: &str) -> Result<()> {
        if let Some(broker) = &self.lightspeed {
            broker.cancel_order(client_order_id).await
        } else {
            anyhow::bail!("LightSpeed broker not available - check configuration and connection")
        }
    }

    pub async fn get_positions(&self) -> Result<HashMap<String, Position>> {
        match self.trading_mode {
            TradingMode::Paper | TradingMode::Simulation => {
                if let Some(paper_broker) = &self.paper_broker {
                    paper_broker.get_positions().await
                } else {
                    Ok(HashMap::new())
                }
            }
            TradingMode::Live => {
                if let Some(broker) = &self.lightspeed {
                    Ok(broker.get_positions().await)
                } else {
                    Ok(HashMap::new())
                }
            }
        }
    }

    pub async fn is_connected(&self) -> bool {
        match self.trading_mode {
            TradingMode::Paper | TradingMode::Simulation => {
                // Paper trading is always "connected" if initialized
                self.paper_enabled
            }
            TradingMode::Live => {
                if let Some(broker) = &self.lightspeed {
                    broker.is_connected().await
                } else {
                    false
                }
            }
        }
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(_broker) = self.lightspeed.take() {
            // The connection will be dropped when the Arc is dropped
        }
        self.lightspeed = None;
        // self.paper_broker = None;  // TODO: Enable when implemented
        Ok(())
    }

    // Event streaming for the trading engine (only for LightSpeed)
    pub async fn get_broker_events(&self) -> Result<mpsc::UnboundedReceiver<BrokerEvent>> {
        if let Some(broker) = &self.lightspeed {
            Ok(broker.get_events().await)
        } else {
            anyhow::bail!("LightSpeed broker not available - check configuration and connection")
        }
    }
    
    pub fn is_lightspeed_enabled(&self) -> bool {
        self.lightspeed_enabled && self.lightspeed.is_some()
    }

    pub fn is_paper_enabled(&self) -> bool {
        // TODO: Check paper broker when implemented
        self.paper_enabled
    }

    pub fn get_trading_mode(&self) -> &TradingMode {
        &self.trading_mode
    }

    // Paper trading specific methods
    pub async fn execute_paper_trade(
        &self,
        symbol: &str,
        side: PositionSide,
        quantity: u32,
        rule_id: Option<String>,
        rule_name: Option<String>,
    ) -> Result<String> {
        if let Some(paper_broker) = &self.paper_broker {
            paper_broker.execute_trade(symbol, side, quantity, rule_id, rule_name).await
        } else {
            anyhow::bail!("Paper broker not available - check configuration")
        }
    }

    pub async fn close_paper_position(&self, symbol: &str, rule_id: Option<String>) -> Result<f64> {
        if let Some(paper_broker) = &self.paper_broker {
            paper_broker.close_position(symbol, rule_id).await
        } else {
            anyhow::bail!("Paper broker not available - check configuration")
        }
    }

    // pub async fn get_paper_session_status(&self) -> Result<paper::PaperSession> {
    //     // TODO: Implement session status retrieval
    //     anyhow::bail!("Paper trading session not implemented yet")
    // }

    pub async fn calculate_unrealized_pnl(&self) -> Result<f64> {
        if let Some(paper_broker) = &self.paper_broker {
            paper_broker.calculate_unrealized_pnl().await
        } else {
            Ok(0.0)
        }
    }

    /// Get real-time portfolio snapshot with actual position values
    pub async fn get_portfolio_snapshot(&self) -> Result<crate::types::PortfolioState> {
        if let Some(paper_broker) = &self.paper_broker {
            paper_broker.get_portfolio_snapshot().await
        } else {
            // Return empty portfolio if no paper broker
            Ok(crate::types::PortfolioState {
                total_value: 0.0,
                available_cash: 0.0,
                total_exposure: 0.0,
                max_positions: 0,
                current_position_count: 0,
                positions: std::collections::HashMap::new(),
                last_updated: chrono::Utc::now(),
            })
        }
    }

    /// Refresh paper broker position cache (used after database reset)
    pub async fn refresh_paper_broker_cache(&mut self) -> Result<()> {
        if let Some(paper_broker) = &self.paper_broker {
            paper_broker.refresh_position_cache().await?;
            tracing::info!("✅ Paper broker position cache refreshed");
        } else {
            tracing::warn!("⚠️ No paper broker available to refresh cache");
        }
        Ok(())
    }

    /// Store live broker position in database
    pub async fn store_live_position(&self, broker_name: &str, position: &BrokerPosition) -> Result<()> {
        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            "INSERT OR REPLACE INTO live_positions (
                id, broker_name, symbol, quantity, avg_cost_basis, current_price,
                market_value, unrealized_pnl, realized_pnl, opened_at, last_updated,
                status, account_id, position_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(format!("{}_{}", broker_name, position.symbol))
        .bind(broker_name)
        .bind(&position.symbol)
        .bind(position.quantity)
        .bind(position.cost_basis / position.quantity.abs() as f64) // avg cost basis
        .bind(position.market_value / position.quantity.abs() as f64) // current price
        .bind(position.market_value)
        .bind(position.unrealized_pnl)
        .bind(position.realized_pnl)
        .bind(position.last_updated.timestamp())
        .bind(now)
        .bind("OPEN")
        .bind("default") // TODO: Get actual account_id from broker config
        .bind(&position.broker_position_id)
        .execute(&*self.database)
        .await?;

        tracing::debug!("📊 Stored live position for {} from {}", position.symbol, broker_name);
        Ok(())
    }

    /// Record a live broker trade in database
    pub async fn record_live_trade(&self, broker_name: &str, trade: &BrokerTrade) -> Result<()> {
        let _now = chrono::Utc::now().timestamp();

        sqlx::query(
            "INSERT INTO live_trades (
                id, broker_name, order_id, client_order_id, symbol, side, quantity,
                executed_price, execution_time, commission, fees, net_amount, account_id,
                realized_pnl, notes
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&trade.trade_id)
        .bind(broker_name)
        .bind(&trade.order_id)
        .bind(&trade.trade_id) // Use trade_id as client_order_id for now
        .bind(&trade.symbol)
        .bind(&trade.side)
        .bind(trade.quantity as i64)
        .bind(trade.price)
        .bind(trade.execution_time.timestamp())
        .bind(trade.commission)
        .bind(trade.fees)
        .bind(trade.price * trade.quantity as f64 - trade.commission - trade.fees)
        .bind("default") // TODO: Get actual account_id
        .bind(trade.realized_pnl)
        .bind(format!("Live trade from {}", broker_name))
        .execute(&*self.database)
        .await?;

        // If there's realized P&L, record it separately
        if let Some(pnl) = trade.realized_pnl {
            self.record_live_pnl(broker_name, &trade.symbol, &trade.trade_id, pnl).await?;
        }

        tracing::info!("📈 Recorded live trade: {} {} shares of {} @ ${:.2}",
                      trade.side, trade.quantity, trade.symbol, trade.price);
        Ok(())
    }

    /// Record realized P&L from live broker
    pub async fn record_live_pnl(&self, broker_name: &str, symbol: &str, trade_id: &str, amount: f64) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let pnl_id = format!("{}_{}_{}_{}", broker_name, symbol, trade_id, now);

        sqlx::query(
            "INSERT INTO live_pnl (
                id, broker_name, symbol, trade_id, pnl_amount, pnl_type,
                calculation_method, recorded_at, account_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(pnl_id)
        .bind(broker_name)
        .bind(symbol)
        .bind(trade_id)
        .bind(amount)
        .bind("REALIZED")
        .bind("BROKER_CALCULATED")
        .bind(now)
        .bind("default") // TODO: Get actual account_id
        .execute(&*self.database)
        .await?;

        tracing::info!("💰 Recorded realized P&L: ${:.2} for {} from {}", amount, symbol, broker_name);
        Ok(())
    }

    /// Get live positions from database for a specific broker
    pub async fn get_live_positions(&self, broker_name: &str) -> Result<HashMap<String, Position>> {
        let rows = sqlx::query_as::<_, (String, String, i64, f64, f64, f64, f64, i64, i64, String)>(
            "SELECT id, symbol, quantity, avg_cost_basis, current_price, unrealized_pnl,
                    realized_pnl, opened_at, last_updated, status
             FROM live_positions
             WHERE broker_name = ? AND status = 'OPEN' AND quantity != 0"
        )
        .bind(broker_name)
        .fetch_all(&*self.database)
        .await?;

        let mut positions = HashMap::new();

        for row in rows {
            let (id, symbol, quantity, avg_cost_basis, current_price, _unrealized_pnl,
                 realized_pnl, opened_at, last_updated, status) = row;

            let position = Position {
                id,
                symbol: symbol.clone(),
                quantity: quantity.abs() as u64,
                avg_cost_basis,
                total_cost: avg_cost_basis * quantity.abs() as f64,
                current_price,
                realized_pnl,
                opened_at: chrono::DateTime::from_timestamp(opened_at, 0).unwrap_or_default(),
                closed_at: None, // Live positions don't have closed_at - they use last_updated
                status: status.parse().unwrap_or(crate::types::PositionStatus::Open),
                session_id: format!("{}_session", broker_name),
            };

            positions.insert(symbol, position);
        }

        tracing::debug!("📊 Retrieved {} live positions for {}", positions.len(), broker_name);
        Ok(positions)
    }

    /// Update broker connection status in database
    pub async fn update_broker_status(&self, broker_name: &str, is_connected: bool, error: Option<String>) -> Result<()> {
        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            "INSERT OR REPLACE INTO broker_status (
                broker_name, is_connected, last_connected, last_disconnected,
                connection_attempts, last_error, account_id
            ) VALUES (?, ?, ?, ?,
                     COALESCE((SELECT connection_attempts FROM broker_status WHERE broker_name = ?), 0) + 1,
                     ?, ?)"
        )
        .bind(broker_name)
        .bind(is_connected)
        .bind(if is_connected { Some(now) } else { None })
        .bind(if !is_connected { Some(now) } else { None })
        .bind(broker_name) // For the COALESCE subquery
        .bind(error)
        .bind("default") // TODO: Get actual account_id
        .execute(&*self.database)
        .await?;

        tracing::debug!("📊 Updated {} broker status: connected={}", broker_name, is_connected);
        Ok(())
    }

    /// Sync positions from live broker to database
    pub async fn sync_lightspeed_positions(&self) -> Result<()> {
        if let Some(broker) = &self.lightspeed {
            let positions = broker.get_positions().await;

            let position_count = positions.len();
            for (_symbol, position) in positions {
                let broker_position = BrokerPosition {
                    broker_position_id: Some(position.id.clone()),
                    symbol: position.symbol.clone(),
                    quantity: position.quantity as i64,
                    market_value: position.current_price * position.quantity as f64,
                    cost_basis: position.avg_cost_basis * position.quantity as f64,
                    unrealized_pnl: (position.current_price - position.avg_cost_basis) * position.quantity as f64,
                    realized_pnl: position.realized_pnl,
                    last_updated: chrono::Utc::now(),
                };

                self.store_live_position("lightspeed", &broker_position).await?;
            }

            self.update_broker_status("lightspeed", true, None).await?;
            tracing::info!("✅ Synced {} Lightspeed positions to database", position_count);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LightspeedConfig;

    #[tokio::test]
    async fn test_broker_initialization() {
        let mut broker = BrokerModule::new(TradingMode::Paper, Arc::new(sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap()));
        
        let config = LightspeedConfig {
            api_key: "test_key".to_string(),
            connection_url: "wss://test.url".to_string(),
            client_id: "TEST_CLIENT".to_string(),
            account_id: "TEST_ACCOUNT".to_string(),
            sandbox: true,
        };

        // This will fail without real credentials, but tests the structure
        let result = broker.initialize_lightspeed(config).await;

        // In a real test environment, this should succeed
        assert!(result.is_err()); // Expected to fail with test credentials
    }
}