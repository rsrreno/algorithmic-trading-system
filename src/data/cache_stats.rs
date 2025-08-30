// src/data/cache_stats.rs
use serde::Serialize;

/// Cache statistics for monitoring
#[derive(Debug, Serialize)]
pub struct CacheStats {
    pub indicator_entries: usize,
    pub market_data_entries: usize,
    pub cache_ttl_seconds: u64,
}