// src/data/mod.rs
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};
use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

use crate::config::Config;

pub mod polygon;
mod cache_stats;

pub use cache_stats::CacheStats;

pub struct DataModule {
    client: Client,
    api_key: Option<String>,
    base_url: String,
    polygon_enabled: bool,
    // High-performance in-memory cache for <5ms decision making
    indicator_cache: Arc<RwLock<HashMap<String, CachedIndicator>>>,
    market_data_cache: Arc<RwLock<HashMap<String, CachedMarketData>>>,
    cache_ttl: Duration,
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
    data_type: String, // "snapshot", "previous_day", "minute_agg"
    timestamp: Instant,
    // Store different data types as needed
    daily_bar: Option<DailyBar>,
    market_movers: Option<Vec<MarketMoverData>>,
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
        
        if polygon_enabled {
            if config.polygon_use_delayed_data {
                info!("✅ Polygon.io data module enabled with delayed data (15-min delay) and in-memory cache");
            } else {
                info!("✅ Polygon.io data module enabled with real-time data and in-memory cache");
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
        })
    }

    /// Get ticker snapshot - uses daily aggregates for basic plan compatibility
    /// This method will be updated to use actual snapshots when plan is upgraded
    pub async fn get_ticker_snapshot(&self, symbol: &str) -> Result<()> {
        if !self.polygon_enabled {
            debug!("Polygon.io disabled - skipping ticker snapshot for {}", symbol);
            return Ok(());
        }
        
        // For basic plan: use daily aggregates for yesterday's data
        // TODO: Switch to actual snapshot endpoint when upgrading to paid plan
        self.get_yesterday_daily_data(symbol).await
    }

    /// Get yesterday's daily data for a symbol via API call
    pub async fn get_symbol_data(&self, symbol: &str) -> Result<SymbolDataResponse> {
        if !self.polygon_enabled {
            anyhow::bail!("Polygon.io not available - check configuration and API key");
        }
        // Get yesterday's date in YYYY-MM-DD format
        let yesterday = chrono::Utc::now() - chrono::Duration::days(1);
        let yesterday_str = yesterday.format("%Y-%m-%d").to_string();
        
        let url = format!(
            "{}/v2/aggs/ticker/{}/range/1/day/{}/{}",
            self.base_url, symbol, yesterday_str, yesterday_str
        );

        info!("API request: Fetching daily data for {} ({})", symbol, yesterday_str);
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
                    date: yesterday_str,
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
                anyhow::bail!("No daily data available for {} on {} (market may be closed)", symbol, yesterday_str);
            }
        } else {
            anyhow::bail!("No daily data found for {} on {}", symbol, yesterday_str);
        }
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

    /// Test API connection using available endpoints for current plan
    pub async fn test_connection(&self) -> Result<()> {
        info!("Testing Polygon.io API connection with AMZN daily data...");
        self.get_yesterday_daily_data("AMZN").await
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

    /// Get yesterday's daily data for a symbol (compatible with basic plan) - for internal testing
    pub async fn get_yesterday_daily_data(&self, symbol: &str) -> Result<()> {
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
            });
        }
        
        Ok(result)
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

#[derive(Debug, Deserialize, Clone)]
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

#[derive(Debug, Deserialize, Clone)]
pub struct NewsPublisher {
    pub name: Option<String>,
    pub homepage_url: Option<String>,
    pub logo_url: Option<String>,
    pub favicon_url: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
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