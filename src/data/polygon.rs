// src/data/polygon.rs
use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use tracing::{info, warn};
use std::time::Duration;

/// Polygon.io API client for market data
pub struct PolygonClient {
    client: Client,
    api_key: String,
    base_url: String,
}

impl PolygonClient {
    pub fn new(api_key: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("TradingSystem/0.1.0")
            .build()
            .context("Failed to create HTTP client")?;

        Ok(PolygonClient {
            client,
            api_key,
            base_url: "https://api.polygon.io".to_string(),
        })
    }

    /// Test the API connection and rate limits
    pub async fn test_connection(&self) -> Result<()> {
        info!("Testing Polygon.io API connection...");
        
        let url = format!("{}/v1/marketstatus/now", self.base_url);
        
        let response = self
            .client
            .get(&url)
            .query(&[("apikey", &self.api_key)])
            .send()
            .await
            .context("Failed to connect to Polygon API")?;

        if response.status().is_success() {
            info!("✅ Polygon.io API connection successful");
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            warn!("❌ API connection failed: {} - {}", status, body);
            anyhow::bail!("API connection failed: {}", status)
        }
    }

    /// Get current market status  
    pub async fn get_market_status(&self) -> Result<MarketStatus> {
        let url = format!("{}/v1/marketstatus/now", self.base_url);
        
        let response = self
            .client
            .get(&url)
            .query(&[("apikey", &self.api_key)])
            .send()
            .await
            .context("Failed to get market status")?;

        let market_status: MarketStatus = response
            .json()
            .await
            .context("Failed to parse market status response")?;

        info!("Market Status: {}", if market_status.market == "open" { "🟢 OPEN" } else { "🔴 CLOSED" });
        
        Ok(market_status)
    }

    /// Get ticker snapshot (similar to main DataModule but through this client)
    pub async fn get_ticker_snapshot(&self, symbol: &str) -> Result<TickerSnapshotResponse> {
        let url = format!(
            "{}/v2/snapshot/locale/us/markets/stocks/tickers/{}",
            self.base_url, symbol
        );

        let response = self
            .client
            .get(&url)
            .query(&[("apikey", &self.api_key)])
            .send()
            .await
            .context("Failed to get ticker snapshot")?;

        let snapshot: TickerSnapshotResponse = response
            .json()
            .await
            .context("Failed to parse ticker snapshot response")?;

        Ok(snapshot)
    }

    /// Get top market movers
    pub async fn get_top_market_movers(&self, direction: &str) -> Result<TopMoversResponse> {
        let url = format!("{}/v2/snapshot/locale/us/markets/stocks/{}", self.base_url, direction);
        
        let response = self
            .client
            .get(&url)
            .query(&[("apikey", &self.api_key)])
            .send()
            .await
            .context("Failed to get top market movers")?;

        let movers: TopMoversResponse = response
            .json()
            .await
            .context("Failed to parse top movers response")?;

        Ok(movers)
    }
}

#[derive(Debug, Deserialize)]
pub struct MarketStatus {
    #[serde(rename = "afterHours")]
    pub after_hours: bool,
    #[serde(rename = "earlyHours")]  
    pub early_hours: bool,
    pub market: String,
    #[serde(rename = "serverTime")]
    pub server_time: String,
}

#[derive(Debug, Deserialize)]
pub struct TickerSnapshotResponse {
    pub status: String,
    pub results: Option<TickerData>,
}

#[derive(Debug, Deserialize)]
pub struct TickerData {
    pub ticker: Option<String>,
    pub day: Option<DayData>,
    #[serde(rename = "lastQuote")]
    pub last_quote: Option<LastQuote>,
    #[serde(rename = "marketCap")]
    pub market_cap: Option<f64>,
    pub updated: Option<String>,
}

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
}

#[derive(Debug, Deserialize)]
pub struct LastQuote {
    #[serde(rename = "P")]
    pub ask: Option<f64>,
    #[serde(rename = "p")]
    pub bid: Option<f64>,
    #[serde(rename = "S")]
    pub ask_size: Option<u64>,
    #[serde(rename = "s")]
    pub bid_size: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct TopMoversResponse {
    pub status: String,
    pub results: Option<Vec<TickerData>>,
}