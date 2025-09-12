// src/config/mod.rs
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub polygon_api_key: Option<String>,
    pub polygon_use_delayed_data: bool,
    pub polygon_websocket_config: Option<PolygonWebSocketConfig>,
    pub lightspeed_config: Option<LightspeedConfig>,
    pub paper_trading_config: PaperTradingConfig,
    pub trading_mode: TradingMode,
    pub max_memory_mb: u64,
    pub bind_address: String,
    pub metrics_address: String,
    pub enable_polygon: bool,
    pub enable_lightspeed: bool,
    pub rules_engine_config: RulesEngineConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradingMode {
    Paper,
    Live,
    Simulation,
}

impl Default for TradingMode {
    fn default() -> Self {
        Self::Paper
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperTradingConfig {
    pub initial_cash: f64,
    pub commission_per_share: f64,
    pub enable_commission: bool,
    pub session_id: String,
}

impl Default for PaperTradingConfig {
    fn default() -> Self {
        Self {
            initial_cash: 100000.0,
            commission_per_share: 0.005,
            enable_commission: true,
            session_id: "default-session".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolygonWebSocketConfig {
    pub enabled: bool,
    pub reconnect_interval_secs: u64,
    pub heartbeat_interval_secs: u64,
    pub max_subscriptions: usize,
    pub buffer_size: usize,
    pub default_subscriptions: Vec<String>,
    pub auto_subscribe_movers: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightspeedConfig {
    pub api_key: String,
    pub connection_url: String,
    pub client_id: String,
    pub account_id: String,
    pub sandbox: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesEngineConfig {
    pub enabled: bool,
    pub evaluation_interval_ms: Option<u64>,
    pub max_decision_time_ms: Option<u64>,
    pub warning_threshold_ms: Option<u64>,
    pub track_performance: bool,
    pub log_all_decisions: bool,
    pub require_user_confirmation: bool,
    pub decision_history_days: Option<u32>,
    pub price_fallback_strategy: PriceFallbackStrategy,
    pub default_watchlist_symbols: Vec<String>,
    pub allow_new_positions: Option<bool>,
    pub max_concurrent_rules: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PriceFallbackStrategy {
    LastKnown,
    MarketClose,
    UserConfigured(f64),
    Refuse, // Don't execute if no price available
}

impl Default for PriceFallbackStrategy {
    fn default() -> Self {
        Self::Refuse // Safe default - don't assume price
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        // Load .env file if it exists (for development)
        dotenvy::dotenv().ok();

        let polygon_api_key = env::var("POLYGON_API_KEY").ok();
        
        let config = Config {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:./data/trading.db".to_string()),
            
            polygon_api_key: polygon_api_key.clone(),
            
            polygon_use_delayed_data: env::var("POLYGON_USE_DELAYED_DATA")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            
            polygon_websocket_config: if polygon_api_key.is_some() && env::var("POLYGON_ENABLE_WEBSOCKET").is_ok() {
                Some(PolygonWebSocketConfig {
                    enabled: env::var("POLYGON_ENABLE_WEBSOCKET")
                        .context("POLYGON_ENABLE_WEBSOCKET not set")?
                        .parse()
                        .context("POLYGON_ENABLE_WEBSOCKET must be true or false")?,
                    reconnect_interval_secs: env::var("POLYGON_WEBSOCKET_RECONNECT_INTERVAL")
                        .context("POLYGON_WEBSOCKET_RECONNECT_INTERVAL not set")?
                        .parse()
                        .context("POLYGON_WEBSOCKET_RECONNECT_INTERVAL must be a number")?,
                    heartbeat_interval_secs: env::var("POLYGON_WEBSOCKET_HEARTBEAT_INTERVAL")
                        .context("POLYGON_WEBSOCKET_HEARTBEAT_INTERVAL not set")?
                        .parse()
                        .context("POLYGON_WEBSOCKET_HEARTBEAT_INTERVAL must be a number")?,
                    max_subscriptions: env::var("POLYGON_WEBSOCKET_MAX_SUBSCRIPTIONS")
                        .context("POLYGON_WEBSOCKET_MAX_SUBSCRIPTIONS not set")?
                        .parse()
                        .context("POLYGON_WEBSOCKET_MAX_SUBSCRIPTIONS must be a number")?,
                    buffer_size: env::var("POLYGON_WEBSOCKET_BUFFER_SIZE")
                        .context("POLYGON_WEBSOCKET_BUFFER_SIZE not set")?
                        .parse()
                        .context("POLYGON_WEBSOCKET_BUFFER_SIZE must be a number")?,
                    default_subscriptions: env::var("POLYGON_DEFAULT_SUBSCRIPTIONS")
                        .context("POLYGON_DEFAULT_SUBSCRIPTIONS not set")?
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                    auto_subscribe_movers: env::var("POLYGON_AUTO_SUBSCRIBE_MOVERS")
                        .context("POLYGON_AUTO_SUBSCRIBE_MOVERS not set")?
                        .parse()
                        .context("POLYGON_AUTO_SUBSCRIBE_MOVERS must be true or false")?,
                })
            } else {
                None
            },
            
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

            // Paper Trading Configuration
            paper_trading_config: PaperTradingConfig {
                initial_cash: env::var("PAPER_INITIAL_CASH")
                    .unwrap_or_else(|_| "100000.0".to_string())
                    .parse()
                    .unwrap_or(100000.0),
                commission_per_share: env::var("PAPER_COMMISSION_PER_SHARE")
                    .unwrap_or_else(|_| "0.005".to_string())
                    .parse()
                    .unwrap_or(0.005),
                enable_commission: env::var("PAPER_ENABLE_COMMISSION")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
                session_id: env::var("PAPER_SESSION_ID")
                    .unwrap_or_else(|_| "default-session".to_string()),
            },

            // Rules Engine Configuration
            rules_engine_config: RulesEngineConfig {
                enabled: env::var("RULES_ENGINE_ENABLED")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()
                    .unwrap_or(false),
                evaluation_interval_ms: env::var("RULES_ENGINE_EVALUATION_INTERVAL_MS")
                    .ok()
                    .and_then(|s| s.parse().ok()),
                max_decision_time_ms: env::var("RULES_ENGINE_MAX_DECISION_TIME_MS")
                    .ok()
                    .and_then(|s| s.parse().ok()),
                warning_threshold_ms: env::var("RULES_ENGINE_WARNING_THRESHOLD_MS")
                    .ok()
                    .and_then(|s| s.parse().ok()),
                track_performance: env::var("RULES_TRACK_PERFORMANCE_METRICS")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
                log_all_decisions: env::var("RULES_LOG_ALL_DECISIONS")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()
                    .unwrap_or(false),
                require_user_confirmation: env::var("RULES_REQUIRE_USER_CONFIRMATION")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
                decision_history_days: env::var("RULES_DECISION_HISTORY_DAYS")
                    .ok()
                    .and_then(|s| s.parse().ok()),
                price_fallback_strategy: match env::var("RULES_FALLBACK_PRICE_SOURCE")
                    .unwrap_or_else(|_| "REFUSE".to_string())
                    .to_uppercase()
                    .as_str()
                {
                    "LAST_KNOWN" => PriceFallbackStrategy::LastKnown,
                    "MARKET_CLOSE" => PriceFallbackStrategy::MarketClose,
                    price if price.starts_with("USER_") => {
                        if let Ok(value) = price.strip_prefix("USER_").unwrap_or("0").parse::<f64>() {
                            PriceFallbackStrategy::UserConfigured(value)
                        } else {
                            PriceFallbackStrategy::Refuse
                        }
                    },
                    _ => PriceFallbackStrategy::Refuse,
                },
                default_watchlist_symbols: env::var("RULES_DEFAULT_WATCHLIST_SYMBOLS")
                    .unwrap_or_else(|_| String::new())
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect(),
                allow_new_positions: env::var("RULES_ALLOW_NEW_POSITIONS")
                    .ok()
                    .and_then(|s| s.parse().ok()),
                max_concurrent_rules: env::var("RULES_MAX_CONCURRENT_RULES")
                    .ok()
                    .and_then(|s| s.parse().ok()),
            },

            // Trading Mode Configuration
            trading_mode: match env::var("TRADING_MODE")
                .unwrap_or_else(|_| "PAPER".to_string())
                .to_uppercase()
                .as_str()
            {
                "LIVE" => TradingMode::Live,
                "SIMULATION" => TradingMode::Simulation,
                _ => TradingMode::Paper, // Default to paper trading
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
        
        // Validate rules engine configuration
        if self.rules_engine_config.enabled {
            if self.rules_engine_config.evaluation_interval_ms.is_none() {
                tracing::warn!("Rules engine enabled but evaluation interval not set - user must configure before starting");
            }
            if self.rules_engine_config.max_decision_time_ms.is_none() {
                tracing::warn!("Rules engine enabled but max decision time not set - user must configure before starting");
            }
            if self.rules_engine_config.default_watchlist_symbols.is_empty() {
                tracing::warn!("Rules engine enabled but no default watchlist symbols configured");
            }
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
    
    pub fn is_rules_engine_enabled(&self) -> bool {
        self.rules_engine_config.enabled
    }
    
    pub fn rules_engine_ready(&self) -> bool {
        self.rules_engine_config.enabled &&
        self.rules_engine_config.evaluation_interval_ms.is_some() &&
        self.rules_engine_config.max_decision_time_ms.is_some()
    }
}