use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub polygon_api_key: String,
    pub lightspeed_config: LightspeedConfig,
    pub max_memory_mb: u64,
    pub bind_address: String,
    pub metrics_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightspeedConfig {
    pub api_key: String,
    pub api_secret: String,
    pub base_url: String,
    pub sandbox: bool,
}

impl Config {
    pub fn load() -> Result<Self> {
        // Load .env file if it exists (for development)
        dotenvy::dotenv().ok();

        let config = Config {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:./data/trading.db".to_string()),
            
            polygon_api_key: env::var("POLYGON_API_KEY")
                .context("POLYGON_API_KEY environment variable is required")?,
            
            lightspeed_config: LightspeedConfig {
                api_key: env::var("LIGHTSPEED_API_KEY")
                    .context("LIGHTSPEED_API_KEY environment variable is required")?,
                
                api_secret: env::var("LIGHTSPEED_API_SECRET")
                    .context("LIGHTSPEED_API_SECRET environment variable is required")?,
                
                base_url: env::var("LIGHTSPEED_BASE_URL")
                    .unwrap_or_else(|_| "https://api.lightspeed.com".to_string()),
                
                sandbox: env::var("LIGHTSPEED_SANDBOX")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
            },
            
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
        if self.polygon_api_key.is_empty() {
            anyhow::bail!("Polygon API key cannot be empty");
        }
        
        if self.lightspeed_config.api_key.is_empty() {
            anyhow::bail!("Lightspeed API key cannot be empty");
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
        
        Ok(())
    }
    
    pub fn max_memory_bytes(&self) -> u64 {
        self.max_memory_mb * 1024 * 1024
    }
}