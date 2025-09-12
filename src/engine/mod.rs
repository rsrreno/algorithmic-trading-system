// src/engine/mod.rs
// Branch: 9.2.25.1

use anyhow::Result;
use std::sync::{atomic::AtomicU64, Arc};
use tokio::sync::RwLock;
use dashmap::DashMap;
use tracing::{info, debug, error, warn};

use crate::config::Config;
use crate::types::{Position, MarketDataStream, RiskParameters};
use crate::broker::BrokerModule;
use crate::data::DataModule;
use crate::rules::RulesEngine;
use crate::database::Database;

pub struct TradingEngine {
    config: Config,
    
    // Database connection
    database: Arc<Database>,
    
    // Data module for market data
    data_module: DataModule,
    
    // Memory management
    total_memory_used: AtomicU64,
    max_memory_bytes: u64,
    
    // Active positions and market data
    positions: Arc<RwLock<DashMap<String, Position>>>,
    market_streams: Arc<RwLock<DashMap<String, MarketDataStream>>>,
    
    // Broker integration
    broker_module: Arc<RwLock<BrokerModule>>,
    
    // Rules engine for automated trading decisions
    rules_engine: Option<Arc<RulesEngine>>,
    
    // Shutdown signal
    shutdown_signal: Arc<tokio::sync::Notify>,
}

impl TradingEngine {
    pub async fn new(config: Config, database: Arc<Database>) -> Result<Self> {
        info!("Initializing trading engine");
        
        let max_memory_bytes = config.max_memory_bytes();
        
        // Initialize data module
        let mut data_module = DataModule::new(&config)?;
        info!("✅ Data module initialized");
        
        // Initialize market schedule system
        if let Err(e) = data_module.initialize_market_schedule(&database).await {
            warn!("⚠️ Failed to initialize market schedule: {}", e);
        }
        
        // Start WebSocket connections if enabled
        if let Err(e) = data_module.start_websocket().await {
            warn!("⚠️ Failed to start WebSocket: {}", e);
        }
        
        // Initialize broker module with trading mode
        let mut broker = BrokerModule::new(config.trading_mode.clone());
        
        // Initialize appropriate broker based on trading mode
        match config.trading_mode {
            crate::config::TradingMode::Paper | crate::config::TradingMode::Simulation => {
                // Initialize paper broker with database and data module
                tracing::info!("📊 Initializing paper trading broker");
                broker.try_initialize_paper_broker(
                    config.paper_trading_config.clone(),
                    database.clone(),
                    Arc::new(data_module.clone()),
                ).await?;
                tracing::info!("✅ Paper trading broker initialized successfully");
            }
            crate::config::TradingMode::Live => {
                // Try to initialize LightSpeed connection for live trading
                if config.is_lightspeed_enabled() {
                    if let Some(lightspeed_config) = config.lightspeed_config.clone() {
                        broker.try_initialize_lightspeed(lightspeed_config).await?;
                    }
                } else {
                    tracing::info!("🔧 LightSpeed broker disabled in configuration");
                }
            }
        }
        
        let engine = TradingEngine {
            config,
            database,
            data_module,
            broker_module: Arc::new(RwLock::new(broker)),
            rules_engine: None, // Will be initialized separately if rules engine is enabled
            total_memory_used: AtomicU64::new(0),
            max_memory_bytes,
            positions: Arc::new(RwLock::new(DashMap::new())),
            market_streams: Arc::new(RwLock::new(DashMap::new())),
            shutdown_signal: Arc::new(tokio::sync::Notify::new()),
        };
        
        info!("Trading engine initialized with {}MB memory limit", 
               engine.max_memory_bytes / 1024 / 1024);
        
        Ok(engine)
    }
    
