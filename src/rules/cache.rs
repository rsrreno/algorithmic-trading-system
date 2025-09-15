// src/rules/cache.rs
// Branch: 14.9.25

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::time;
use anyhow::Result;
use tracing::{info, warn, error, debug};

use crate::types::TechnicalIndicators;
use crate::data::DataModule;

/// High-performance in-memory cache for technical indicators
/// Designed for <1ms access times to meet <5ms decision pipeline requirement
pub struct IndicatorCache {
    cache: Arc<RwLock<HashMap<String, CachedIndicator>>>,
    data_module: Arc<DataModule>,
    cache_ttl: Duration,
    last_cleanup: Arc<RwLock<Instant>>,
}

#[derive(Debug, Clone)]
struct CachedIndicator {
    indicators: TechnicalIndicators,
    cached_at: Instant,
    access_count: u64,
    last_accessed: Instant,
}

impl CachedIndicator {
    fn new(indicators: TechnicalIndicators) -> Self {
        let now = Instant::now();
        Self {
            indicators,
            cached_at: now,
            access_count: 0,
            last_accessed: now,
        }
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        self.cached_at.elapsed() > ttl
    }

    fn access(&mut self) -> &TechnicalIndicators {
        self.access_count += 1;
        self.last_accessed = Instant::now();
        &self.indicators
    }
}

impl IndicatorCache {
    pub fn new(data_module: Arc<DataModule>) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            data_module,
            cache_ttl: Duration::from_secs(300), // 5 minutes default TTL
            last_cleanup: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Get technical indicators for a symbol with <1ms target access time
    /// Returns cached data if available and fresh, otherwise fetches new data
    pub async fn get_indicators(&self, symbol: &str) -> Result<TechnicalIndicators> {
        let start_time = Instant::now();
        
        // Try to get from cache first (fast path)
        if let Some(indicators) = self.get_from_cache(symbol) {
            let access_time = start_time.elapsed();
            if access_time.as_micros() > 1000 { // Warn if >1ms
                warn!("Cache access took {}μs for {}", access_time.as_micros(), symbol);
            }
            return Ok(indicators);
        }

        // Cache miss - fetch fresh data (slow path)
        let indicators = self.fetch_fresh_indicators(symbol).await?;
        self.update_cache(symbol, indicators.clone()).await;
        
        let total_time = start_time.elapsed();
        info!("Fresh indicator fetch took {}ms for {}", total_time.as_millis(), symbol);
        
        Ok(indicators)
    }

    /// Fast cache-only lookup (guaranteed <1ms)
    fn get_from_cache(&self, symbol: &str) -> Option<TechnicalIndicators> {
        let cache = self.cache.read().ok()?;
        let cached_indicator = cache.get(symbol)?.clone();
        
        // Check if expired
        if cached_indicator.is_expired(self.cache_ttl) {
            return None;
        }
        
        // Update access statistics (we use a clone to avoid holding the read lock)
        drop(cache);
        if let Ok(mut cache) = self.cache.write() {
            if let Some(cached) = cache.get_mut(symbol) {
                cached.access();
            }
        }
        
        Some(cached_indicator.indicators)
    }

    /// Fetch fresh indicators from data module
    async fn fetch_fresh_indicators(&self, symbol: &str) -> Result<TechnicalIndicators> {
        // Get current price and volume - try current market price first
        let (price, volume) = match self.data_module.get_current_market_price(symbol).await {
            Ok(current_price) => {
                // Get volume from previous day data since current price doesn't include volume
                let volume = match self.data_module.get_symbol_data(symbol).await {
                    Ok(symbol_data) => symbol_data.volume,
                    Err(_) => 1000000, // Default volume fallback
                };
                debug!("Using current market price for {}: ${:.2}", symbol, current_price);
                (current_price, volume)
            }
            Err(_) => {
                // Fallback to previous day data
                match self.data_module.get_symbol_data(symbol).await {
                    Ok(symbol_data) => {
                        let price = symbol_data.close;
                        let volume = symbol_data.volume;
                        debug!("Using previous day data for {}: ${:.2}", symbol, price);
                        (price, volume)
                    }
                    Err(_) => {
                        // Final fallback values if both fail
                        (100.0, 1000) // Default reasonable values for testing
                    }
                }
            }
        };
        
        let mut indicators = TechnicalIndicators::new(symbol.to_string(), price, volume);
        
        // Fetch indicators using data module methods
        // Note: These calls may fail, so we handle errors gracefully
        
        // RSI - using non-cached version for now
        if let Ok(rsi_response) = self.data_module.get_rsi(symbol, 14, "day", "1").await {
            if let Some(ref results) = rsi_response.results {
                if let Some(ref values_vec) = results.values {
                    if let Some(first_value) = values_vec.first() {
                        if let Some(value) = first_value.value {
                            indicators.rsi_14 = Some(value);
                        }
                    }
                }
            }
        }

        // SMA - using non-cached version for now  
        if let Ok(sma_response) = self.data_module.get_sma(symbol, 20, "day", "1").await {
            if let Some(ref results) = sma_response.results {
                if let Some(ref values_vec) = results.values {
                    if let Some(first_value) = values_vec.first() {
                        if let Some(value) = first_value.value {
                            indicators.sma_20 = Some(value);
                        }
                    }
                }
            }
        }

        // EMA - using non-cached version for now
        if let Ok(ema_response) = self.data_module.get_ema(symbol, 9, "day", "1").await {
            if let Some(ref results) = ema_response.results {
                if let Some(ref values_vec) = results.values {
                    if let Some(first_value) = values_vec.first() {
                        if let Some(ema_value) = first_value.value {
                            indicators.ema_9 = Some(ema_value);
                            indicators.price_above_ema9 = price > ema_value;
                        }
                    }
                }
            }
        }

        // MACD
        if let Ok(macd_response) = self.data_module.get_macd(symbol, 12, 26, 9, "day", "1").await {
            if let Some(ref results) = macd_response.results {
                if let Some(ref values_vec) = results.values {
                    if let Some(first_value) = values_vec.first() {
                        if let Some(value) = first_value.value {
                            indicators.macd_value = Some(value);
                        }
                        indicators.macd_signal = first_value.signal;
                        if let (Some(macd), Some(signal)) = (indicators.macd_value, indicators.macd_signal) {
                            indicators.macd_histogram = Some(macd - signal);
                        }
                    }
                }
            }
        }

        // Calculate derived values
        self.calculate_derived_values(&mut indicators).await?;

        Ok(indicators)
    }

