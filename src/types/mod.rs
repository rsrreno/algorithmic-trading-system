use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::VecDeque;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleConditions {
    pub macd_positive: Option<bool>,
    pub price_above_ema9: Option<bool>,
    pub ema9_increasing: Option<bool>,
    pub min_price: Option<f64>,
    pub max_price: Option<f64>,
    pub rsi_min: Option<f64>,
    pub rsi_max: Option<f64>,
}

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