    pub async fn run(&self) -> Result<()> {
        info!("Starting trading engine main loop");
        
        // Test data connections on startup
        if let Err(e) = self.data_module.test_connection().await {
            error!("Failed to test data connection: {}", e);
        }
        
        // Start broker event handler
        let mut broker_events_task: Option<tokio::task::JoinHandle<()>> = None;
        
        loop {
            tokio::select! {
                // Wait for shutdown signal
                _ = self.shutdown_signal.notified() => {
                    info!("Shutdown signal received, stopping trading engine");
                    break;
                }
                
                // Main trading loop
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    // TODO: Implement main trading logic
                    if let Err(e) = self.process_market_data().await {
                        error!("Error processing market data: {}", e);
                    }
                    
                    // Process WebSocket messages and update cache
                    if let Err(e) = self.data_module.process_websocket_messages().await {
                        debug!("Error processing WebSocket messages: {}", e);
                    }
                    
                    if let Err(e) = self.evaluate_trading_rules().await {
                        error!("Error evaluating trading rules: {}", e);
                    }
                }
                
                // Handle broker events task completion (if exists)
                _ = async {
                    if let Some(task) = &mut broker_events_task {
                        task.await
                    } else {
                        // If no task, just sleep forever
                        std::future::pending().await
                    }
                } => {
                    warn!("Broker event handler task completed unexpectedly");
                }
            }
        }
        
