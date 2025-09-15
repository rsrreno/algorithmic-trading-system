// src/data/mod.rs
// Branch: 14.9.25

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};
use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::{HashMap, HashSet};
use chrono::{DateTime, Utc};

use crate::config::Config;
use crate::market::{MarketSchedule, DataSourceStrategy};

pub mod polygon;
pub mod websocket;
mod cache_stats;

pub use cache_stats::CacheStats;
pub use websocket::{PolygonWebSocket, WebSocketMessage, ConnectionStatus};

/// Shareable WebSocket service for DataModule
/// This separates the WebSocket handling from DataModule to enable Arc sharing
#[derive(Debug)]
pub struct WebSocketService {
    websocket: Option<Arc<RwLock<PolygonWebSocket>>>,
    realtime_cache: Arc<RwLock<HashMap<String, RealtimeTickerData>>>,
}

impl WebSocketService {
    pub fn new(config: &Config) -> Self {
        let websocket = if config.is_polygon_enabled() && 
                          config.polygon_websocket_config.as_ref().map(|c| c.enabled).unwrap_or(false) {
            match PolygonWebSocket::new(config) {
                Ok(ws) => Some(Arc::new(RwLock::new(ws))),
                Err(e) => {
                    warn!("📡 Failed to initialize WebSocket: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Self {
            websocket,
            realtime_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn start_websocket(&self) -> Result<()> {
        if let Some(ref ws) = self.websocket.as_ref() {
            let mut ws_guard = ws.write().await;
            ws_guard.start().await?;
            info!("📡 WebSocket started successfully");
        }
        Ok(())
    }

    pub async fn websocket_subscribe(&self, symbols: Vec<String>) -> Result<()> {
        if let Some(ref ws) = self.websocket.as_ref() {
            let ws_guard = ws.read().await;
            ws_guard.subscribe(symbols).await?;
        } else {
            anyhow::bail!("WebSocket not initialized");
        }
        Ok(())
    }

    pub async fn websocket_unsubscribe(&self, symbols: Vec<String>) -> Result<()> {
        if let Some(ref ws) = self.websocket.as_ref() {
            let ws_guard = ws.read().await;
            ws_guard.unsubscribe(symbols).await?;
        } else {
            anyhow::bail!("WebSocket not initialized");
        }
        Ok(())
    }

    pub async fn websocket_status(&self) -> Option<ConnectionStatus> {
        if let Some(ref ws) = self.websocket.as_ref() {
            let ws_guard = ws.read().await;
            Some(ws_guard.get_status().await)
        } else {
            None
        }
    }

    pub async fn websocket_subscriptions(&self) -> Result<HashSet<String>> {
        if let Some(ref ws) = self.websocket.as_ref() {
            let ws_guard = ws.read().await;
            Ok(ws_guard.get_subscriptions().await)
        } else {
            Ok(HashSet::new())
        }
    }

    pub async fn process_websocket_messages(&self) -> Result<()> {
        if let Some(ref ws) = self.websocket.as_ref() {
            let ws_guard = ws.read().await;
            let messages = ws_guard.get_recent_messages(100).await;
            drop(ws_guard); // Release the lock before processing messages
            
            for message in messages {
                self.update_realtime_cache_from_websocket(message).await?;
            }
        }
        Ok(())
    }

    pub async fn get_current_price(&self, symbol: &str) -> Result<f64> {
        let cache = self.realtime_cache.read().await;
        if let Some(data) = cache.get(symbol) {
            // Check if data is recent (within last 5 minutes)
            if data.cached_at.elapsed() < Duration::from_secs(300) {
                return Ok(data.get_current_price());
            }
        }
        // Default fallback price if no real-time data
        Ok(100.0)
    }

    /// Update real-time cache from WebSocket message
    async fn update_realtime_cache_from_websocket(&self, message: WebSocketMessage) -> Result<()> {
        match message {
            WebSocketMessage::AggregateMinute { sym, c, v, t, .. } => {
                let mut cache = self.realtime_cache.write().await;
                cache.insert(sym.clone(), RealtimeTickerData {
                    symbol: sym,
                    last_price: c,
                    last_volume: v,
                    bid: None,
                    ask: None,
                    last_trade_time: t,
                    cached_at: Instant::now(),
                });
                debug!("📡 Updated real-time cache from aggregate minute data");
            },
            WebSocketMessage::Quote { sym, bp, ap, t, .. } => {
                let mut cache = self.realtime_cache.write().await;
                if let Some(data) = cache.get_mut(&sym) {
                    data.bid = Some(bp);
                    data.ask = Some(ap);
                    data.cached_at = Instant::now();
                    debug!("📡 Updated real-time cache from quote data for {}", sym);
                } else {
                    // Create new entry if doesn't exist
                    cache.insert(sym.clone(), RealtimeTickerData {
                        symbol: sym,
                        last_price: (bp + ap) / 2.0, // Mid price
                        last_volume: 0.0,
                        bid: Some(bp),
                        ask: Some(ap),
                        last_trade_time: t,
                        cached_at: Instant::now(),
                    });
                }
            },
            WebSocketMessage::Trade { sym, p, s, t, .. } => {
                let mut cache = self.realtime_cache.write().await;
                if let Some(data) = cache.get_mut(&sym) {
                    data.last_price = p;
                    data.last_volume = s;
                    data.last_trade_time = t;
                    data.cached_at = Instant::now();
                } else {
                    cache.insert(sym.clone(), RealtimeTickerData {
                        symbol: sym.clone(),
                        last_price: p,
                        last_volume: s,
                        bid: None,
                        ask: None,
                        last_trade_time: t,
                        cached_at: Instant::now(),
                    });
                }
                debug!("📡 Updated real-time cache from trade data for {}", sym);
            },
            _ => {
                debug!("📡 Ignoring WebSocket message: {:?}", message);
            }
        }
        Ok(())
    }
}

/// DataModule now supports Arc sharing by separating WebSocket service
#[derive(Debug, Clone)]
pub struct DataModule {
    client: Client,
    api_key: Option<String>,
    base_url: String,
    polygon_enabled: bool,
    // High-performance in-memory cache for <5ms decision making
    indicator_cache: Arc<RwLock<HashMap<String, CachedIndicator>>>,
    market_data_cache: Arc<RwLock<HashMap<String, CachedMarketData>>>,
    cache_ttl: Duration,
    // Shared WebSocket service for real-time data
    websocket_service: Arc<WebSocketService>,
    // Market schedule for intelligent data source selection
    market_schedule: Arc<RwLock<MarketSchedule>>,
}

/// Cached technical indicator data with timestamp for TTL
#[derive(Debug, Clone)]
struct CachedIndicator {
    symbol: String,
    indicator_type: String, // "sma", "ema", "rsi", "macd"
    data: TechnicalIndicatorResponse,
    cached_at: Instant,
}

/// Cached market data with timestamp for TTL
#[derive(Debug, Clone)]
struct CachedMarketData {
    symbol: String,
    data_type: String, // "snapshot", "previous_day", "minute_agg", "full_market_snapshot", "daily_market_summary"
    timestamp: Instant,
    // Store different data types as needed
    daily_bar: Option<DailyBar>,
    market_movers: Option<Vec<MarketMoverData>>,
    full_market_snapshot: Option<Vec<MarketSnapshotTicker>>,
    daily_market_summary: Option<Vec<DailyMarketSummaryTicker>>,
}

/// Real-time ticker data from WebSocket streams
#[derive(Debug, Clone)]
pub struct RealtimeTickerData {
    symbol: String,
    last_price: f64,
    last_volume: f64,
    bid: Option<f64>,
    ask: Option<f64>,
    last_trade_time: i64,
    cached_at: Instant,
}

impl RealtimeTickerData {
    pub fn get_current_price(&self) -> f64 {
        // Use mid-price (bid+ask)/2 if available, otherwise last_price
        match (self.bid, self.ask) {
            (Some(bid), Some(ask)) => (bid + ask) / 2.0,
            _ => self.last_price,
        }
    }
    
    pub fn last_price(&self) -> f64 {
        self.last_price
    }
    
    pub fn bid(&self) -> Option<f64> {
        self.bid
    }
    
    pub fn ask(&self) -> Option<f64> {
        self.ask
    }
}

/// Smart market data response with automatic source selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartMarketData {
    pub symbol: String,
    pub price: f64,
    pub volume: u64,
    pub data_source: String,       // Description of data source used
    pub timestamp: DateTime<Utc>,  // When data was retrieved
    pub market_date: String,       // YYYY-MM-DD trading date
    pub is_live: bool,            // True for real-time data, false for historical
}

impl DataModule {
    pub fn new(config: &Config) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("TradingSystem/0.1.0")
            .build()
            .context("Failed to create HTTP client")?;

        let polygon_enabled = config.is_polygon_enabled();
        
        // All REST API calls use the same base URL - delayed data is indicated by response status
        let base_url = "https://api.polygon.io".to_string();
        
        // Initialize WebSocket service
        let websocket_service = Arc::new(WebSocketService::new(config));
        
        if polygon_enabled {
            if config.polygon_use_delayed_data {
                info!("✅ Polygon.io data module enabled with delayed data (15-min delay) and in-memory cache");
            } else {
                info!("✅ Polygon.io data module enabled with real-time data and in-memory cache");
            }
            
            if websocket_service.websocket.is_some() {
                info!("📡 WebSocket integration enabled for streaming data");
            } else {
                info!("📡 WebSocket integration disabled");
            }
        } else {
            info!("🔧 Polygon.io data module disabled - no API key provided");
        }

        Ok(DataModule {
            client,
            api_key: config.polygon_api_key.clone(),
            base_url,
            polygon_enabled,
            indicator_cache: Arc::new(RwLock::new(HashMap::new())),
            market_data_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: Duration::from_secs(300), // 5 minutes cache TTL
            websocket_service,
            market_schedule: Arc::new(RwLock::new(MarketSchedule::new())),
        })
    }

    /// Initialize market schedule with database connection
    pub async fn initialize_market_schedule(&mut self, db: &crate::database::Database) -> Result<()> {
        let mut schedule = self.market_schedule.write().await;
        
        // Load cached holidays from database
        schedule.load_holidays_from_cache(db).await?;
        
        // Update holidays if cache is stale
        if schedule.should_refresh_holidays() {
            if let Some(api_key) = &self.api_key {
                if let Err(e) = schedule.refresh_holidays_from_api(db, api_key).await {
                    warn!("Failed to refresh market holidays: {}", e);
                    info!("Using existing holiday cache");
                }
            }
        }
        
        let status = schedule.get_market_status();
        let description = schedule.get_status_description();
        info!("📅 Market Schedule initialized: {}", description);
        info!("📊 Current market status: {:?}", status);
        
        Ok(())
    }

    /// Get current market status and data source strategy
    pub async fn get_market_info(&self) -> Result<DataSourceStrategy> {
        let schedule = self.market_schedule.read().await;
        let strategy = schedule.get_data_source_strategy();
        Ok(strategy)
    }

    /// Smart data retrieval method that automatically selects data source based on market status
    /// Returns the most appropriate data for current market conditions
    pub async fn get_smart_market_data(&self, symbol: &str) -> Result<SmartMarketData> {
        let schedule = self.market_schedule.read().await;
        let strategy = schedule.get_data_source_strategy();
        let description = schedule.get_status_description();
        
        debug!("🧠 Smart data selection for {}: {}", symbol, description);
        
        match strategy {
            DataSourceStrategy::RealTime { session, market_date } => {
                // Market is open - use real-time or current data
                debug!("📈 Market OPEN - fetching real-time data for {}", symbol);
                
                // Try WebSocket real-time data first (if available)
                if let Some(realtime_data) = self.get_realtime_data(symbol).await? {
                    return Ok(SmartMarketData {
                        symbol: symbol.to_string(),
                        price: realtime_data.get_current_price(),
                        volume: realtime_data.last_volume as u64,
                        data_source: format!("Real-time WebSocket ({:?})", session),
                        timestamp: chrono::Utc::now(),
                        market_date,
                        is_live: true,
                    });
                }
                
                // Fallback to REST API current price
                match self.get_current_market_price(symbol).await {
                    Ok(price) => {
                        Ok(SmartMarketData {
                            symbol: symbol.to_string(),
                            price,
                            volume: 0, // Volume not available in this endpoint
                            data_source: format!("REST API snapshot ({:?})", session),
                            timestamp: chrono::Utc::now(),
                            market_date,
                            is_live: true,
                        })
                    },
                    Err(_) => {
                        // Final fallback to previous day if current data unavailable
                        warn!("Current price unavailable for {}, falling back to previous day", symbol);
                        let prev_day = self.get_previous_day_cached(symbol).await?;
                        Ok(SmartMarketData {
                            symbol: symbol.to_string(),
                            price: prev_day.close.unwrap_or(0.0),
                            volume: prev_day.volume.map(|v| v as u64).unwrap_or(0),
                            data_source: "Previous day fallback".to_string(),
                            timestamp: chrono::Utc::now(),
                            market_date,
                            is_live: false,
                        })
                    }
                }
            },
            DataSourceStrategy::PreviousDay { summary_date, reason } => {
                // Market is closed - use previous trading day's summary
                debug!("📊 Market CLOSED - fetching summary data for {} ({})", symbol, summary_date);
                
                // Use daily market summary for the specified date
                let prev_day = self.get_previous_day_cached(symbol).await?;
                Ok(SmartMarketData {
                    symbol: symbol.to_string(),
                    price: prev_day.close.unwrap_or(0.0),
                    volume: prev_day.volume.map(|v| v as u64).unwrap_or(0),
                    data_source: format!("Daily summary - {:?}", reason),
                    timestamp: chrono::Utc::now(),
                    market_date: summary_date,
                    is_live: false,
                })
            }
        }
    }

    /// Get ticker snapshot - uses daily aggregates for basic plan compatibility
    /// This method will be updated to use actual snapshots when plan is upgraded
    pub async fn get_ticker_snapshot(&self, symbol: &str) -> Result<()> {
        if !self.polygon_enabled {
            debug!("Polygon.io disabled - skipping ticker snapshot for {}", symbol);
            return Ok(());
        }
        
        // For basic plan: use daily aggregates for market-appropriate date
        // TODO: Switch to actual snapshot endpoint when upgrading to paid plan
        self.get_market_daily_data(symbol).await
    }

    /// Get market-appropriate daily data for a symbol via API call
    /// Uses market schedule to determine the correct trading date to fetch
    pub async fn get_symbol_data(&self, symbol: &str) -> Result<SymbolDataResponse> {
        if !self.polygon_enabled {
            anyhow::bail!("Polygon.io not available - check configuration and API key");
        }
        
        // Use market schedule to determine appropriate date
        let schedule = self.market_schedule.read().await;
        let strategy = schedule.get_data_source_strategy();
        let target_date = match strategy {
            DataSourceStrategy::RealTime { market_date, .. } => {
                // Market is open - get current trading day data (but this will likely be incomplete)
                // For daily aggregates, we should still get the previous complete day
                // since current day aggregates may not be complete until market close
                let schedule_time = schedule.current_time.with_timezone(&chrono_tz::US::Eastern);
                if schedule_time.time() < chrono::NaiveTime::from_hms_opt(16, 0, 0).unwrap() {
                    // Before 4 PM ET - use previous trading day for complete data
                    let previous_date = chrono::Utc::now() - chrono::Duration::days(1);
                    previous_date.format("%Y-%m-%d").to_string()
                } else {
                    // After 4 PM ET - current day data should be available
                    market_date
                }
            },
            DataSourceStrategy::PreviousDay { summary_date, .. } => {
                // Market is closed - use the determined summary date
                summary_date
            }
        };
        drop(schedule);
        
        let url = format!(
            "{}/v2/aggs/ticker/{}/range/1/day/{}/{}",
            self.base_url, symbol, target_date, target_date
        );

        info!("API request: Fetching market-appropriate daily data for {} ({})", symbol, target_date);
        debug!("Request URL: {}", url);

        let response = self
            .client
            .get(&url)
            .query(&[("apikey", self.api_key.as_ref().unwrap().as_str())])
            .send()
            .await
            .context("Failed to send request to Polygon API")?;

        let status = response.status();
        debug!("Response status: {}", status);

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            error!("API request failed with status {}: {}", status, error_text);
            anyhow::bail!("API request failed with status {}: {}", status, error_text);
        }

        let response_text = response
            .text()
            .await
            .context("Failed to read response body")?;

        debug!("Raw response: {}", response_text);

        let api_response: DailyAggregatesResponse = serde_json::from_str(&response_text)
            .context("Failed to parse JSON response")?;

        // Accept both OK and DELAYED status - DELAYED is expected for Stock Starter plan
        if api_response.status != "OK" && api_response.status != "DELAYED" {
            error!("API returned error status: {}", api_response.status);
            anyhow::bail!("API returned error status: {}", api_response.status);
        }
        
        if api_response.status == "DELAYED" {
            debug!("Received delayed data for {} (15-minute delay as expected on Stock Starter plan)", symbol);
        }

        if let Some(results) = api_response.results {
            if !results.is_empty() {
                let bar = &results[0];
                
                // Calculate daily change
                let open = bar.open.unwrap_or(0.0);
                let close = bar.close.unwrap_or(0.0);
                let change = if open > 0.0 { close - open } else { 0.0 };
                let change_percent = if open > 0.0 { (change / open) * 100.0 } else { 0.0 };
                
                info!("API response: {} - ${:.2} ({:+.2}%)", symbol, close, change_percent);
                
                Ok(SymbolDataResponse {
                    symbol: symbol.to_string(),
                    date: target_date,
                    open: bar.open.unwrap_or(0.0),
                    high: bar.high.unwrap_or(0.0),
                    low: bar.low.unwrap_or(0.0),
                    close: bar.close.unwrap_or(0.0),
                    volume: bar.volume.unwrap_or(0.0) as u64,
                    vwap: bar.vwap.unwrap_or(0.0),
                    transactions: bar.transactions.unwrap_or(0),
                    change,
                    change_percent,
                })
            } else {
                anyhow::bail!("No daily data available for {} on {} (market may be closed)", symbol, target_date);
            }
        } else {
            anyhow::bail!("No daily data found for {} on {}", symbol, target_date);
        }
    }

    /// Get current market price using the snapshot API
    pub async fn get_current_market_price(&self, symbol: &str) -> Result<f64> {
        let url = format!(
            "{}/v2/snapshot/locale/us/markets/stocks/tickers/{}",
            self.base_url, symbol
        );

        debug!("Fetching current price for {} from snapshot API", symbol);

        let response = self
            .client
            .get(&url)
            .query(&[("apikey", self.api_key.as_ref().unwrap().as_str())])
            .send()
            .await
            .context("Failed to send snapshot request to Polygon API")?;

        if !response.status().is_success() {
            anyhow::bail!("Snapshot API request failed with status: {}", response.status());
        }

        let json_text = response.text().await?;
        let snapshot_response: serde_json::Value = serde_json::from_str(&json_text)
            .context("Failed to parse snapshot response")?;

        // Extract current price from ticker.day.c (current day close) or ticker.min.c (latest minute)
        if let Some(ticker) = snapshot_response.get("ticker") {
            // Try current day close first
            if let Some(day_close) = ticker.get("day").and_then(|d| d.get("c")).and_then(|c| c.as_f64()) {
                if day_close > 0.0 {
                    debug!("Using current day price: ${:.2} for {}", day_close, symbol);
                    return Ok(day_close);
                }
            }
            
            // Fallback to latest minute close
            if let Some(min_close) = ticker.get("min").and_then(|m| m.get("c")).and_then(|c| c.as_f64()) {
                if min_close > 0.0 {
                    debug!("Using latest minute price: ${:.2} for {}", min_close, symbol);
                    return Ok(min_close);
                }
            }
        }

        // If no current data, fall back to previous day method
        warn!("No current price data available for {}, falling back to previous day", symbol);
        let prev_day_data = self.get_previous_day_cached(symbol).await?;
        Ok(prev_day_data.close.unwrap_or(100.0))
    }

    /// Get ticker snapshot using the premium API endpoint
    /// This method will be used when upgrading to a paid plan
    pub async fn get_ticker_snapshot_premium(&self, symbol: &str) -> Result<()> {
        let url = format!(
            "{}/v2/snapshot/locale/us/markets/stocks/tickers/{}",
            self.base_url, symbol
        );

        info!("Fetching ticker snapshot for {}", symbol);
        debug!("Request URL: {}", url);

        let response = self
            .client
            .get(&url)
            .query(&[("apikey", self.api_key.as_ref().unwrap().as_str())])
            .send()
            .await
            .context("Failed to send request to Polygon API")?;

        let status = response.status();
        debug!("Response status: {}", status);

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            error!("API request failed with status {}: {}", status, error_text);
            anyhow::bail!("API request failed with status {}: {}", status, error_text);
        }

        let response_text = response
            .text()
            .await
            .context("Failed to read response body")?;

        debug!("Raw response: {}", response_text);

        let api_response: PolygonApiResponse = serde_json::from_str(&response_text)
            .context("Failed to parse JSON response")?;

        // Accept both OK and DELAYED status - DELAYED is expected for Stock Starter plan
        if api_response.status != "OK" && api_response.status != "DELAYED" {
            error!("API returned error status: {}", api_response.status);
            anyhow::bail!("API returned error status: {}", api_response.status);
        }
        
        if api_response.status == "DELAYED" {
            debug!("Received delayed data for {} (15-minute delay as expected on Stock Starter plan)", symbol);
        }

        if let Some(ticker_data) = api_response.results {
            info!("=== {} Stock Data ===", symbol);
            info!("Symbol: {}", ticker_data.ticker.as_deref().unwrap_or("N/A"));
            
            if let Some(day) = &ticker_data.day {
                info!("Current Price: ${:.2}", day.close.unwrap_or(0.0));
                info!("Daily Change: ${:.2}", day.change.unwrap_or(0.0));
                info!("Daily Change %: {:.2}%", day.change_percent.unwrap_or(0.0));
                info!("Volume: {}", day.volume.unwrap_or(0));
                info!("High: ${:.2}", day.high.unwrap_or(0.0));
                info!("Low: ${:.2}", day.low.unwrap_or(0.0));
                info!("Open: ${:.2}", day.open.unwrap_or(0.0));
            }
            
            if let Some(last_quote) = &ticker_data.last_quote {
                info!("Bid: ${:.2}", last_quote.bid.unwrap_or(0.0));
                info!("Ask: ${:.2}", last_quote.ask.unwrap_or(0.0));
                info!("Spread: ${:.2}", 
                    last_quote.ask.unwrap_or(0.0) - last_quote.bid.unwrap_or(0.0));
            }
            
            info!("Market Cap: ${:.0}", ticker_data.market_cap.unwrap_or(0.0));
            info!("Updated: {}", ticker_data.updated.as_deref().unwrap_or("N/A"));
            info!("=========================");
        } else {
            error!("No ticker data found in response");
        }

        Ok(())
    }

    /// Test API connection using market status endpoint (always available)
    pub async fn test_connection(&self) -> Result<()> {
        info!("Testing Polygon.io API connection...");
        match self.get_market_status().await {
            Ok(status) => {
                info!("✅ API connection successful - Market is currently: {}", 
                    status.market.as_deref().unwrap_or("unknown"));
                if let Some(server_time) = &status.server_time {
                    info!("📅 Server time: {}", server_time);
                }
                Ok(())
            }
            Err(e) => {
                anyhow::bail!("❌ API connection failed: {}", e);
            }
        }
    }

    // ===============================
    // Technical Indicators API Implementation
    // ===============================

    /// Get SMA with caching for <5ms performance
    /// Checks cache first, then fetches from API if needed
    pub async fn get_sma_cached(&self, symbol: &str, window: u32, timespan: &str, timestamp: &str) -> Result<TechnicalIndicatorResponse> {
        let cache_key = format!("{}:sma:{}:{}:{}", symbol, window, timespan, timestamp);
        
        // Check cache first
        {
            let cache = self.indicator_cache.read().await;
            if let Some(cached) = cache.get(&cache_key) {
                if cached.cached_at.elapsed() < self.cache_ttl {
                    debug!("Cache HIT for SMA {}: {:?}", symbol, cached.cached_at.elapsed());
                    return Ok(cached.data.clone());
                }
            }
        }
        
        // Cache miss - fetch from API
        debug!("Cache MISS for SMA {} - fetching from API", symbol);
        let result = self.get_sma(symbol, window, timespan, timestamp).await?;
        
        // Update cache
        {
            let mut cache = self.indicator_cache.write().await;
            cache.insert(cache_key, CachedIndicator {
                symbol: symbol.to_string(),
                indicator_type: "sma".to_string(),
                data: result.clone(),
                cached_at: Instant::now(),
            });
        }
        
        Ok(result)
    }

    /// Get EMA with caching for <5ms performance
    pub async fn get_ema_cached(&self, symbol: &str, window: u32, timespan: &str, timestamp: &str) -> Result<TechnicalIndicatorResponse> {
        let cache_key = format!("{}:ema:{}:{}:{}", symbol, window, timespan, timestamp);
        
        // Check cache first
        {
            let cache = self.indicator_cache.read().await;
            if let Some(cached) = cache.get(&cache_key) {
                if cached.cached_at.elapsed() < self.cache_ttl {
                    debug!("Cache HIT for EMA {}: {:?}", symbol, cached.cached_at.elapsed());
                    return Ok(cached.data.clone());
                }
            }
        }
        
        // Cache miss - fetch from API
        debug!("Cache MISS for EMA {} - fetching from API", symbol);
        let result = self.get_ema(symbol, window, timespan, timestamp).await?;
        
        // Update cache
        {
            let mut cache = self.indicator_cache.write().await;
            cache.insert(cache_key, CachedIndicator {
                symbol: symbol.to_string(),
                indicator_type: "ema".to_string(),
                data: result.clone(),
                cached_at: Instant::now(),
            });
        }
        
        Ok(result)
    }

    /// Get RSI with caching for <5ms performance
    pub async fn get_rsi_cached(&self, symbol: &str, window: u32, timespan: &str, timestamp: &str) -> Result<TechnicalIndicatorResponse> {
        let cache_key = format!("{}:rsi:{}:{}:{}", symbol, window, timespan, timestamp);
        
        // Check cache first
        {
            let cache = self.indicator_cache.read().await;
            if let Some(cached) = cache.get(&cache_key) {
                if cached.cached_at.elapsed() < self.cache_ttl {
                    debug!("Cache HIT for RSI {}: {:?}", symbol, cached.cached_at.elapsed());
                    return Ok(cached.data.clone());
                }
            }
        }
        
        // Cache miss - fetch from API
        debug!("Cache MISS for RSI {} - fetching from API", symbol);
        let result = self.get_rsi(symbol, window, timespan, timestamp).await?;
        
        // Update cache
        {
            let mut cache = self.indicator_cache.write().await;
            cache.insert(cache_key, CachedIndicator {
                symbol: symbol.to_string(),
                indicator_type: "rsi".to_string(),
                data: result.clone(),
                cached_at: Instant::now(),
            });
        }
        
        Ok(result)
    }

    /// Cache management - clear expired entries
    pub async fn clean_cache(&self) -> Result<()> {
        let mut removed_count = 0;
        
        // Clean indicator cache
        {
            let mut cache = self.indicator_cache.write().await;
            cache.retain(|_key, cached| {
                let expired = cached.cached_at.elapsed() >= self.cache_ttl;
                if expired {
                    removed_count += 1;
                }
                !expired
            });
        }
        
        // Clean market data cache
        {
            let mut cache = self.market_data_cache.write().await;
            cache.retain(|_key, cached| {
                let expired = cached.timestamp.elapsed() >= self.cache_ttl;
                if expired {
                    removed_count += 1;
                }
                !expired
            });
        }
        
        if removed_count > 0 {
            debug!("Cleaned {} expired cache entries", removed_count);
        }
        
        Ok(())
    }

    /// Get cache statistics for monitoring
    pub async fn get_cache_stats(&self) -> Result<CacheStats> {
        let indicator_count = self.indicator_cache.read().await.len();
        let market_data_count = self.market_data_cache.read().await.len();
        
        Ok(CacheStats {
            indicator_entries: indicator_count,
            market_data_entries: market_data_count,
            cache_ttl_seconds: self.cache_ttl.as_secs(),
        })
    }

    // ===============================
    // WebSocket Integration Methods
    // ===============================

    /// Start WebSocket connections and subscribe to default symbols
    pub async fn start_websocket(&mut self) -> Result<()> {
        if let Some(ref ws) = self.websocket_service.websocket.as_ref() {
            ws.write().await.start().await?;
            info!("📡 WebSocket started successfully");
        }
        Ok(())
    }

    /// Subscribe to symbols via WebSocket (non-blocking)
    pub async fn websocket_subscribe(&self, symbols: Vec<String>) -> Result<()> {
        if let Some(ref ws) = self.websocket_service.websocket.as_ref() {
            ws.write().await.subscribe(symbols).await?;
        } else {
            anyhow::bail!("WebSocket not initialized");
        }
        Ok(())
    }

    /// Unsubscribe from symbols via WebSocket (non-blocking)
    pub async fn websocket_unsubscribe(&self, symbols: Vec<String>) -> Result<()> {
        if let Some(ref ws) = self.websocket_service.websocket.as_ref() {
            ws.write().await.unsubscribe(symbols).await?;
        } else {
            anyhow::bail!("WebSocket not initialized");
        }
        Ok(())
    }

    /// Get WebSocket connection status
    pub async fn websocket_status(&self) -> Option<ConnectionStatus> {
        if let Some(ref ws) = self.websocket_service.websocket.as_ref() {
            Some(ws.read().await.get_status().await)
        } else {
            None
        }
    }

    /// Get current WebSocket subscriptions
    pub async fn websocket_subscriptions(&self) -> Result<HashSet<String>> {
        if let Some(ref ws) = self.websocket_service.websocket.as_ref() {
            Ok(ws.read().await.get_subscriptions().await)
        } else {
            Ok(HashSet::new())
        }
    }

    /// Process WebSocket messages and update real-time cache (called periodically)
    pub async fn process_websocket_messages(&self) -> Result<()> {
        if let Some(ref ws) = self.websocket_service.websocket.as_ref() {
            let messages: Vec<crate::data::websocket::WebSocketMessage> = ws.read().await.get_recent_messages(100).await;
            
            for message in messages {
                self.update_realtime_cache_from_websocket(message).await?;
            }
        }
        Ok(())
    }

    /// Update real-time cache from WebSocket message
    async fn update_realtime_cache_from_websocket(&self, message: WebSocketMessage) -> Result<()> {
        match message {
            WebSocketMessage::AggregateMinute { sym, c, v, t, .. } => {
                let mut cache = self.websocket_service.realtime_cache.write().await;
                cache.insert(sym.clone(), RealtimeTickerData {
                    symbol: sym,
                    last_price: c,
                    last_volume: v,
                    bid: None,
                    ask: None,
                    last_trade_time: t,
                    cached_at: Instant::now(),
                });
                debug!("📡 Updated real-time cache from aggregate minute data");
            },
            WebSocketMessage::Quote { sym, bp, ap, t, .. } => {
                let mut cache = self.websocket_service.realtime_cache.write().await;
                if let Some(data) = cache.get_mut(&sym) {
                    data.bid = Some(bp);
                    data.ask = Some(ap);
                    data.cached_at = Instant::now();
                    debug!("📡 Updated real-time cache from quote data for {}", sym);
                } else {
                    // Create new entry if doesn't exist
                    cache.insert(sym.clone(), RealtimeTickerData {
                        symbol: sym,
                        last_price: (bp + ap) / 2.0, // Mid price
                        last_volume: 0.0,
                        bid: Some(bp),
                        ask: Some(ap),
                        last_trade_time: t,
                        cached_at: Instant::now(),
                    });
                }
            },
            WebSocketMessage::Trade { sym, p, s, t, .. } => {
                let mut cache = self.websocket_service.realtime_cache.write().await;
                if let Some(data) = cache.get_mut(&sym) {
                    data.last_price = p;
                    data.last_volume = s;
                    data.last_trade_time = t;
                    data.cached_at = Instant::now();
                    debug!("📡 Updated real-time cache from trade data for {}", sym);
                } else {
                    // Create new entry if doesn't exist
                    cache.insert(sym.clone(), RealtimeTickerData {
                        symbol: sym,
                        last_price: p,
                        last_volume: s,
                        bid: None,
                        ask: None,
                        last_trade_time: t,
                        cached_at: Instant::now(),
                    });
                }
            },
            _ => {} // Handle other message types as needed
        }
        Ok(())
    }

    /// Get real-time data for a symbol (WebSocket cache first, fallback to REST)
    pub async fn get_realtime_data(&self, symbol: &str) -> Result<Option<RealtimeTickerData>> {
        // Check real-time cache first
        {
            let cache = self.websocket_service.realtime_cache.read().await;
            if let Some(data) = cache.get(symbol) {
                // Check if data is fresh (within cache TTL)
                if data.cached_at.elapsed() < self.cache_ttl {
                    return Ok(Some(data.clone()));
                }
            }
        }
        
        // Fallback to REST API if no WebSocket data available
        debug!("📡 No fresh real-time data for {}, falling back to REST API", symbol);
        Ok(None)
    }

    /// Get Simple Moving Average (SMA) for a symbol
    /// Uses confirmed working Polygon endpoint: /v1/indicators/sma/{ticker}
    pub async fn get_sma(&self, symbol: &str, window: u32, timespan: &str, timestamp: &str) -> Result<TechnicalIndicatorResponse> {
        if !self.polygon_enabled {
            anyhow::bail!("Polygon.io not available - check configuration and API key");
        }

        let url = format!("{}/v1/indicators/sma/{}", self.base_url, symbol);
        
        let response = self
            .client
            .get(&url)
            .query(&[
                ("apikey", self.api_key.as_ref().unwrap().as_str()),
                ("timestamp", timestamp),
                ("timespan", timespan),
                ("window", &window.to_string()),
                ("series_type", "close"),
                ("order", "desc"),
                ("limit", "10")
            ])
            .send()
            .await
            .context("Failed to fetch SMA data")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("SMA request failed: {}", error_text);
        }

        let indicator_response: TechnicalIndicatorResponse = response
            .json()
            .await
            .context("Failed to parse SMA response")?;

        debug!("SMA for {}: {:?}", symbol, indicator_response);
        Ok(indicator_response)
    }

