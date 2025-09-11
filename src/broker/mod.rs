// src/broker/mod.rs
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

pub mod lightspeed;
pub mod paper;

use crate::config::{LightspeedConfig, TradingMode};
use crate::types::{Position, PositionSide};
use crate::database::Database;
use crate::data::DataModule;
pub use lightspeed::{LightspeedBroker, BrokerEvent};
pub use paper::PaperBroker;

#[derive(Debug)]
pub struct BrokerModule {
    lightspeed: Option<Arc<LightspeedBroker>>,
    paper_broker: Option<Arc<PaperBroker>>,
    lightspeed_enabled: bool,
    paper_enabled: bool,
    trading_mode: TradingMode,
}

impl BrokerModule {
    pub fn new(trading_mode: TradingMode) -> Self {
        Self {
            lightspeed: None,
            paper_broker: None,
            lightspeed_enabled: false,
            paper_enabled: false,
            trading_mode,
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
                    // Convert side string to PositionSide
                    let position_side = match side.to_uppercase().as_str() {
                        "BUY" => PositionSide::Long,
                        "SELL" | "SELL_SHORT" => PositionSide::Short,
                        _ => return Err(anyhow::anyhow!("Invalid order side: {}", side)),
                    };
                    
                    // Execute paper trade
                    tracing::info!("📝 Executing paper trade: {} {} shares of {}", side, quantity, symbol);
                    paper_broker.execute_trade(
                        symbol, 
                        position_side, 
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LightspeedConfig;

    #[tokio::test]
    async fn test_broker_initialization() {
        let mut broker = BrokerModule::new();
        
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