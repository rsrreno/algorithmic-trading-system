// src/data/mod.rs
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};
use std::time::Duration;

use crate::config::Config;

pub mod polygon;

pub struct DataModule {
    client: Client,
    api_key: String,
    base_url: String,
}

impl DataModule {
    pub fn new(config: &Config) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("TradingSystem/0.1.0")
            .build()
            .context("Failed to create HTTP client")?;

        Ok(DataModule {
            client,
            api_key: config.polygon_api_key.clone(),
            base_url: "https://api.polygon.io".to_string(),
        })
    }

    /// Get ticker snapshot - uses daily aggregates for basic plan compatibility
    /// This method will be updated to use actual snapshots when plan is upgraded
    pub async fn get_ticker_snapshot(&self, symbol: &str) -> Result<()> {
        // For basic plan: use daily aggregates for yesterday's data
        // TODO: Switch to actual snapshot endpoint when upgrading to paid plan
        self.get_yesterday_daily_data(symbol).await
    }

    /// Get yesterday's daily data for a symbol via API call
    pub async fn get_symbol_data(&self, symbol: &str) -> Result<SymbolDataResponse> {
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
            .query(&[("apikey", self.api_key.as_str())])
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

        if api_response.status != "OK" {
            error!("API returned error status: {}", api_response.status);
            anyhow::bail!("API returned error status: {}", api_response.status);
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
            .query(&[("apikey", self.api_key.as_str())])
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

        if api_response.status != "OK" {
            error!("API returned error status: {}", api_response.status);
            anyhow::bail!("API returned error status: {}", api_response.status);
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
            .query(&[("apikey", self.api_key.as_str())])
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
}

// ===============================
// API Response Structures
// ===============================

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
#[derive(Debug, Deserialize)]
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