    /// Get Exponential Moving Average (EMA) for a symbol
    /// Uses confirmed working Polygon endpoint: /v1/indicators/ema/{ticker}
    pub async fn get_ema(&self, symbol: &str, window: u32, timespan: &str, timestamp: &str) -> Result<TechnicalIndicatorResponse> {
        if !self.polygon_enabled {
            anyhow::bail!("Polygon.io not available - check configuration and API key");
        }

        let url = format!("{}/v1/indicators/ema/{}", self.base_url, symbol);
        
        let response = self
            .client
            .get(&url)
            .query(&[
                ("apikey", self.api_key.as_ref().unwrap().as_str()),
                ("timestamp", timestamp),
                ("timespan", timespan),
                ("window", &window.to_string()),
                ("series_type", "close"),
                ("order", "desc"),
                ("limit", "10")
            ])
            .send()
            .await
            .context("Failed to fetch EMA data")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("EMA request failed: {}", error_text);
        }

        let indicator_response: TechnicalIndicatorResponse = response
            .json()
            .await
            .context("Failed to parse EMA response")?;

        debug!("EMA for {}: {:?}", symbol, indicator_response);
        Ok(indicator_response)
    }

    /// Get Relative Strength Index (RSI) for a symbol
    /// Uses confirmed working Polygon endpoint: /v1/indicators/rsi/{ticker}
    pub async fn get_rsi(&self, symbol: &str, window: u32, timespan: &str, timestamp: &str) -> Result<TechnicalIndicatorResponse> {
        if !self.polygon_enabled {
            anyhow::bail!("Polygon.io not available - check configuration and API key");
        }

        let url = format!("{}/v1/indicators/rsi/{}", self.base_url, symbol);
        
        let response = self
            .client
            .get(&url)
            .query(&[
                ("apikey", self.api_key.as_ref().unwrap().as_str()),
                ("timestamp", timestamp),
                ("timespan", timespan),
                ("window", &window.to_string()),
                ("series_type", "close"),
                ("order", "desc"),
                ("limit", "10")
            ])
            .send()
            .await
            .context("Failed to fetch RSI data")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("RSI request failed: {}", error_text);
        }

        let indicator_response: TechnicalIndicatorResponse = response
            .json()
            .await
            .context("Failed to parse RSI response")?;

        debug!("RSI for {}: {:?}", symbol, indicator_response);
        Ok(indicator_response)
    }

    /// Get Moving Average Convergence Divergence (MACD) for a symbol
    /// Uses confirmed working Polygon endpoint: /v1/indicators/macd/{ticker}
    pub async fn get_macd(&self, symbol: &str, short_window: u32, long_window: u32, signal_window: u32, timespan: &str, timestamp: &str) -> Result<MacdResponse> {
        if !self.polygon_enabled {
            anyhow::bail!("Polygon.io not available - check configuration and API key");
        }

        let url = format!("{}/v1/indicators/macd/{}", self.base_url, symbol);
        
        let response = self
            .client
            .get(&url)
            .query(&[
                ("apikey", self.api_key.as_ref().unwrap().as_str()),
                ("timestamp", timestamp),
                ("timespan", timespan),
                ("short_window", &short_window.to_string()),
                ("long_window", &long_window.to_string()),
                ("signal_window", &signal_window.to_string()),
                ("series_type", "close"),
                ("order", "desc"),
                ("limit", "10")
            ])
            .send()
            .await
            .context("Failed to fetch MACD data")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("MACD request failed: {}", error_text);
        }

        let indicator_response: MacdResponse = response
            .json()
            .await
            .context("Failed to parse MACD response")?;

        debug!("MACD for {}: {:?}", symbol, indicator_response);
        Ok(indicator_response)
    }

    /// Get market-appropriate daily data for a symbol (compatible with basic plan) - for internal testing
    /// Uses market schedule to determine correct trading date
    pub async fn get_market_daily_data(&self, symbol: &str) -> Result<()> {
        match self.get_symbol_data(symbol).await {
            Ok(data) => {
                info!("=== {} Daily Data ({}) ===", data.symbol, data.date);
                info!("Date: {}", data.date);
                info!("Open: ${:.2}", data.open);
                info!("High: ${:.2}", data.high);
                info!("Low: ${:.2}", data.low);
                info!("Close: ${:.2}", data.close);
                info!("Volume: {}", data.volume);
                info!("VWAP: ${:.2}", data.vwap);
                info!("Transactions: {}", data.transactions);
                info!("Daily Change: ${:.2} ({:.2}%)", data.change, data.change_percent);
                info!("=========================");
                Ok(())
            }
            Err(e) => Err(e)
        }
    }

    /// Get daily aggregates for a symbol (available on basic plan)
    pub async fn get_daily_aggregates(&self, symbol: &str, from: &str, to: &str) -> Result<Vec<DailyBar>> {
        let url = format!(
            "{}/v2/aggs/ticker/{}/range/1/day/{}/{}",
            self.base_url, symbol, from, to
        );

        debug!("Fetching daily aggregates: {}", url);

        let response = self
            .client
            .get(&url)
            .query(&[("apikey", self.api_key.as_ref().unwrap().as_str())])
            .send()
            .await
            .context("Failed to fetch daily aggregates")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Daily aggregates request failed: {}", error_text);
        }

        let api_response: DailyAggregatesResponse = response
            .json()
            .await
            .context("Failed to parse daily aggregates response")?;

        Ok(api_response.results.unwrap_or_default())
    }

    // ===============================
    // Market Data API Implementation
    // ===============================

    /// Get minute-level aggregates for intraday analysis
    /// Uses confirmed working Polygon endpoint: /v2/aggs/ticker/{ticker}/range/1/minute/{from}/{to}
    pub async fn get_minute_aggregates(&self, symbol: &str, from: &str, to: &str) -> Result<Vec<MinuteBar>> {
        if !self.polygon_enabled {
            anyhow::bail!("Polygon.io not available - check configuration and API key");
        }

        let url = format!(
            "{}/v2/aggs/ticker/{}/range/1/minute/{}/{}",
            self.base_url, symbol, from, to
        );

        debug!("Fetching minute aggregates: {}", url);

        let response = self
            .client
            .get(&url)
            .query(&[("apikey", self.api_key.as_ref().unwrap().as_str())])
            .send()
            .await
            .context("Failed to fetch minute aggregates")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Minute aggregates request failed: {}", error_text);
        }

        let api_response: MinuteAggregatesResponse = response
            .json()
            .await
            .context("Failed to parse minute aggregates response")?;

        Ok(api_response.results.unwrap_or_default())
    }

    /// Get previous day aggregates for a symbol
    /// Uses confirmed working Polygon endpoint: /v2/aggs/ticker/{ticker}/prev
    pub async fn get_previous_day_aggregates(&self, symbol: &str) -> Result<DailyBar> {
        if !self.polygon_enabled {
            anyhow::bail!("Polygon.io not available - check configuration and API key");
        }

        let url = format!("{}/v2/aggs/ticker/{}/prev", self.base_url, symbol);

        debug!("Fetching previous day aggregates: {}", url);

        let response = self
            .client
            .get(&url)
            .query(&[("apikey", self.api_key.as_ref().unwrap().as_str())])
            .send()
            .await
            .context("Failed to fetch previous day aggregates")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Previous day aggregates request failed: {}", error_text);
        }

        let api_response: DailyAggregatesResponse = response
            .json()
            .await
            .context("Failed to parse previous day aggregates response")?;

        let results = api_response.results.ok_or_else(|| anyhow::anyhow!("No previous day data available for {}", symbol))?;
        
        if results.is_empty() {
            anyhow::bail!("No previous day data found for {}", symbol);
        }

        Ok(results[0].clone())
    }

    /// Get top market movers (gainers or losers)
    /// Uses confirmed working Polygon endpoints: /v2/snapshot/locale/us/markets/stocks/gainers|losers
    pub async fn get_market_movers(&self, direction: &str) -> Result<Vec<MarketMoverData>> {
        if !self.polygon_enabled {
            anyhow::bail!("Polygon.io not available - check configuration and API key");
        }

        let endpoint = match direction.to_lowercase().as_str() {
            "gainers" => "gainers",
            "losers" => "losers",
            _ => anyhow::bail!("Direction must be 'gainers' or 'losers'"),
        };

        let url = format!("{}/v2/snapshot/locale/us/markets/stocks/{}", self.base_url, endpoint);

        debug!("Fetching market movers: {}", url);

        let response = self
            .client
            .get(&url)
            .query(&[("apikey", self.api_key.as_ref().unwrap().as_str())])
            .send()
            .await
            .context("Failed to fetch market movers")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Market movers request failed: {}", error_text);
        }

        let api_response: MarketMoversResponse = response
            .json()
            .await
            .context("Failed to parse market movers response")?;

        Ok(api_response.results.unwrap_or_default())
    }

    /// Get market movers with caching for performance
    pub async fn get_market_movers_cached(&self, direction: &str) -> Result<Vec<MarketMoverData>> {
        let cache_key = format!("market_movers:{}", direction);
        
        // Check cache first
        {
            let cache = self.market_data_cache.read().await;
            if let Some(cached) = cache.get(&cache_key) {
                if cached.timestamp.elapsed() < self.cache_ttl {
                    if let Some(movers) = &cached.market_movers {
                        debug!("Cache HIT for market movers {}: {:?}", direction, cached.timestamp.elapsed());
                        return Ok(movers.clone());
                    }
                }
            }
        }
        
        // Cache miss - fetch from API
        debug!("Cache MISS for market movers {} - fetching from API", direction);
        let result = self.get_market_movers(direction).await?;
        
        // Update cache
        {
            let mut cache = self.market_data_cache.write().await;
            cache.insert(cache_key, CachedMarketData {
                symbol: "*".to_string(), // Global data
                data_type: format!("movers_{}", direction),
                timestamp: Instant::now(),
                daily_bar: None,
                market_movers: Some(result.clone()),
                full_market_snapshot: None,
                daily_market_summary: None,
            });
        }
        
        Ok(result)
    }

    /// Get market status
    /// Uses confirmed working Polygon endpoint: /v1/marketstatus/now  
    pub async fn get_market_status(&self) -> Result<MarketStatusResponse> {
        if !self.polygon_enabled {
            anyhow::bail!("Polygon.io not available - check configuration and API key");
        }

        let url = format!("{}/v1/marketstatus/now", self.base_url);

        debug!("Fetching market status: {}", url);

        let response = self
            .client
            .get(&url)
            .query(&[("apikey", self.api_key.as_ref().unwrap().as_str())])
            .send()
            .await
            .context("Failed to fetch market status")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Market status request failed: {}", error_text);
        }

        let market_status: MarketStatusResponse = response
            .json()
            .await
            .context("Failed to parse market status response")?;

        Ok(market_status)
    }

    /// Get news for a specific ticker with sentiment analysis
    /// Uses confirmed working Polygon endpoint: /v2/reference/news
    pub async fn get_news(&self, symbol: Option<&str>, limit: u32) -> Result<Vec<NewsArticle>> {
        if !self.polygon_enabled {
            anyhow::bail!("Polygon.io not available - check configuration and API key");
        }

        let url = format!("{}/v2/reference/news", self.base_url);
        
        let limit_str = limit.to_string();
        let mut query_params = vec![
            ("apikey", self.api_key.as_ref().unwrap().as_str()),
            ("limit", &limit_str),
            ("order", "desc"),
        ];
        
        if let Some(ticker) = symbol {
            query_params.push(("ticker", ticker));
        }

        debug!("Fetching news: {}", url);

        let response = self
            .client
            .get(&url)
            .query(&query_params)
            .send()
            .await
            .context("Failed to fetch news")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("News request failed: {}", error_text);
        }

        let api_response: NewsResponse = response
            .json()
            .await
            .context("Failed to parse news response")?;

        Ok(api_response.results.unwrap_or_default())
    }

    /// Get previous day data with caching for performance
    pub async fn get_previous_day_cached(&self, symbol: &str) -> Result<DailyBar> {
        let cache_key = format!("{}:prev_day", symbol);
        
        // Check cache first
        {
            let cache = self.market_data_cache.read().await;
            if let Some(cached) = cache.get(&cache_key) {
                if cached.timestamp.elapsed() < self.cache_ttl {
                    if let Some(daily_bar) = &cached.daily_bar {
                        debug!("Cache HIT for previous day {}: {:?}", symbol, cached.timestamp.elapsed());
                        return Ok(daily_bar.clone());
                    }
                }
            }
        }
        
        // Cache miss - fetch from API
        debug!("Cache MISS for previous day {} - fetching from API", symbol);
        let result = self.get_previous_day_aggregates(symbol).await?;
        
        // Update cache
        {
            let mut cache = self.market_data_cache.write().await;
            cache.insert(cache_key, CachedMarketData {
                symbol: symbol.to_string(),
                data_type: "previous_day".to_string(),
                timestamp: Instant::now(),
                daily_bar: Some(result.clone()),
                market_movers: None,
                full_market_snapshot: None,
                daily_market_summary: None,
            });
        }
        
        Ok(result)
    }

    /// Get all US stock tickers from Polygon
    /// Uses confirmed working Polygon endpoint: /v3/reference/tickers
    pub async fn get_all_tickers(&self, limit: u32) -> Result<Vec<TickerInfo>> {
        if !self.polygon_enabled {
            anyhow::bail!("Polygon.io not available - check configuration and API key");
        }

        let url = format!("{}/v3/reference/tickers", self.base_url);
        
        let limit_str = limit.to_string();
        let query_params = vec![
            ("apikey", self.api_key.as_ref().unwrap().as_str()),
            ("market", "stocks"),
            ("active", "true"),
            ("limit", &limit_str),
            ("order", "asc"),
        ];

        debug!("Fetching tickers: {}", url);

        let response = self
            .client
            .get(&url)
            .query(&query_params)
            .send()
            .await
            .context("Failed to fetch tickers")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Tickers request failed: {}", error_text);
        }

        let api_response: TickersResponse = response
            .json()
            .await
            .context("Failed to parse tickers response")?;

        Ok(api_response.results.unwrap_or_default())
    }

    /// Get stock exchanges from Polygon
    /// Uses confirmed working Polygon endpoint: /v3/reference/exchanges
    pub async fn get_exchanges(&self) -> Result<Vec<ExchangeInfo>> {
        if !self.polygon_enabled {
            anyhow::bail!("Polygon.io not available - check configuration and API key");
        }

        let url = format!("{}/v3/reference/exchanges", self.base_url);
        
        let query_params = vec![
            ("apikey", self.api_key.as_ref().unwrap().as_str()),
            ("market", "stocks"),
            ("asset_class", "stocks"),
        ];

        debug!("Fetching exchanges: {}", url);

        let response = self
            .client
            .get(&url)
            .query(&query_params)
            .send()
            .await
            .context("Failed to fetch exchanges")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Exchanges request failed: {}", error_text);
        }

        let api_response: ExchangesResponse = response
            .json()
            .await
            .context("Failed to parse exchanges response")?;

        Ok(api_response.results.unwrap_or_default())
    }

    /// Get stock splits for a specific ticker from Polygon
    /// Uses confirmed working Polygon endpoint: /v3/reference/splits
    pub async fn get_stock_splits(&self, symbol: &str) -> Result<Vec<SplitInfo>> {
        if !self.polygon_enabled {
            anyhow::bail!("Polygon.io not available - check configuration and API key");
        }

        let url = format!("{}/v3/reference/splits", self.base_url);
        
        let query_params = vec![
            ("apikey", self.api_key.as_ref().unwrap().as_str()),
            ("ticker", symbol),
            ("order", "desc"),
            ("limit", "10"),
        ];

        debug!("Fetching splits for {}: {}", symbol, url);

        let response = self
            .client
            .get(&url)
            .query(&query_params)
            .send()
            .await
            .context("Failed to fetch stock splits")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Stock splits request failed: {}", error_text);
        }

        let api_response: SplitsResponse = response
            .json()
            .await
            .context("Failed to parse splits response")?;

        Ok(api_response.results.unwrap_or_default())
    }

    /// Get full market snapshot for all US stocks
    /// Uses Polygon endpoint: /v2/snapshot/locale/us/markets/stocks
    /// NOTE: Returns empty results when markets are closed
    pub async fn get_full_market_snapshot(&self) -> Result<Vec<MarketSnapshotTicker>> {
        if !self.polygon_enabled {
            anyhow::bail!("Polygon.io not available - check configuration and API key");
        }

        let url = format!("{}/v2/snapshot/locale/us/markets/stocks", self.base_url);

        debug!("Fetching full market snapshot: {}", url);

        let response = self
            .client
            .get(&url)
            .query(&[("apikey", self.api_key.as_ref().unwrap().as_str())])
            .send()
            .await
            .context("Failed to fetch full market snapshot")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Full market snapshot request failed: {}", error_text);
        }

        let api_response: FullMarketSnapshotResponse = response
            .json()
            .await
            .context("Failed to parse full market snapshot response")?;

        let results = api_response.results.unwrap_or_default();
        info!("Full market snapshot returned {} tickers", results.len());

        Ok(results)
    }

    /// Get full market snapshot with caching for performance
    pub async fn get_full_market_snapshot_cached(&self) -> Result<Vec<MarketSnapshotTicker>> {
        let cache_key = "full_market_snapshot".to_string();
        
        // Check cache first
        {
            let cache = self.market_data_cache.read().await;
            if let Some(cached) = cache.get(&cache_key) {
                if cached.timestamp.elapsed() < self.cache_ttl {
                    if let Some(snapshot) = &cached.full_market_snapshot {
                        debug!("Cache HIT for full market snapshot: {:?}", cached.timestamp.elapsed());
                        return Ok(snapshot.clone());
                    }
                }
            }
        }
        
        // Cache miss - fetch from API
        debug!("Cache MISS for full market snapshot - fetching from API");
        let result = self.get_full_market_snapshot().await?;
        
        // Update cache
        {
            let mut cache = self.market_data_cache.write().await;
            cache.insert(cache_key, CachedMarketData {
                symbol: "*".to_string(), // Global data
                data_type: "full_market_snapshot".to_string(),
                timestamp: Instant::now(),
                daily_bar: None,
                market_movers: None,
                full_market_snapshot: Some(result.clone()),
                daily_market_summary: None,
            });
        }
        
        Ok(result)
    }

    /// Get daily market summary (grouped aggregates) for all US stocks for a specific date
    /// Uses Polygon endpoint: /v2/aggs/grouped/locale/us/market/stocks/{date}
    /// NOTE: Requires market hours testing for full functionality
    pub async fn get_daily_market_summary(&self, date: &str) -> Result<Vec<DailyMarketSummaryTicker>> {
        if !self.polygon_enabled {
            anyhow::bail!("Polygon.io not available - check configuration and API key");
        }

        let url = format!("{}/v2/aggs/grouped/locale/us/market/stocks/{}", self.base_url, date);

        debug!("Fetching daily market summary for {}: {}", date, url);

        let response = self
            .client
            .get(&url)
            .query(&[
                ("apikey", self.api_key.as_ref().unwrap().as_str()),
                ("adjusted", "true"),
            ])
            .send()
            .await
            .context("Failed to fetch daily market summary")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Daily market summary request failed: {}", error_text);
        }

        let api_response: DailyMarketSummaryResponse = response
            .json()
            .await
            .context("Failed to parse daily market summary response")?;

        // Accept both OK and DELAYED status - DELAYED is expected for Stock Starter plan
        if api_response.status != "OK" && api_response.status != "DELAYED" {
            error!("API returned error status: {}", api_response.status);
            anyhow::bail!("API returned error status: {}", api_response.status);
        }
        
        if api_response.status == "DELAYED" {
            debug!("Received delayed data for market summary {} (15-minute delay as expected on Stock Starter plan)", date);
        }

        let results = api_response.results.unwrap_or_default();
        info!("Daily market summary for {} returned {} tickers", date, results.len());

        Ok(results)
    }

    /// Get daily market summary with caching for performance
    pub async fn get_daily_market_summary_cached(&self, date: &str) -> Result<Vec<DailyMarketSummaryTicker>> {
        let cache_key = format!("daily_market_summary:{}", date);
        
        // Check cache first
        {
            let cache = self.market_data_cache.read().await;
            if let Some(cached) = cache.get(&cache_key) {
                if cached.timestamp.elapsed() < self.cache_ttl {
                    if let Some(summary) = &cached.daily_market_summary {
                        debug!("Cache HIT for daily market summary {}: {:?}", date, cached.timestamp.elapsed());
                        return Ok(summary.clone());
                    }
                }
            }
        }
        
        // Cache miss - fetch from API
        debug!("Cache MISS for daily market summary {} - fetching from API", date);
        let result = self.get_daily_market_summary(date).await?;
        
        // Update cache
        {
            let mut cache = self.market_data_cache.write().await;
            cache.insert(cache_key, CachedMarketData {
                symbol: "*".to_string(), // Global data
                data_type: format!("daily_market_summary:{}", date),
                timestamp: Instant::now(),
                daily_bar: None,
                market_movers: None,
                full_market_snapshot: None,
                daily_market_summary: Some(result.clone()),
            });
        }
        
        Ok(result)
    }

    /// Get financial statements for a specific ticker from Polygon
    /// Uses confirmed working Polygon endpoint: /vX/reference/financials
    pub async fn get_financials(&self, symbol: &str) -> Result<Vec<FinancialData>> {
        if !self.polygon_enabled {
            anyhow::bail!("Polygon.io not available - check configuration and API key");
        }

        let url = format!("{}/vX/reference/financials", self.base_url);
        
        let query_params = vec![
            ("apikey", self.api_key.as_ref().unwrap().as_str()),
            ("ticker", symbol),
            ("limit", "4"),
            ("order", "desc"),
        ];

        debug!("Fetching financials for {}: {}", symbol, url);

        let response = self
            .client
            .get(&url)
            .query(&query_params)
            .send()
            .await
            .context("Failed to fetch financials")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Financials request failed: {}", error_text);
        }

        let api_response: FinancialsResponse = response
            .json()
            .await
            .context("Failed to parse financials response")?;

        Ok(api_response.results.unwrap_or_default())
    }
}