        Ok(())
    }
    
    async fn process_market_data(&self) -> Result<()> {
        // Placeholder for market data processing
        debug!("Processing market data");
        
        // Periodically fetch test symbol data (using LightSpeed certification symbols)
        static mut COUNTER: u32 = 0;
        static mut SYMBOL_INDEX: usize = 0;
        
        unsafe {
            COUNTER += 1;
            // Fetch test symbol data every 600 iterations (roughly every 60 seconds at 100ms intervals)
            if COUNTER % 600 == 0 {
                let test_symbols = ["GOOGL", "AMZN", "TSLA", "MSFT"]; // LightSpeed test symbols
                let symbol = test_symbols[SYMBOL_INDEX % test_symbols.len()];
                
                if let Err(e) = self.data_module.get_ticker_snapshot(symbol).await {
                    error!("Failed to fetch {} data: {}", symbol, e);
                } else {
                    debug!("Fetched {} data successfully", symbol);
                }
                
                SYMBOL_INDEX += 1;
            }
        }
        
        Ok(())
    }
    
    async fn evaluate_trading_rules(&self) -> Result<()> {
        // Rules engine evaluation is handled automatically by the RulesEngine
        // when it's started. This method is kept for manual evaluation if needed.
        debug!("Trading rules evaluation (handled by RulesEngine if enabled)");
        
        Ok(())
    }
    
    pub async fn lookup_symbol(&self, symbol: &str) -> Result<crate::data::SymbolDataResponse> {
        self.data_module.get_symbol_data(symbol).await
    }

    /// Get current market data for symbol (uses snapshot API for real-time prices)
    pub async fn get_current_symbol_data(&self, symbol: &str) -> Result<crate::data::SymbolDataResponse> {
        // Try to get current price from snapshot API first
        match self.data_module.get_current_market_price(symbol).await {
            Ok(current_price) => {
                // Get previous day data for volume/other stats, but use current price
                match self.data_module.get_symbol_data(symbol).await {
                    Ok(mut prev_data) => {
                        // Update with current price and calculate change
                        let prev_close = prev_data.close;
                        prev_data.close = current_price;
                        prev_data.change = current_price - prev_close;
                        prev_data.change_percent = if prev_close > 0.0 {
                            ((current_price - prev_close) / prev_close) * 100.0
                        } else {
                            0.0
                        };
                        prev_data.date = chrono::Utc::now().format("%Y-%m-%d").to_string();
                        Ok(prev_data)
                    }
                    Err(_) => {
                        // Create minimal response with current price only
                        Ok(crate::data::SymbolDataResponse {
                            symbol: symbol.to_string(),
                            date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                            open: current_price,
                            high: current_price,
                            low: current_price,
                            close: current_price,
                            volume: 0,
                            vwap: current_price,
                            transactions: 0,
                            change: 0.0,
                            change_percent: 0.0,
                        })
                    }
                }
            }
            Err(_) => {
                // Fall back to previous day data if current price not available
                self.data_module.get_symbol_data(symbol).await
            }
        }
    }
    
    pub async fn place_order(
        &self,
        symbol: &str,
        side: &str,
        order_type: &str,
        quantity: u64,
        price: Option<f64>,
    ) -> Result<String> {
        let broker = self.broker_module.read().await;
        broker.place_stock_order(symbol, side, order_type, quantity, price).await
    }
    
    pub async fn cancel_order(&self, client_order_id: &str) -> Result<()> {
        let broker = self.broker_module.read().await;
        broker.cancel_order(client_order_id).await
    }
    
    pub async fn get_positions(&self) -> Result<std::collections::HashMap<String, Position>> {
        let broker = self.broker_module.read().await;
        broker.get_positions().await
    }
    
    pub async fn is_broker_connected(&self) -> bool {
        let broker = self.broker_module.read().await;
        broker.is_connected().await
    }
    
    pub fn get_data_module(&self) -> &DataModule {
        &self.data_module
    }
    
    pub fn get_config(&self) -> &Config {
        &self.config
    }
    
    pub async fn shutdown(&self) -> Result<()> {
        info!("Initiating trading engine shutdown");
        
        // Signal shutdown
        self.shutdown_signal.notify_waiters();
        
        // Disconnect broker
        let mut broker = self.broker_module.write().await;
        broker.disconnect().await?;
        
        // Close all positions (placeholder)
        let positions = self.positions.read().await;
        for entry in positions.iter() {
            let symbol = entry.key();
            info!("Position still open for {}", symbol);
        }
        
        // Clear market data streams
        let streams = self.market_streams.write().await;
        let memory_freed = streams.len() * 1024; // Rough estimate
        streams.clear();
        
        self.total_memory_used.store(0, std::sync::atomic::Ordering::Relaxed);
        
        info!("Trading engine shutdown completed, freed ~{}KB memory", memory_freed);
        Ok(())
    }
    
    pub fn get_memory_usage(&self) -> u64 {
        self.total_memory_used.load(std::sync::atomic::Ordering::Relaxed)
    }
    
    pub fn get_memory_usage_mb(&self) -> f64 {
        self.get_memory_usage() as f64 / 1024.0 / 1024.0
    }

    /// Initialize and start the rules engine (placeholder for now)
    pub async fn start_rules_engine(&mut self, initial_cash: f64) -> Result<()> {
        info!("🚀 Starting rules engine with ${:.2} initial cash", initial_cash);
        
        // Load risk parameters from database
        let risk_params = match RiskParameters::load_from_database(&self.database).await {
            Ok(params) => {
                info!("✅ Risk parameters loaded from database: max_positions={}, max_exposure={}%", 
                    params.max_positions, params.max_portfolio_exposure_percent);
                params
            }
            Err(e) => {
                warn!("Failed to load risk parameters from database, using defaults: {}", e);
                RiskParameters {
                    max_portfolio_exposure_percent: 80.0,
                    max_single_position_percent: 10.0,
                    max_positions: 5,
                    max_loss_per_trade_dollars: Some(1000.0),
                    max_daily_loss_dollars: Some(5000.0),
                    require_volume_confirmation: false,
                    min_volume_ratio: 1.0,
                    default_stop_loss_percent: 5.0,
                }
            }
        };

        // Initialize rules engine with configuration
        let rules_engine = RulesEngine::new(
            Arc::new(self.data_module.clone()),
            self.broker_module.clone(),
            self.database.clone(),
            risk_params,
            initial_cash,
            self.config.rules_engine_config.clone(),
        )?;
        
        // Start the rules engine
        if let Err(e) = rules_engine.start().await {
            error!("Failed to start rules engine: {}", e);
            return Err(e);
        }
        
        info!("✅ Rules engine started successfully with user-configured parameters");
        
        // Store the rules engine for later use
        self.rules_engine = Some(Arc::new(rules_engine));

        // Load existing rules from database
        if let Some(engine) = &self.rules_engine {
            if let Err(e) = engine.load_rules_from_database().await {
                warn!("Failed to load rules from database: {}", e);
            }
        }

        Ok(())
    }

    /// Stop the rules engine
    pub async fn stop_rules_engine(&mut self) -> Result<()> {
        if let Some(rules_engine) = &self.rules_engine {
            info!("Stopping rules engine");
            rules_engine.stop().await?;
            self.rules_engine = None;
            info!("✅ Rules engine stopped");
        } else {
            warn!("Rules engine was not running");
        }
        Ok(())
    }

    /// Add a trading rule to the rules engine
    pub async fn add_trading_rule(&self, rule: crate::types::EnhancedTradingRule) -> Result<()> {
        if let Some(rules_engine) = &self.rules_engine {
            rules_engine.add_rule(rule).await
        } else {
            Err(anyhow::anyhow!("Rules engine not initialized"))
        }
    }

    /// Remove a trading rule from the rules engine
    pub async fn remove_trading_rule(&self, rule_id: &str) -> Result<()> {
        if let Some(rules_engine) = &self.rules_engine {
            rules_engine.remove_rule(rule_id).await
        } else {
            Err(anyhow::anyhow!("Rules engine not initialized"))
        }
    }

    /// Get all trading rules from the rules engine
    pub async fn get_all_trading_rules(&self) -> Result<Vec<crate::types::EnhancedTradingRule>> {
        if let Some(rules_engine) = &self.rules_engine {
            rules_engine.get_all_rules().await
        } else {
            // If rules engine is not initialized, return empty vector
            Ok(Vec::new())
        }
    }

    /// Get portfolio state from rules engine
    pub async fn get_portfolio_state(&self) -> Result<crate::types::PortfolioState> {
        if let Some(rules_engine) = &self.rules_engine {
            rules_engine.get_portfolio().await
        } else {
            Err(anyhow::anyhow!("Rules engine not initialized"))
        }
    }

    /// Get rules engine performance statistics
    pub async fn get_rules_engine_stats(&self) -> Result<crate::rules::engine::EnginePerformanceStats> {
        if let Some(rules_engine) = &self.rules_engine {
            rules_engine.get_performance_stats().await
        } else {
            Err(anyhow::anyhow!("Rules engine not initialized"))
        }
    }

    /// Check if rules engine is running
    pub fn is_rules_engine_running(&self) -> bool {
        self.rules_engine.is_some()
    }

    /// Get broker module reference for rules engine integration  
    pub async fn get_broker_module(&self) -> tokio::sync::RwLockReadGuard<BrokerModule> {
        self.broker_module.read().await
    }

    /// Get database reference for configuration and persistence
    pub async fn get_database(&self) -> Result<Arc<Database>> {
        Ok(Arc::clone(&self.database))
    }

    /// Execute a paper trade through the broker
    pub async fn execute_paper_trade(
        &self,
        symbol: &str,
        side: crate::types::PositionSide,
        quantity: u32,
        rule_id: Option<String>,
        rule_name: Option<String>,
    ) -> Result<String> {
        let broker = self.broker_module.read().await;
        broker.execute_paper_trade(symbol, side, quantity, rule_id, rule_name).await
    }

    /// Close a paper trading position
    pub async fn close_paper_position(&self, symbol: &str, rule_id: Option<String>) -> Result<f64> {
        let broker = self.broker_module.read().await;
        broker.close_paper_position(symbol, rule_id).await
    }

    // /// Get paper trading session status
    // pub async fn get_paper_session_status(&self) -> Result<crate::broker::paper::PaperSession> {
    //     let broker = self.broker.read().await;
    //     broker.get_paper_session_status().await
    // }

    /// Calculate unrealized P&L for paper trading positions
    pub async fn calculate_unrealized_pnl(&self) -> Result<f64> {
        let broker = self.broker_module.read().await;
        broker.calculate_unrealized_pnl().await
    }

    /// Check if paper trading is enabled
    pub async fn is_paper_trading_enabled(&self) -> bool {
        let broker = self.broker_module.read().await;
        broker.is_paper_enabled()
    }

    /// Get current trading mode
    pub async fn get_trading_mode(&self) -> crate::config::TradingMode {
        let broker = self.broker_module.read().await;
        broker.get_trading_mode().clone()
    }

    /// Refresh paper broker cache after database reset
    pub async fn refresh_paper_broker_cache(&self) -> Result<()> {
        let mut broker = self.broker_module.write().await;
        broker.refresh_paper_broker_cache().await
    }
}