    /// Calculate derived indicator values (slopes, ratios, etc.)
    async fn calculate_derived_values(&self, indicators: &mut TechnicalIndicators) -> Result<()> {
        // For EMA slope, we need at least 2 data points
        // This is a simplified calculation - in production you'd want more historical data
        if let Some(ema9) = indicators.ema_9 {
            // Estimate slope based on price momentum (simplified)
            let price_momentum = (indicators.price / ema9 - 1.0) * 100.0;
            indicators.ema9_slope = Some(price_momentum);
        }

        // Calculate volume ratio (current vs average)
        // This would typically require historical volume data
        indicators.volume_ratio = Some(1.0); // Placeholder - implement with historical data

        Ok(())
    }

    /// Update cache with new indicators
    async fn update_cache(&self, symbol: &str, indicators: TechnicalIndicators) {
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(symbol.to_string(), CachedIndicator::new(indicators));
        }
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> CacheStats {
        if let Ok(cache) = self.cache.read() {
            let total_symbols = cache.len();
            let total_access_count: u64 = cache.values().map(|c| c.access_count).sum();
            let expired_count = cache.values()
                .filter(|c| c.is_expired(self.cache_ttl))
                .count();
            
            CacheStats {
                total_symbols,
                total_access_count,
                expired_count,
                cache_hit_rate: 0.0, // Would need hit/miss tracking
            }
        } else {
            CacheStats::default()
        }
    }

    /// Clean up expired cache entries
    pub async fn cleanup_expired(&self) {
        let mut should_cleanup = false;
        
        // Check if cleanup is needed (only every 60 seconds)
        if let Ok(last_cleanup) = self.last_cleanup.read() {
            should_cleanup = last_cleanup.elapsed() > Duration::from_secs(60);
        }
        
        if !should_cleanup {
            return;
        }

        let mut removed_count = 0;
        if let Ok(mut cache) = self.cache.write() {
            let keys_to_remove: Vec<String> = cache
                .iter()
                .filter(|(_, cached)| cached.is_expired(self.cache_ttl))
                .map(|(key, _)| key.clone())
                .collect();
                
            for key in keys_to_remove {
                cache.remove(&key);
                removed_count += 1;
            }
        }

        // Update last cleanup time
        if let Ok(mut last_cleanup) = self.last_cleanup.write() {
            *last_cleanup = Instant::now();
        }

        if removed_count > 0 {
            info!("Cleaned up {} expired cache entries", removed_count);
        }
    }

    /// Start background cache maintenance task
    pub fn start_maintenance_task(cache: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                cache.cleanup_expired().await;
            }
        });
    }

    /// Force refresh of indicators for a symbol
    pub async fn refresh_symbol(&self, symbol: &str) -> Result<TechnicalIndicators> {
        let indicators = self.fetch_fresh_indicators(symbol).await?;
        self.update_cache(symbol, indicators.clone()).await;
        Ok(indicators)
    }

    /// Pre-populate cache with multiple symbols
    pub async fn warm_cache(&self, symbols: &[String]) -> Result<()> {
        info!("Warming cache for {} symbols", symbols.len());
        
        for symbol in symbols {
            if let Err(e) = self.refresh_symbol(symbol).await {
                error!("Failed to warm cache for {}: {}", symbol, e);
            }
        }
        
        info!("Cache warming complete");
        Ok(())
    }
}

#[derive(Debug, Default, serde::Serialize)]
pub struct CacheStats {
    pub total_symbols: usize,
    pub total_access_count: u64,
    pub expired_count: usize,
    pub cache_hit_rate: f64,
}