// ===============================
// API Response Structures
// ===============================

// Technical Indicators Response Structures

/// Response structure for technical indicators (SMA, EMA, RSI)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TechnicalIndicatorResponse {
    pub status: String,
    pub request_id: Option<String>,
    pub next_url: Option<String>,
    pub results: Option<IndicatorResults>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IndicatorResults {
    pub underlying: Option<IndicatorUnderlying>,
    pub values: Option<Vec<IndicatorValue>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IndicatorUnderlying {
    pub aggregates: Option<Vec<IndicatorAggregate>>,
    pub url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IndicatorAggregate {
    #[serde(rename = "c")]
    pub close: Option<f64>,
    #[serde(rename = "h")]
    pub high: Option<f64>,
    #[serde(rename = "l")]
    pub low: Option<f64>,
    #[serde(rename = "o")]
    pub open: Option<f64>,
    #[serde(rename = "t")]
    pub timestamp: Option<i64>,
    #[serde(rename = "v")]
    pub volume: Option<f64>,
    #[serde(rename = "vw")]
    pub vwap: Option<f64>,
    #[serde(rename = "n")]
    pub transactions: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IndicatorValue {
    pub timestamp: Option<i64>,
    pub value: Option<f64>,
}

/// Response structure specifically for MACD (has additional fields)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MacdResponse {
    pub status: String,
    pub request_id: Option<String>,
    pub next_url: Option<String>,
    pub results: Option<MacdResults>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MacdResults {
    pub underlying: Option<IndicatorUnderlying>,
    pub values: Option<Vec<MacdValue>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MacdValue {
    pub timestamp: Option<i64>,
    pub value: Option<f64>,
    pub signal: Option<f64>,
    pub histogram: Option<f64>,
}

// Market Data Response Structures

/// Response structure for market movers
#[derive(Debug, Deserialize, Serialize)]
pub struct MarketMoversResponse {
    pub status: String,
    pub results: Option<Vec<MarketMoverData>>,
    pub count: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MarketMoverData {
    pub ticker: Option<String>,
    #[serde(rename = "todaysChange")]
    pub todays_change: Option<f64>,
    #[serde(rename = "todaysChangePerc")]
    pub todays_change_perc: Option<f64>,
    pub updated: Option<i64>,
    pub day: Option<MarketMoverDayData>,
    #[serde(rename = "min")]
    pub minute: Option<MarketMoverMinuteData>,
    #[serde(rename = "prevDay")]
    pub prev_day: Option<MarketMoverDayData>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MarketMoverDayData {
    #[serde(rename = "c")]
    pub close: Option<f64>,
    #[serde(rename = "h")]
    pub high: Option<f64>,
    #[serde(rename = "l")]
    pub low: Option<f64>,
    #[serde(rename = "o")]
    pub open: Option<f64>,
    #[serde(rename = "v")]
    pub volume: Option<f64>,
    #[serde(rename = "vw")]
    pub vwap: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MarketMoverMinuteData {
    #[serde(rename = "av")]
    pub average_volume: Option<f64>,
    #[serde(rename = "c")]
    pub close: Option<f64>,
    #[serde(rename = "h")]
    pub high: Option<f64>,
    #[serde(rename = "l")]
    pub low: Option<f64>,
    #[serde(rename = "o")]
    pub open: Option<f64>,
    #[serde(rename = "t")]
    pub timestamp: Option<i64>,
    #[serde(rename = "v")]
    pub volume: Option<f64>,
    #[serde(rename = "vw")]
    pub vwap: Option<f64>,
}

/// Market status response structure
#[derive(Debug, Deserialize)]
pub struct MarketStatusResponse {
    #[serde(rename = "afterHours")]
    pub after_hours: Option<bool>,
    #[serde(rename = "earlyHours")]
    pub early_hours: Option<bool>,
    pub currencies: Option<MarketStatus>,
    pub crypto: Option<MarketStatus>,
    pub fx: Option<MarketStatus>,
    pub indices: Option<MarketStatus>,
    pub market: Option<String>,
    #[serde(rename = "serverTime")]
    pub server_time: Option<String>,
    pub stocks: Option<MarketStatus>,
}

#[derive(Debug, Deserialize)]
pub struct MarketStatus {
    pub market: Option<String>,
}

// News Response Structures

/// Response structure for news articles
#[derive(Debug, Deserialize)]
pub struct NewsResponse {
    pub status: String,
    pub request_id: Option<String>,
    pub count: Option<u32>,
    pub next_url: Option<String>,
    pub results: Option<Vec<NewsArticle>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NewsArticle {
    pub id: Option<String>,
    pub publisher: Option<NewsPublisher>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub published_utc: Option<String>,
    pub article_url: Option<String>,
    pub tickers: Option<Vec<String>>,
    pub image_url: Option<String>,
    pub description: Option<String>,
    pub keywords: Option<Vec<String>>,
    pub insights: Option<Vec<NewsInsight>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NewsPublisher {
    pub name: Option<String>,
    pub homepage_url: Option<String>,
    pub logo_url: Option<String>,
    pub favicon_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NewsInsight {
    pub ticker: Option<String>,
    pub sentiment: Option<String>,
    pub sentiment_reasoning: Option<String>,
}

/// Response structure for API symbol lookup
#[derive(Debug, Serialize)]
pub struct SymbolDataResponse {
    pub symbol: String,
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
    pub vwap: f64,
    pub transactions: u32,
    pub change: f64,
    pub change_percent: f64,
}

/// Response structure for minute aggregates endpoint (basic plan compatible)
#[derive(Debug, Deserialize)]
pub struct MinuteAggregatesResponse {
    pub status: String,
    pub ticker: Option<String>,
    #[serde(rename = "queryCount")]
    pub query_count: Option<u32>,
    #[serde(rename = "resultsCount")]
    pub results_count: Option<u32>,
    pub adjusted: Option<bool>,
    pub results: Option<Vec<MinuteBar>>,
}

/// Individual minute bar data
#[derive(Debug, Deserialize)]
pub struct MinuteBar {
    #[serde(rename = "c")]
    pub close: Option<f64>,
    #[serde(rename = "h")]
    pub high: Option<f64>,
    #[serde(rename = "l")]
    pub low: Option<f64>,
    #[serde(rename = "o")]
    pub open: Option<f64>,
    #[serde(rename = "v")]
    pub volume: Option<f64>,  // Changed from u64 to f64 to handle scientific notation
    #[serde(rename = "vw")]
    pub vwap: Option<f64>,
    #[serde(rename = "t")]
    pub timestamp: Option<i64>,
    #[serde(rename = "n")]
    pub transactions: Option<u32>,
}

/// Response structure for daily aggregates
#[derive(Debug, Deserialize)]
pub struct DailyAggregatesResponse {
    pub status: String,
    pub ticker: Option<String>,
    #[serde(rename = "queryCount")]
    pub query_count: Option<u32>,
    #[serde(rename = "resultsCount")]
    pub results_count: Option<u32>,
    pub adjusted: Option<bool>,
    pub results: Option<Vec<DailyBar>>,
}

/// Daily bar structure
#[derive(Debug, Deserialize, Clone)]
pub struct DailyBar {
    #[serde(rename = "c")]
    pub close: Option<f64>,
    #[serde(rename = "h")]
    pub high: Option<f64>,
    #[serde(rename = "l")]
    pub low: Option<f64>,
    #[serde(rename = "o")]
    pub open: Option<f64>,
    #[serde(rename = "v")]
    pub volume: Option<f64>,  // Changed from u64 to f64 to handle scientific notation
    #[serde(rename = "vw")]
    pub vwap: Option<f64>,
    #[serde(rename = "t")]
    pub timestamp: Option<i64>,
    #[serde(rename = "n")]
    pub transactions: Option<u32>,
}

// ===============================
// Premium API Structures (for future use)
// ===============================

/// Response structure for ticker snapshots (premium plan)
#[derive(Debug, Deserialize)]
pub struct PolygonApiResponse {
    pub status: String,
    pub results: Option<TickerSnapshot>,
}

/// Ticker snapshot data (premium plan)
#[derive(Debug, Deserialize)]
pub struct TickerSnapshot {
    pub ticker: Option<String>,
    pub day: Option<DayData>,
    pub last_quote: Option<LastQuote>,
    pub market_cap: Option<f64>,
    pub updated: Option<String>,
}

/// Day summary data (premium plan)
#[derive(Debug, Deserialize)]
pub struct DayData {
    #[serde(rename = "c")]
    pub close: Option<f64>,
    #[serde(rename = "h")]
    pub high: Option<f64>,
    #[serde(rename = "l")]
    pub low: Option<f64>,
    #[serde(rename = "o")]
    pub open: Option<f64>,
    #[serde(rename = "v")]
    pub volume: Option<u64>,
    #[serde(rename = "vw")]
    pub volume_weighted: Option<f64>,
    #[serde(rename = "t")]
    pub timestamp: Option<i64>,
    #[serde(rename = "n")]
    pub transactions: Option<u32>,
    // Calculated fields
    pub change: Option<f64>,
    pub change_percent: Option<f64>,
}

/// Last quote data (premium plan)
#[derive(Debug, Deserialize)]
pub struct LastQuote {
    #[serde(rename = "P")]
    pub ask: Option<f64>,
    #[serde(rename = "S")]
    pub ask_size: Option<u64>,
    #[serde(rename = "p")]
    pub bid: Option<f64>,
    #[serde(rename = "s")]
    pub bid_size: Option<u64>,
    #[serde(rename = "t")]
    pub timestamp: Option<i64>,
}

/// Response structure for tickers endpoint
#[derive(Debug, Deserialize, Serialize)]
pub struct TickersResponse {
    pub status: String,
    pub request_id: Option<String>,
    pub count: Option<u32>,
    pub next_url: Option<String>,
    pub results: Option<Vec<TickerInfo>>,
}

/// Individual ticker information
#[derive(Debug, Deserialize, Serialize)]
pub struct TickerInfo {
    pub ticker: Option<String>,
    pub name: Option<String>,
    pub market: Option<String>,
    pub locale: Option<String>,
    pub primary_exchange: Option<String>,
    #[serde(rename = "type")]
    pub ticker_type: Option<String>,
    pub active: Option<bool>,
    pub currency_name: Option<String>,
    pub cik: Option<String>,
    pub composite_figi: Option<String>,
    pub share_class_figi: Option<String>,
}

/// Response structure for exchanges endpoint
#[derive(Debug, Deserialize, Serialize)]
pub struct ExchangesResponse {
    pub status: String,
    pub request_id: Option<String>,
    pub count: Option<u32>,
    pub results: Option<Vec<ExchangeInfo>>,
}

/// Individual exchange information
#[derive(Debug, Deserialize, Serialize)]
pub struct ExchangeInfo {
    pub acronym: Option<String>,
    pub asset_class: Option<String>,
    pub id: Option<i32>,
    pub locale: Option<String>,
    pub mic: Option<String>,
    pub name: Option<String>,
    pub operating_mic: Option<String>,
    pub participant_id: Option<String>,
    #[serde(rename = "type")]
    pub exchange_type: Option<String>,
    pub url: Option<String>,
}

/// Response structure for stock splits endpoint
#[derive(Debug, Deserialize, Serialize)]
pub struct SplitsResponse {
    pub status: String,
    pub request_id: Option<String>,
    pub count: Option<u32>,
    pub next_url: Option<String>,
    pub results: Option<Vec<SplitInfo>>,
}

/// Individual stock split information
#[derive(Debug, Deserialize, Serialize)]
pub struct SplitInfo {
    pub ticker: Option<String>,
    pub execution_date: Option<String>,
    pub split_from: Option<f64>,
    pub split_to: Option<f64>,
}

/// Response structure for financials endpoint
#[derive(Debug, Deserialize, Serialize)]
pub struct FinancialsResponse {
    pub status: String,
    pub request_id: Option<String>,
    pub count: Option<u32>,
    pub next_url: Option<String>,
    pub results: Option<Vec<FinancialData>>,
}

/// Financial statements data
#[derive(Debug, Deserialize, Serialize)]
pub struct FinancialData {
    pub cik: Option<String>,
    pub company_name: Option<String>,
    pub ticker: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub filing_date: Option<String>,
    pub period_of_report_date: Option<String>,
    pub timeframe: Option<String>, // "annual", "quarterly"
    pub fiscal_period: Option<String>,
    pub fiscal_year: Option<String>,
    pub financials: Option<FinancialStatements>,
}

/// Financial statements breakdown
#[derive(Debug, Deserialize, Serialize)]
pub struct FinancialStatements {
    pub balance_sheet: Option<serde_json::Value>,
    pub cash_flow_statement: Option<serde_json::Value>,
    pub income_statement: Option<serde_json::Value>,
    pub comprehensive_income: Option<serde_json::Value>,
}

/// Response structure for full market snapshot endpoint
#[derive(Debug, Deserialize, Serialize)]
pub struct FullMarketSnapshotResponse {
    pub status: String,
    pub request_id: Option<String>,
    pub count: Option<u32>,
    pub results: Option<Vec<MarketSnapshotTicker>>,
}

/// Individual ticker snapshot in full market snapshot
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MarketSnapshotTicker {
    pub ticker: Option<String>,
    #[serde(rename = "todaysChange")]
    pub todays_change: Option<f64>,
    #[serde(rename = "todaysChangePerc")]
    pub todays_change_perc: Option<f64>,
    pub updated: Option<i64>,
    pub day: Option<MarketSnapshotDayData>,
    #[serde(rename = "min")]
    pub minute: Option<MarketSnapshotMinuteData>,
    #[serde(rename = "prevDay")]
    pub prev_day: Option<MarketSnapshotDayData>,
    #[serde(rename = "lastQuote")]
    pub last_quote: Option<MarketSnapshotLastQuote>,
    #[serde(rename = "lastTrade")]
    pub last_trade: Option<MarketSnapshotLastTrade>,
}

/// Day data for market snapshot ticker
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MarketSnapshotDayData {
    #[serde(rename = "c")]
    pub close: Option<f64>,
    #[serde(rename = "h")]
    pub high: Option<f64>,
    #[serde(rename = "l")]
    pub low: Option<f64>,
    #[serde(rename = "o")]
    pub open: Option<f64>,
    #[serde(rename = "v")]
    pub volume: Option<f64>,
    #[serde(rename = "vw")]
    pub vwap: Option<f64>,
}

/// Minute data for market snapshot ticker
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MarketSnapshotMinuteData {
    #[serde(rename = "av")]
    pub average_volume: Option<f64>,
    #[serde(rename = "c")]
    pub close: Option<f64>,
    #[serde(rename = "h")]
    pub high: Option<f64>,
    #[serde(rename = "l")]
    pub low: Option<f64>,
    #[serde(rename = "o")]
    pub open: Option<f64>,
    #[serde(rename = "t")]
    pub timestamp: Option<i64>,
    #[serde(rename = "v")]
    pub volume: Option<f64>,
    #[serde(rename = "vw")]
    pub vwap: Option<f64>,
}

/// Last quote data for market snapshot ticker
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MarketSnapshotLastQuote {
    #[serde(rename = "P")]
    pub ask: Option<f64>,
    #[serde(rename = "S")]
    pub ask_size: Option<i64>,
    #[serde(rename = "p")]
    pub bid: Option<f64>,
    #[serde(rename = "s")]
    pub bid_size: Option<i64>,
    #[serde(rename = "t")]
    pub timestamp: Option<i64>,
}

/// Last trade data for market snapshot ticker
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MarketSnapshotLastTrade {
    #[serde(rename = "c")]
    pub conditions: Option<Vec<i32>>,
    #[serde(rename = "i")]
    pub id: Option<String>,
    #[serde(rename = "p")]
    pub price: Option<f64>,
    #[serde(rename = "s")]
    pub sip_timestamp: Option<i64>,
    #[serde(rename = "t")]
    pub participant_timestamp: Option<i64>,
    #[serde(rename = "x")]
    pub exchange: Option<i32>,
    #[serde(rename = "z")]
    pub size: Option<i64>,
}

/// Response structure for daily market summary (grouped aggregates)
#[derive(Debug, Deserialize, Serialize)]
pub struct DailyMarketSummaryResponse {
    pub status: String,
    pub request_id: Option<String>,
    #[serde(rename = "queryCount")]
    pub query_count: Option<u32>,
    #[serde(rename = "resultsCount")]
    pub results_count: Option<u32>,
    pub adjusted: Option<bool>,
    pub results: Option<Vec<DailyMarketSummaryTicker>>,
}

/// Individual ticker summary in daily market summary
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DailyMarketSummaryTicker {
    #[serde(rename = "T")]
    pub ticker: Option<String>,
    #[serde(rename = "c")]
    pub close: Option<f64>,
    #[serde(rename = "h")]
    pub high: Option<f64>,
    #[serde(rename = "l")]
    pub low: Option<f64>,
    #[serde(rename = "o")]
    pub open: Option<f64>,
    #[serde(rename = "v")]
    pub volume: Option<f64>,
    #[serde(rename = "vw")]
    pub vwap: Option<f64>,
    #[serde(rename = "t")]
    pub timestamp: Option<i64>,
    #[serde(rename = "n")]
    pub transactions: Option<u32>,
}