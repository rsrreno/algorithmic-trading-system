// src/engine/mod.rs
// Branch: 9.2.25.1

use anyhow::Result;
use std::sync::{atomic::AtomicU64, Arc};
use tokio::sync::RwLock;
use dashmap::DashMap;
use tracing::{info, debug, error, warn};

use crate::config::Config;
use crate::types::{Position, MarketDataStream};
use crate::broker::BrokerModule;
use crate::data::DataModule;
use crate::rules::RulesEngine;

pub struct TradingEngine {
    config: Config,
    
    // Data module for market data
    data_module: DataModule,
    
    // Rules engine for trading decisions
    rules_engine: Option<Arc<RulesEngine>>,
    
    // Memory management
    total_memory_used: AtomicU64,
    max_memory_bytes: u64,
    
    // Active positions and market data
    positions: Arc<RwLock<DashMap<String, Position>>>,
    market_streams: Arc<RwLock<DashMap<String, MarketDataStream>>>,
    
    // Broker integration
    broker: Arc<RwLock<BrokerModule>>,
    
    // Shutdown signal
    shutdown_signal: Arc<tokio::sync::Notify>,
}

impl TradingEngine {
    pub async fn new(config: Config) -> Result<Self> {
        info!("Initializing trading engine");
        
        let max_memory_bytes = config.max_memory_bytes();
        
        // Initialize data module
        let mut data_module = DataModule::new(&config)?;
        info!("✅ Data module initialized");
        
        // Start WebSocket connections if enabled
        if let Err(e) = data_module.start_websocket().await {
            warn!("⚠️ Failed to start WebSocket: {}", e);
        }
        
        // Initialize broker module
        let mut broker = BrokerModule::new();
        
        // Try to initialize LightSpeed connection if enabled
        if config.is_lightspeed_enabled() {
            if let Some(lightspeed_config) = config.lightspeed_config.clone() {
                broker.try_initialize_lightspeed(lightspeed_config).await?;
            }
        } else {
            tracing::info!("🔧 LightSpeed broker disabled in configuration");
        }
        
        let engine = TradingEngine {
            config,
            data_module,
            rules_engine: None, // Will be initialized separately if rules engine is enabled
            total_memory_used: AtomicU64::new(0),
            max_memory_bytes,
            positions: Arc::new(RwLock::new(DashMap::new())),
            market_streams: Arc::new(RwLock::new(DashMap::new())),
            broker: Arc::new(RwLock::new(broker)),
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
    
    pub async fn place_order(
        &self,
        symbol: &str,
        side: &str,
        order_type: &str,
        quantity: u64,
        price: Option<f64>,
    ) -> Result<String> {
        let broker = self.broker.read().await;
        broker.place_stock_order(symbol, side, order_type, quantity, price).await
    }
    
    pub async fn cancel_order(&self, client_order_id: &str) -> Result<()> {
        let broker = self.broker.read().await;
        broker.cancel_order(client_order_id).await
    }
    
    pub async fn get_positions(&self) -> Result<std::collections::HashMap<String, Position>> {
        let broker = self.broker.read().await;
        broker.get_positions().await
    }
    
    pub async fn is_broker_connected(&self) -> bool {
        let broker = self.broker.read().await;
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
        let mut broker = self.broker.write().await;
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
        info!("Rules engine initialization requested with ${:.2} initial cash", initial_cash);
        
        // TODO: Complete integration once data module and broker module expose required clients
        warn!("Rules engine integration is in development - placeholder implementation");
        
        // For now, we'll create a basic rules engine setup without full integration
        // This will be completed in the next phase
        
        info!("⏳ Rules engine integration pending - architecture preparation complete");
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
        self.broker.read().await
    }
}