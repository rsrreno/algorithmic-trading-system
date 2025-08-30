// src/config/mod.rs
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub polygon_api_key: Option<String>,
    pub polygon_use_delayed_data: bool,
    pub lightspeed_config: Option<LightspeedConfig>,
    pub max_memory_mb: u64,
    pub bind_address: String,
    pub metrics_address: String,
    pub enable_polygon: bool,
    pub enable_lightspeed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightspeedConfig {
    pub api_key: String,
    pub connection_url: String,
    pub client_id: String,
    pub account_id: String,
    pub sandbox: bool,
}

impl Config {
    pub fn load() -> Result<Self> {
        // Load .env file if it exists (for development)
        dotenvy::dotenv().ok();

        let config = Config {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:./data/trading.db".to_string()),
            
            polygon_api_key: env::var("POLYGON_API_KEY").ok(),
            
            polygon_use_delayed_data: env::var("POLYGON_USE_DELAYED_DATA")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            
            lightspeed_config: if env::var("LIGHTSPEED_API_KEY").is_ok() && env::var("LIGHTSPEED_ACCOUNT_ID").is_ok() {
                Some(LightspeedConfig {
                    api_key: env::var("LIGHTSPEED_API_KEY").unwrap(),
                    
                    connection_url: env::var("LIGHTSPEED_CONNECTION_URL")
                        .unwrap_or_else(|_| "wss://onboarding.connecttrade.com:28052".to_string()),
                    
                    client_id: env::var("LIGHTSPEED_CLIENT_ID")
                        .unwrap_or_else(|_| "LIGHTSPEED".to_string()),
                    
                    account_id: env::var("LIGHTSPEED_ACCOUNT_ID").unwrap(),
                    
                    sandbox: env::var("LIGHTSPEED_SANDBOX")
                        .unwrap_or_else(|_| "true".to_string())
                        .parse()
                        .unwrap_or(true),
                })
            } else {
                None
            },
            
            enable_polygon: env::var("ENABLE_POLYGON")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            
            enable_lightspeed: env::var("ENABLE_LIGHTSPEED")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            
            max_memory_mb: env::var("MAX_MEMORY_MB")
                .unwrap_or_else(|_| "1024".to_string())
                .parse()
                .context("MAX_MEMORY_MB must be a valid number")?,
            
            bind_address: env::var("BIND_ADDRESS")
                .unwrap_or_else(|_| "127.0.0.1:8080".to_string()),
            
            metrics_address: env::var("METRICS_ADDRESS")
                .unwrap_or_else(|_| "127.0.0.1:9090".to_string()),
        };

        // Validate configuration
        config.validate()?;
        
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        // Validate enabled modules have required configuration
        if self.enable_polygon && self.polygon_api_key.is_none() {
            tracing::warn!("Polygon module enabled but no API key provided - module will be disabled");
        }
        
        if self.enable_lightspeed && self.lightspeed_config.is_none() {
            tracing::warn!("LightSpeed module enabled but configuration missing - module will be disabled");
        }
        
        if let Some(ref lightspeed_config) = self.lightspeed_config {
            if lightspeed_config.api_key.is_empty() {
                tracing::warn!("LightSpeed API key is empty - module will be disabled");
            }
        }
        
        if self.max_memory_mb == 0 {
            anyhow::bail!("Max memory must be greater than 0");
        }
        
        if self.max_memory_mb > 4096 {
            tracing::warn!(
                "Max memory set to {}MB, ensure your system has sufficient RAM", 
                self.max_memory_mb
            );
        }
        
        // Check if at least one data source is available
        let polygon_available = self.enable_polygon && self.polygon_api_key.is_some();
        let lightspeed_available = self.enable_lightspeed && self.lightspeed_config.is_some();
        
        if !polygon_available && !lightspeed_available {
            tracing::warn!("No data sources or brokers configured - system will run in limited mode");
        }
        
        Ok(())
    }
    
    pub fn max_memory_bytes(&self) -> u64 {
        self.max_memory_mb * 1024 * 1024
    }
    
    pub fn is_polygon_enabled(&self) -> bool {
        self.enable_polygon && self.polygon_api_key.is_some()
    }
    
    pub fn is_lightspeed_enabled(&self) -> bool {
        self.enable_lightspeed && self.lightspeed_config.is_some()
    }
}