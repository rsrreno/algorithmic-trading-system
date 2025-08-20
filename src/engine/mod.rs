// src/engine/mod.rs
use anyhow::Result;
use std::sync::{atomic::AtomicU64, Arc};
use tokio::sync::RwLock;
use dashmap::DashMap;
use tracing::{info, debug, error, warn};

use crate::config::Config;
use crate::types::{Position, MarketDataStream};
use crate::broker::{BrokerModule, BrokerEvent};

pub struct TradingEngine {
    config: Config,
    
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
        
        // Initialize broker module
        let mut broker = BrokerModule::new();
        
        // Initialize LightSpeed connection
        broker.initialize_lightspeed(config.lightspeed_config.clone()).await?;
        
        let engine = TradingEngine {
            config,
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
        
        // Start broker event handler only if we need it
        let mut broker_events_task: Option<tokio::task::JoinHandle<()>> = None;
        
        // For now, skip the broker event handler since it's causing issues
        // We'll add it back once we have real events to handle
        // let mut broker_events_task = Some(self.start_broker_event_handler().await?);
        
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
    
    async fn start_broker_event_handler(&self) -> Result<tokio::task::JoinHandle<()>> {
        let broker: Arc<RwLock<BrokerModule>> = Arc::clone(&self.broker);
        let positions = Arc::clone(&self.positions);
        
        // Get broker event stream
        let mut events = {
            let broker_guard = broker.read().await;
            broker_guard.get_broker_events().await?
        };
        
        let task = tokio::spawn(async move {
            while let Some(event) = events.recv().await {
                if let Err(e) = Self::handle_broker_event(event, &positions).await {
                    error!("Error handling broker event: {}", e);
                }
            }
            warn!("Broker event stream ended");
        });
        
        Ok(task)
    }
    
    async fn handle_broker_event(
        event: BrokerEvent,
        positions: &Arc<RwLock<DashMap<String, Position>>>,
    ) -> Result<()> {
        match event {
            BrokerEvent::Connected => {
                info!("Broker connected successfully");
            }
            BrokerEvent::Disconnected => {
                warn!("Broker disconnected");
            }
            BrokerEvent::OrderAck { client_order_id, order_id } => {
                info!("Order acknowledged: {} -> {}", client_order_id, order_id);
            }
            BrokerEvent::OrderFill { client_order_id, symbol, side, qty, price } => {
                info!("Order filled: {} {} {} @ {}", client_order_id, qty, symbol, price);
                
                // Update position tracking
                Self::update_position_from_fill(positions, &symbol, &side, qty, price).await;
            }
            BrokerEvent::OrderReject { client_order_id, reason } => {
                error!("Order rejected: {} - {}", client_order_id, reason);
            }
            BrokerEvent::Error { message } => {
                error!("Broker error: {}", message);
            }
        }
        
        Ok(())
    }
    
    async fn update_position_from_fill(
        positions: &Arc<RwLock<DashMap<String, Position>>>,
        symbol: &str,
        side: &str,
        qty: f64,
        price: f64,
    ) {
        let positions_map = positions.read().await;
        
        // This is a simplified position update - you might want more sophisticated logic
        if let Some(mut position) = positions_map.get_mut(symbol) {
            match side {
                "BUY" => {
                    position.quantity += qty as u64;
                    position.current_price = price;
                }
                "SELL" | "SELL_SHORT" => {
                    if position.quantity >= qty as u64 {
                        position.quantity -= qty as u64;
                    } else {
                        // Going short or reducing position below zero
                        position.quantity = 0;
                    }
                    position.current_price = price;
                }
                _ => {}
            }
        }
        // Drop the reference by ending the scope here
        drop(positions_map);
    }
    
    async fn process_market_data(&self) -> Result<()> {
        // Placeholder for market data processing
        debug!("Processing market data");
        
        // TODO: Integrate with data module to process real-time market data
        // TODO: Update market_streams with new data
        // TODO: Check memory usage and clean up old data if necessary
        
        Ok(())
    }
    
    async fn evaluate_trading_rules(&self) -> Result<()> {
        // Placeholder for rule evaluation
        debug!("Evaluating trading rules");
        
        // TODO: Integrate with rules engine
        // TODO: Check if any trading conditions are met
        // TODO: Place orders through broker if rules trigger
        
        // Example of how to place an order:
        /*
        if some_condition_met {
            let broker = self.broker.read().await;
            let order_id = broker.place_stock_order(
                "AAPL",
                "BUY",
                "MARKET",
                100,
                None, // Market order, no price
            ).await?;
            info!("Placed order: {}", order_id);
        }
        */
        
        Ok(())
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
            // TODO: Implement position closing logic
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
}