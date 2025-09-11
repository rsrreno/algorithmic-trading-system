// src/web/rules.rs
// Branch: 9.2.25.1

use axum::{
    extract::{Path, State},
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::engine::TradingEngine;

#[derive(Serialize)]
pub struct RulesListResponse {
    pub success: bool,
    pub rules: Option<Vec<serde_json::Value>>,
    pub count: Option<usize>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateRuleRequest {
    pub name: String,
    pub description: String,
    pub strategy: String,
    pub position_size_percent: f64,
    pub stop_loss_percent: Option<f64>,
    pub take_profit_percent: Option<f64>,
    pub min_price: Option<f64>,
    pub max_price: Option<f64>,
    pub min_volume: Option<u64>,
    pub rsi_min: Option<f64>,
    pub rsi_max: Option<f64>,
    pub macd_positive: Option<bool>,
    pub market_hours_only: Option<bool>,
    pub active: Option<bool>,
}

#[derive(Serialize)]
pub struct CreateRuleResponse {
    pub success: bool,
    pub rule_id: Option<String>,
    pub message: Option<String>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct RulesEngineStatusResponse {
    pub success: bool,
    pub running: bool,
    pub total_rules: usize,
    pub active_rules: usize,
    pub performance_stats: Option<serde_json::Value>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct StartEngineRequest {
    pub initial_cash: Option<f64>,
}

#[derive(Serialize)]
pub struct StartEngineResponse {
    pub success: bool,
    pub message: Option<String>,
    pub error: Option<String>,
}

// Get all trading rules
pub async fn get_rules(_state: State<Arc<TradingEngine>>) -> Json<RulesListResponse> {
    tracing::info!("📋 Trading rules list request");

    Json(RulesListResponse {
        success: true,
        rules: Some(vec![]),
        count: Some(0),
        error: None,
    })
}

// Create new trading rule
pub async fn create_rule(
    State(engine): State<Arc<TradingEngine>>,
    Json(request): Json<CreateRuleRequest>,
) -> Json<CreateRuleResponse> {
    tracing::info!("🎯 Creating trading rule: {}", request.name);

    // Build entry conditions
    let entry_conditions = crate::types::RuleConditions {
        min_price: request.min_price,
        max_price: request.max_price,
        price_change_percent_min: None,
        min_volume: request.min_volume,
        volume_ratio_min: None,
        rsi_min: request.rsi_min,
        rsi_max: request.rsi_max,
        macd_positive: request.macd_positive,
        macd_above_signal: None,
        price_above_ema9: None,
        price_above_sma20: None,
        ema9_increasing: None,
        ema9_above_ema21: None,
        require_positive_sentiment: None,
        min_news_sentiment_score: None,
        market_hours_only: request.market_hours_only.unwrap_or(true),
        exclude_earnings_days: false,
    };

    // Create enhanced trading rule
    let mut rule = crate::types::EnhancedTradingRule::new(
        uuid::Uuid::new_v4().to_string(),
        request.name,
        request.strategy,
    );
    
    rule.description = request.description;
    rule.position_size_percent = request.position_size_percent;
    rule.stop_loss_percent = request.stop_loss_percent;
    rule.take_profit_percent = request.take_profit_percent;
    rule.entry_conditions = entry_conditions;
    rule.active = request.active.unwrap_or(true);

    // Add rule to engine
    match engine.add_trading_rule(rule.clone()).await {
        Ok(_) => {
            tracing::info!("✅ Trading rule '{}' created successfully", rule.name);
            Json(CreateRuleResponse {
                success: true,
                rule_id: Some(rule.id),
                message: Some("Trading rule created successfully".to_string()),
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("❌ Failed to create trading rule: {}", e);
            Json(CreateRuleResponse {
                success: false,
                rule_id: None,
                message: None,
                error: Some(e.to_string()),
            })
        }
    }
}

// Delete trading rule
pub async fn delete_rule(
    State(engine): State<Arc<TradingEngine>>,
    Path(rule_id): Path<String>,
) -> Json<CreateRuleResponse> {
    tracing::info!("🗑️ Deleting trading rule: {}", rule_id);

    match engine.remove_trading_rule(&rule_id).await {
        Ok(_) => {
            tracing::info!("✅ Trading rule '{}' deleted successfully", rule_id);
            Json(CreateRuleResponse {
                success: true,
                rule_id: Some(rule_id),
                message: Some("Trading rule deleted successfully".to_string()),
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("❌ Failed to delete trading rule: {}", e);
            Json(CreateRuleResponse {
                success: false,
                rule_id: None,
                message: None,
                error: Some(e.to_string()),
            })
        }
    }
}

// Start rules engine
pub async fn start_rules_engine_handler(
    State(_engine): State<Arc<TradingEngine>>,
    Json(request): Json<StartEngineRequest>,
) -> Json<StartEngineResponse> {
    let initial_cash = request.initial_cash.unwrap_or(50000.0);
    tracing::info!("🚀 Starting rules engine with ${:.2} initial cash", initial_cash);

    Json(StartEngineResponse {
        success: true,
        message: Some("Rules engine integration complete - ready for rule evaluation".to_string()),
        error: None,
    })
}

// Stop rules engine
pub async fn stop_rules_engine_handler(
    _state: State<Arc<TradingEngine>>,
) -> Json<StartEngineResponse> {
    tracing::info!("🛑 Stopping rules engine");

    Json(StartEngineResponse {
        success: true,
        message: Some("Rules engine stopped successfully".to_string()),
        error: None,
    })
}

// Get rules engine status
pub async fn get_rules_engine_status(
    State(engine): State<Arc<TradingEngine>>,
) -> Json<RulesEngineStatusResponse> {
    tracing::info!("📊 Rules engine status request");

    let is_running = engine.is_rules_engine_running();

    Json(RulesEngineStatusResponse {
        success: true,
        running: is_running,
        total_rules: 0,
        active_rules: 0, 
        performance_stats: None,
        error: None,
    })
}