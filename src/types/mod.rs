// src/types/mod.rs
// Branch: 9.2.25.1

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::{VecDeque, HashMap};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: String,
    pub symbol: String,
    pub side: PositionSide,
    pub quantity: u64,
    pub entry_price: f64,
    pub current_price: f64,
    pub opened_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub status: PositionStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum PositionSide {
    Long,
    Short,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PositionStatus {
    Open,
    Closed,
    Closing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub symbol: String,
    pub price: f64,
    pub size: u64,
    pub timestamp: i64,
    pub exchange: String,
    pub conditions: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub symbol: String,
    pub bid_price: f64,
    pub ask_price: f64,
    pub bid_size: u64,
    pub ask_size: u64,
    pub timestamp: i64,
    pub exchange: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aggregate {
    pub symbol: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
    pub timestamp: i64,
    pub transactions: u32,
}

#[derive(Debug, Clone)]
pub struct MarketDataStream {
    pub symbol: String,
    pub trades: VecDeque<Trade>,
    pub quotes: VecDeque<Quote>,
    pub aggregates: VecDeque<Aggregate>,
    pub memory_footprint: u64,
    pub created_at: DateTime<Utc>,
}

impl MarketDataStream {
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            trades: VecDeque::new(),
            quotes: VecDeque::new(),
            aggregates: VecDeque::new(),
            memory_footprint: 0,
            created_at: Utc::now(),
        }
    }
    
    pub fn add_trade(&mut self, trade: Trade) {
        let trade_size = std::mem::size_of::<Trade>() as u64;
        self.trades.push_back(trade);
        self.memory_footprint += trade_size;
    }
    
    pub fn add_quote(&mut self, quote: Quote) {
        let quote_size = std::mem::size_of::<Quote>() as u64;
        self.quotes.push_back(quote);
        self.memory_footprint += quote_size;
    }
    
    pub fn add_aggregate(&mut self, aggregate: Aggregate) {
        let agg_size = std::mem::size_of::<Aggregate>() as u64;
        self.aggregates.push_back(aggregate);
        self.memory_footprint += agg_size;
    }
    
    pub fn get_current_price(&self) -> Option<f64> {
        self.trades.back().map(|t| t.price)
            .or_else(|| self.aggregates.back().map(|a| a.close))
    }
    
    pub fn get_memory_usage_mb(&self) -> f64 {
        self.memory_footprint as f64 / 1024.0 / 1024.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingRule {
    pub id: String,
    pub name: String,
    pub conditions: RuleConditions,
    pub max_loss_percent: Option<f64>,
    pub max_loss_dollars: Option<f64>,
    pub max_positions: u32,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

// Legacy RuleConditions struct - removed to avoid duplication
// Enhanced version defined below

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
    pub action: DecisionAction,
    pub rule_name: String,
    pub conditions_met: Vec<String>,
    pub snapshot: DecisionSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DecisionAction {
    Buy,
    Sell,
    Hold,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionSnapshot {
    pub price: f64,
    pub volume: u64,
    pub macd_value: f64,
    pub macd_signal: f64,
    pub rsi_value: f64,
    pub ema9_value: f64,
    pub ema9_slope: f64,
}

// Rules Engine Data Structures

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioState {
    pub total_value: f64,
    pub available_cash: f64,
    pub positions: HashMap<String, Position>,
    pub total_exposure: f64,
    pub max_positions: u32,
    pub current_position_count: u32,
    pub last_updated: DateTime<Utc>,
}

impl PortfolioState {
    pub fn new(initial_cash: f64, max_positions: u32) -> Self {
        Self {
            total_value: initial_cash,
            available_cash: initial_cash,
            positions: HashMap::new(),
            total_exposure: 0.0,
            max_positions,
            current_position_count: 0,
            last_updated: Utc::now(),
        }
    }

    pub fn can_open_position(&self) -> bool {
        self.current_position_count < self.max_positions
    }

    pub fn get_exposure_percentage(&self) -> f64 {
        if self.total_value <= 0.0 {
            0.0
        } else {
            (self.total_exposure / self.total_value) * 100.0
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicalIndicators {
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub price: f64,
    pub volume: u64,
    
    // Moving Averages
    pub sma_20: Option<f64>,
    pub sma_50: Option<f64>,
    pub ema_9: Option<f64>,
    pub ema_21: Option<f64>,
    
    // Momentum Indicators
    pub rsi_14: Option<f64>,
    pub macd_value: Option<f64>,
    pub macd_signal: Option<f64>,
    pub macd_histogram: Option<f64>,
    
    // Derived Values
    pub ema9_slope: Option<f64>,
    pub price_above_ema9: bool,
    pub volume_ratio: Option<f64>, // Current volume vs average volume
}

impl TechnicalIndicators {
    pub fn new(symbol: String, price: f64, volume: u64) -> Self {
        Self {
            symbol,
            timestamp: Utc::now(),
            price,
            volume,
            sma_20: None,
            sma_50: None,
            ema_9: None,
            ema_21: None,
            rsi_14: None,
            macd_value: None,
            macd_signal: None,
            macd_histogram: None,
            ema9_slope: None,
            price_above_ema9: false,
            volume_ratio: None,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.ema_9.is_some() && 
        self.rsi_14.is_some() && 
        self.macd_value.is_some() && 
        self.macd_signal.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskParameters {
    pub max_positions: u32,
    pub max_portfolio_exposure_percent: f64,
    pub max_single_position_percent: f64,
    pub default_stop_loss_percent: f64,
    pub max_loss_per_trade_dollars: Option<f64>,
    pub max_daily_loss_dollars: Option<f64>,
    pub require_volume_confirmation: bool,
    pub min_volume_ratio: f64, // Minimum volume vs average
}

impl RiskParameters {
    /// Load risk parameters from database
    /// This replaces the hardcoded Default implementation
    pub async fn load_from_database(db: &crate::database::Database) -> anyhow::Result<Self> {
        let row = sqlx::query_as::<_, (i64, f64, f64, f64, Option<f64>, Option<f64>, i64, f64)>(
            "SELECT 
                max_positions,
                max_portfolio_exposure_percent,
                max_single_position_percent,
                default_stop_loss_percent,
                max_loss_per_trade_dollars,
                max_daily_loss_dollars,
                require_volume_confirmation,
                min_volume_ratio
            FROM risk_config WHERE id = 1"
        )
        .fetch_one(db)
        .await?;

        Ok(Self {
            max_positions: row.0 as u32,
            max_portfolio_exposure_percent: row.1,
            max_single_position_percent: row.2,
            default_stop_loss_percent: row.3,
            max_loss_per_trade_dollars: row.4,
            max_daily_loss_dollars: row.5,
            require_volume_confirmation: row.6 != 0,
            min_volume_ratio: row.7,
        })
    }

    /// Save risk parameters to database
    pub async fn save_to_database(&self, db: &crate::database::Database) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE risk_config SET
                max_positions = ?,
                max_portfolio_exposure_percent = ?,
                max_single_position_percent = ?,
                default_stop_loss_percent = ?,
                max_loss_per_trade_dollars = ?,
                max_daily_loss_dollars = ?,
                require_volume_confirmation = ?,
                min_volume_ratio = ?,
                updated_at = strftime('%s', 'now')
            WHERE id = 1"
        )
        .bind(self.max_positions)
        .bind(self.max_portfolio_exposure_percent)
        .bind(self.max_single_position_percent)
        .bind(self.default_stop_loss_percent)
        .bind(self.max_loss_per_trade_dollars)
        .bind(self.max_daily_loss_dollars)
        .bind(self.require_volume_confirmation)
        .bind(self.min_volume_ratio)
        .execute(db)
        .await?;

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedTradingRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub active: bool,
    pub priority: u8, // 1 = highest, 10 = lowest
    
    // Entry Conditions
    pub entry_conditions: RuleConditions,
    
    // Risk Management
    pub stop_loss_percent: Option<f64>,
    pub stop_loss_dollars: Option<f64>,
    pub position_size_percent: f64, // Percentage of available cash
    pub max_position_value: Option<f64>,
    
    // Exit Conditions
    pub take_profit_percent: Option<f64>,
    pub trailing_stop_percent: Option<f64>,
    pub max_hold_time_minutes: Option<u32>,
    
    // Performance Tracking
    pub times_triggered: u32,
    pub successful_trades: u32,
    pub total_pnl: f64,
    pub average_hold_time_minutes: f64,
    
    pub created_at: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
    pub last_triggered: Option<DateTime<Utc>>,
}

impl EnhancedTradingRule {
    pub fn new(id: String, name: String, description: String) -> Self {
        Self {
            id,
            name,
            description,
            active: true,
            priority: 5,
            entry_conditions: RuleConditions::default(),
            stop_loss_percent: Some(10.0),
            stop_loss_dollars: None,
            position_size_percent: 10.0,
            max_position_value: None,
            take_profit_percent: None,
            trailing_stop_percent: None,
            max_hold_time_minutes: None,
            times_triggered: 0,
            successful_trades: 0,
            total_pnl: 0.0,
            average_hold_time_minutes: 0.0,
            created_at: Utc::now(),
            last_modified: Utc::now(),
            last_triggered: None,
        }
    }

    pub fn success_rate(&self) -> f64 {
        if self.times_triggered == 0 {
            0.0
        } else {
            (self.successful_trades as f64 / self.times_triggered as f64) * 100.0
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleConditions {
    // Price Conditions
    pub min_price: Option<f64>,
    pub max_price: Option<f64>,
    pub price_change_percent_min: Option<f64>,
    
    // Volume Conditions
    pub min_volume: Option<u64>,
    pub volume_ratio_min: Option<f64>, // vs average volume
    
    // Technical Indicator Conditions
    pub rsi_min: Option<f64>,
    pub rsi_max: Option<f64>,
    pub macd_positive: Option<bool>,
    pub macd_above_signal: Option<bool>,
    pub price_above_ema9: Option<bool>,
    pub price_above_sma20: Option<bool>,
    pub ema9_increasing: Option<bool>,
    pub ema9_above_ema21: Option<bool>,
    
    // News/Sentiment Conditions
    pub require_positive_sentiment: Option<bool>,
    pub min_news_sentiment_score: Option<f64>,
    
    // Market Conditions
    pub market_hours_only: bool,
    pub exclude_earnings_days: bool,
}

impl Default for RuleConditions {
    fn default() -> Self {
        Self {
            min_price: Some(5.0),   // Minimum $5 stock price
            max_price: Some(500.0), // Maximum $500 stock price
            price_change_percent_min: None,
            min_volume: None,
            volume_ratio_min: Some(1.5), // 1.5x average volume
            rsi_min: Some(30.0),    // Oversold but not extreme
            rsi_max: Some(70.0),    // Not overbought
            macd_positive: Some(true),
            macd_above_signal: Some(true),
            price_above_ema9: Some(true),
            price_above_sma20: None,
            ema9_increasing: Some(true),
            ema9_above_ema21: Some(true),
            require_positive_sentiment: None,
            min_news_sentiment_score: None,
            market_hours_only: true,
            exclude_earnings_days: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleEvaluationResult {
    pub rule_id: String,
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub action: RuleAction,
    pub confidence_score: f64, // 0.0 to 1.0
    pub conditions_met: Vec<String>,
    pub conditions_failed: Vec<String>,
    pub indicators_snapshot: TechnicalIndicators,
    pub risk_assessment: RiskAssessment,
    pub execution_time_micros: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuleAction {
    Buy { 
        quantity: u64, 
        price_limit: Option<f64>,
        stop_loss_price: f64,
        take_profit_price: Option<f64>,
    },
    Sell { 
        quantity: u64, 
        price_limit: Option<f64>,
        reason: SellReason,
    },
    Hold { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SellReason {
    StopLoss,
    TakeProfit,
    TrailingStop,
    TimeLimit,
    RuleChange,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub position_size_dollars: f64,
    pub max_loss_dollars: f64,
    pub portfolio_exposure_after: f64,
    pub risk_reward_ratio: Option<f64>,
    pub risk_level: RiskLevel,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}