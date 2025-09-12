// src/rules/engine.rs  
// Branch: 9.2.25.1

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tokio::time::{interval, Duration};
use anyhow::{Result, anyhow};
use tracing::{info, warn, error, debug};
use uuid::Uuid;
use sqlx::Row;

use crate::types::{
    EnhancedTradingRule, PortfolioState, RiskParameters, 
    RuleEvaluationResult, RuleAction, TechnicalIndicators,
    RiskAssessment, Position, PositionSide, PositionStatus
};
use crate::rules::{ConditionEvaluator, RiskManager, IndicatorCache};
use crate::data::DataModule;
use crate::broker::BrokerModule;
use crate::database::Database;
use crate::config::{RulesEngineConfig, PriceFallbackStrategy};

/// Core rules engine for high-frequency algorithmic trading
/// Designed for <5ms decision latency with momentum/breakout strategy focus
#[derive(Clone)]
pub struct RulesEngine {
    // Core Components
    condition_evaluator: ConditionEvaluator,
    risk_manager: RiskManager,
    indicator_cache: Arc<IndicatorCache>,
    
    // Trading Rules
    rules: Arc<RwLock<HashMap<String, EnhancedTradingRule>>>,
    
    // Portfolio State
    portfolio: Arc<RwLock<PortfolioState>>,
    
    // External Modules
    data_module: Arc<DataModule>,
    broker_module: Arc<tokio::sync::RwLock<BrokerModule>>,
    database: Arc<Database>,
    
    // Performance Tracking
    decision_count: Arc<RwLock<u64>>,
    total_decision_time_micros: Arc<RwLock<u64>>,
    
    // Configuration
    enabled: Arc<RwLock<bool>>,
    config: RulesEngineConfig,
}

impl RulesEngine {
    pub fn new(
        data_module: Arc<DataModule>,
        broker_module: Arc<tokio::sync::RwLock<BrokerModule>>,
        database: Arc<Database>,
        risk_parameters: RiskParameters,
        initial_cash: f64,
        config: RulesEngineConfig,
    ) -> Result<Self> {
        // Validate required configuration
        if config.enabled && config.evaluation_interval_ms.is_none() {
            anyhow::bail!("Rules engine evaluation interval must be configured when enabled");
        }
        if config.enabled && config.max_decision_time_ms.is_none() {
            anyhow::bail!("Rules engine max decision time must be configured when enabled");
        }
        let indicator_cache = Arc::new(IndicatorCache::new(data_module.clone()));
        
        Ok(Self {
            condition_evaluator: ConditionEvaluator::new(),
            risk_manager: RiskManager::new(risk_parameters.clone()),
            indicator_cache: indicator_cache.clone(),
            rules: Arc::new(RwLock::new(HashMap::new())),
            portfolio: Arc::new(RwLock::new(PortfolioState::new(
                initial_cash, 
                risk_parameters.max_positions
            ))),
            data_module,
            broker_module,
            database,
            decision_count: Arc::new(RwLock::new(0)),
            total_decision_time_micros: Arc::new(RwLock::new(0)),
            enabled: Arc::new(RwLock::new(false)),
            config,
        })
    }

    /// Start the rules engine with continuous evaluation
    pub async fn start(&self) -> Result<()> {
        info!("Starting rules engine");
        
        // Enable the engine
        {
            let mut enabled = self.enabled.write().map_err(|e| anyhow!("Failed to acquire enabled lock: {}", e))?;
            *enabled = true;
        }

        // Start cache maintenance
        IndicatorCache::start_maintenance_task(self.indicator_cache.clone());

        // Start main evaluation loop in background task
        let evaluation_task = self.start_evaluation_loop_background();
        
        info!("✅ Rules engine evaluation loop started in background");
        
        Ok(())
    }

    /// Stop the rules engine
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping rules engine");
        
        let mut enabled = self.enabled.write().map_err(|e| anyhow!("Failed to acquire enabled lock: {}", e))?;
        *enabled = false;
        
        info!("Rules engine stopped");
        Ok(())
    }

    /// Main evaluation loop - continuously evaluates all rules
    async fn start_evaluation_loop(&self) -> Result<()> {
        // Get evaluation interval from configuration (required when enabled)
        let evaluation_interval_ms = self.config.evaluation_interval_ms
            .ok_or_else(|| anyhow!("Evaluation interval not configured"))?;
            
        let mut interval = interval(Duration::from_millis(evaluation_interval_ms));
        
        loop {
            interval.tick().await;
            
            // Check if still enabled
            let is_enabled = {
                if let Ok(enabled) = self.enabled.read() {
                    *enabled
                } else {
                    error!("Failed to read enabled status");
                    break;
                }
            };
            
            if !is_enabled {
                break;
            }

            // Perform evaluation cycle
            if let Err(e) = self.evaluate_all_rules().await {
                error!("Error in evaluation cycle: {}", e);
            }
        }
        
        info!("Evaluation loop terminated");
        Ok(())
    }

    /// Start evaluation loop in background task (non-blocking)
    fn start_evaluation_loop_background(&self) -> tokio::task::JoinHandle<()> {
        let engine_clone = self.clone(); // Need to implement Clone for RulesEngine
        
        tokio::spawn(async move {
            if let Err(e) = engine_clone.start_evaluation_loop().await {
                error!("Rules engine evaluation loop failed: {}", e);
            }
        })
    }

    /// Evaluate all active rules against current market conditions
    async fn evaluate_all_rules(&self) -> Result<()> {
        let start_time = Instant::now();
        
        // Get all active rules
        let active_rules: Vec<EnhancedTradingRule> = {
            let rules = self.rules.read().map_err(|e| anyhow!("Failed to acquire rules lock: {}", e))?;
            rules.values()
                .filter(|rule| rule.active)
                .cloned()
                .collect()
        };

        if active_rules.is_empty() {
            debug!("No active rules to evaluate");
            return Ok(());
        }

        info!("Evaluating {} active rules", active_rules.len());

        // Get current portfolio state
        let portfolio = {
            let portfolio = self.portfolio.read().map_err(|e| anyhow!("Failed to acquire portfolio lock: {}", e))?;
            portfolio.clone()
        };

        // Get symbols to evaluate (from rules or existing positions)
        let symbols = self.get_evaluation_symbols(&active_rules, &portfolio).await;
        
        // Evaluate each symbol against all applicable rules
        for symbol in symbols {
            if let Err(e) = self.evaluate_symbol(&symbol, &active_rules, &portfolio).await {
                error!("Error evaluating symbol {}: {}", symbol, e);
            }
        }

        let total_time = start_time.elapsed();
        debug!("Rule evaluation cycle completed in {}ms", total_time.as_millis());

        // Track performance using configured thresholds
        let max_decision_time_ms = self.config.max_decision_time_ms.unwrap_or(5); // Default 5ms if not configured
        let warning_threshold_ms = self.config.warning_threshold_ms.unwrap_or(max_decision_time_ms / 2);
        
        if total_time.as_millis() > warning_threshold_ms as u128 {
            warn!("Evaluation cycle took {}ms (warning threshold: {}ms, max: {}ms)", 
                total_time.as_millis(), warning_threshold_ms, max_decision_time_ms);
        }

        Ok(())
    }

    /// Evaluate a specific symbol against all applicable rules
    async fn evaluate_symbol(
        &self,
        symbol: &str,
        rules: &[EnhancedTradingRule],
        portfolio: &PortfolioState,
    ) -> Result<()> {
        let start_time = Instant::now();

        // Get current technical indicators (from cache for speed)
        let indicators = match self.indicator_cache.get_indicators(symbol).await {
            Ok(indicators) => indicators,
            Err(e) => {
                debug!("Could not get indicators for {}: {}", symbol, e);
                return Ok(()); // Skip this symbol
            }
        };

        // Ensure indicators are complete enough for evaluation
        if !indicators.is_complete() {
            debug!("Incomplete indicators for {}, skipping", symbol);
            return Ok(());
        }

        // Evaluate each rule for this symbol
        for rule in rules {
            if let Err(e) = self.evaluate_rule_for_symbol(rule, &indicators, portfolio).await {
                error!("Error evaluating rule {} for {}: {}", rule.name, symbol, e);
            }
        }

        let evaluation_time = start_time.elapsed();
        let max_decision_time_ms = self.config.max_decision_time_ms.unwrap_or(5);
        let max_decision_time_micros = max_decision_time_ms * 1000;
        
        if evaluation_time.as_micros() > max_decision_time_micros as u128 {
            warn!("Symbol {} evaluation took {}μs (max: {}μs)", 
                symbol, evaluation_time.as_micros(), max_decision_time_micros);
        }

        Ok(())
    }

    /// Evaluate a specific rule for a symbol
    async fn evaluate_rule_for_symbol(
        &self,
        rule: &EnhancedTradingRule,
        indicators: &TechnicalIndicators,
        portfolio: &PortfolioState,
    ) -> Result<()> {
        let start_time = Instant::now();

        // Evaluate rule conditions
        let evaluation_result = self.condition_evaluator.evaluate_rule(rule, indicators)?;
        
        if !evaluation_result.passed {
            debug!("Rule {} conditions not met for {}", rule.name, indicators.symbol);
            return Ok(());
        }

        info!("Rule {} triggered for {} (confidence: {:.2})", 
            rule.name, indicators.symbol, evaluation_result.confidence_score);

        // Determine action based on existing position
        let action = self.determine_action(rule, &indicators.symbol, portfolio).await?;

        // Perform risk assessment
        let risk_assessment = self.risk_manager.assess_trade_risk(
            portfolio, rule, indicators, &action
        )?;

        // Execute action if risk is acceptable
        if self.should_execute_action(&risk_assessment) {
            self.execute_action(rule, &action, &risk_assessment, indicators).await?;
        } else {
            warn!("Action blocked by risk assessment for {} (risk: {:?})", 
                indicators.symbol, risk_assessment.risk_level);
        }

        // Update performance metrics
        let execution_time = start_time.elapsed();
        self.update_performance_metrics(execution_time);

        // Create evaluation result for logging
        let result = RuleEvaluationResult {
            rule_id: rule.id.clone(),
            symbol: indicators.symbol.clone(),
            timestamp: chrono::Utc::now(),
            action,
            confidence_score: evaluation_result.confidence_score,
            conditions_met: evaluation_result.conditions_met,
            conditions_failed: evaluation_result.conditions_failed,
            indicators_snapshot: indicators.clone(),
            risk_assessment,
            execution_time_micros: execution_time.as_micros() as u64,
        };

        // Log the decision (in production, this would go to database)
        self.log_decision(result).await?;

        Ok(())
    }

    /// Determine what action to take based on rule and current positions
    async fn determine_action(
        &self,
        rule: &EnhancedTradingRule,
        symbol: &str,
        portfolio: &PortfolioState,
    ) -> Result<RuleAction> {
        // Check if we already have a position in this symbol
        if let Some(position) = portfolio.positions.get(symbol) {
            match position.status {
                PositionStatus::Open => {
                    // We have an open position - consider selling
                    let sell_reason = crate::types::SellReason::RuleChange; // Simplified
                    return Ok(RuleAction::Sell {
                        quantity: position.quantity,
                        price_limit: None,
                        reason: sell_reason,
                    });
                }
                _ => {
                    // Position is closing or closed, treat as no position
                }
            }
        }

        // No position - consider buying
        let current_price = self.get_current_price_with_fallback(symbol).await?;
        let quantity = self.risk_manager.calculate_position_size(
            portfolio, 
            rule, 
            current_price
        )?;

        if quantity == 0 {
            return Ok(RuleAction::Hold { 
                reason: "Insufficient funds or position size too small".to_string() 
            });
        }

        let entry_price = current_price;
        let stop_loss_price = self.risk_manager.calculate_stop_loss_price(rule, entry_price)?;

        Ok(RuleAction::Buy {
            quantity,
            price_limit: None,
            stop_loss_price,
            take_profit_price: rule.take_profit_percent
                .map(|pct| entry_price * (1.0 + pct / 100.0)),
        })
    }

    /// Check if action should be executed based on risk assessment
    fn should_execute_action(&self, risk_assessment: &RiskAssessment) -> bool {
        match risk_assessment.risk_level {
            crate::types::RiskLevel::Critical => false,
            crate::types::RiskLevel::High => risk_assessment.warnings.len() <= 1,
            crate::types::RiskLevel::Medium => true,
            crate::types::RiskLevel::Low => true,
        }
    }

    /// Execute the determined action through the broker
    async fn execute_action(
        &self,
        _rule: &EnhancedTradingRule,
        action: &RuleAction,
        _risk_assessment: &RiskAssessment,
        indicators: &TechnicalIndicators,
    ) -> Result<()> {
        match action {
            RuleAction::Buy { quantity, price_limit, stop_loss_price, .. } => {
                info!("Executing BUY order: {} shares of {} (stop loss: ${:.2})", 
                    quantity, indicators.symbol, stop_loss_price);
                
                // Execute buy order through broker module
                let broker = self.broker_module.read().await;
                let order_result = broker.place_stock_order(
                    &indicators.symbol,
                    "BUY",
                    "MARKET",
                    *quantity,
                    *price_limit,
                ).await;

                match order_result {
                    Ok(order_id) => {
                        info!("Buy order placed successfully: {}", order_id);
                        
                        // Update portfolio (simplified - in production, wait for fill confirmation)
                        self.update_portfolio_after_buy(&indicators.symbol, *quantity, indicators.price).await?;
                    }
                    Err(e) => {
                        error!("Failed to place buy order for {}: {}", indicators.symbol, e);
                    }
                }
            }
            RuleAction::Sell { quantity, price_limit, reason } => {
                info!("Executing SELL order: {} shares of {} (reason: {:?})", 
                    quantity, indicators.symbol, reason);
                
                let broker = self.broker_module.read().await;
                let order_result = broker.place_stock_order(
                    &indicators.symbol,
                    "SELL",
                    "MARKET", 
                    *quantity,
                    *price_limit,
                ).await;

                match order_result {
                    Ok(order_id) => {
                        info!("Sell order placed successfully: {}", order_id);
                        self.update_portfolio_after_sell(&indicators.symbol, *quantity).await?;
                    }
                    Err(e) => {
                        error!("Failed to place sell order for {}: {}", indicators.symbol, e);
                    }
                }
            }
            RuleAction::Hold { reason } => {
                debug!("Holding position for {}: {}", indicators.symbol, reason);
            }
        }

        Ok(())
    }

    /// Update portfolio state after successful buy
    async fn update_portfolio_after_buy(&self, symbol: &str, quantity: u64, price: f64) -> Result<()> {
        let mut portfolio = self.portfolio.write()
            .map_err(|e| anyhow!("Failed to acquire portfolio lock: {}", e))?;
        
        let position_value = quantity as f64 * price;
        portfolio.available_cash -= position_value;
        portfolio.total_exposure += position_value;
        
        let position = Position {
            id: Uuid::new_v4().to_string(),
            symbol: symbol.to_string(),
            quantity,
            avg_cost_basis: price,
            total_cost: quantity as f64 * price,
            current_price: price,
            realized_pnl: 0.0,
            opened_at: chrono::Utc::now(),
            closed_at: None,
            status: PositionStatus::Open,
            session_id: "rules-engine-session".to_string(),
        };
        
        portfolio.positions.insert(symbol.to_string(), position);
        portfolio.current_position_count += 1;
        portfolio.last_updated = chrono::Utc::now();
        
        info!("Portfolio updated after buy: {} shares of {} at ${:.2}", quantity, symbol, price);
        Ok(())
    }

    /// Update portfolio state after successful sell
    async fn update_portfolio_after_sell(&self, symbol: &str, quantity: u64) -> Result<()> {
        let mut portfolio = self.portfolio.write()
            .map_err(|e| anyhow!("Failed to acquire portfolio lock: {}", e))?;
        
        // Get the current price first to avoid borrow conflicts
        let current_price = portfolio.positions.get(symbol)
            .map(|p| p.current_price)
            .unwrap_or(0.0);
            
        let sale_value = quantity as f64 * current_price;
        portfolio.available_cash += sale_value;
        portfolio.total_exposure -= sale_value;
        
        // Now modify the position
        if let Some(position) = portfolio.positions.get_mut(symbol) {
            position.quantity -= quantity;
            if position.quantity == 0 {
                position.status = PositionStatus::Closed;
                position.closed_at = Some(chrono::Utc::now());
                portfolio.current_position_count -= 1;
            }
        }
        
        portfolio.last_updated = chrono::Utc::now();
        
        info!("Portfolio updated after sell: {} shares of {} at ${:.2}", 
            quantity, symbol, current_price);
        
        Ok(())
    }

    /// Get symbols that need evaluation (from database watchlist and existing positions)
    async fn get_evaluation_symbols(
        &self,
        _rules: &[EnhancedTradingRule],
        portfolio: &PortfolioState,
    ) -> Vec<String> {
        let mut symbols = std::collections::HashSet::new();
        
        // Add symbols from existing positions
        for symbol in portfolio.positions.keys() {
            symbols.insert(symbol.clone());
        }
        
        // Load active symbols from database watchlist
        match self.load_active_symbols_from_database().await {
            Ok(watchlist_symbols) => {
                for symbol in watchlist_symbols {
                    symbols.insert(symbol);
                }
                debug!("Loaded {} symbols from database watchlist", symbols.len() - portfolio.positions.len());
            }
            Err(e) => {
                warn!("Failed to load symbols from database: {}", e);
                
                // Use configured fallback symbols if available
                if !self.config.default_watchlist_symbols.is_empty() {
                    info!("Using configured fallback symbols: {:?}", self.config.default_watchlist_symbols);
                    for symbol in &self.config.default_watchlist_symbols {
                        symbols.insert(symbol.clone());
                    }
                } else {
                    warn!("No fallback symbols configured and database unavailable - rules engine will only evaluate existing positions");
                }
            }
        }
        
        symbols.into_iter().collect()
    }

    /// Get current price with configurable fallback strategy
    async fn get_current_price_with_fallback(&self, symbol: &str) -> Result<f64> {
        // Try to get current market price from snapshot API first
        match self.data_module.get_current_market_price(symbol).await {
            Ok(current_price) => {
                if current_price > 0.0 {
                    debug!("Using current market price for {}: ${:.2}", symbol, current_price);
                    return Ok(current_price);
                }
            }
            Err(e) => {
                debug!("Failed to get current market price for {}: {}", symbol, e);
            }
        }
        
        // Fallback to previous day data if current price not available
        match self.data_module.get_symbol_data(symbol).await {
            Ok(data) => {
                if data.close > 0.0 {
                    debug!("Using previous day close for {}: ${:.2}", symbol, data.close);
                    return Ok(data.close);
                }
            }
            Err(e) => {
                debug!("Failed to get previous day price for {}: {}", symbol, e);
            }
        }
        
        // Apply fallback strategy based on configuration
        match &self.config.price_fallback_strategy {
            PriceFallbackStrategy::LastKnown => {
                // Try to get last known price from portfolio or cache
                let portfolio = self.portfolio.read().map_err(|e| anyhow!("Portfolio lock error: {}", e))?;
                if let Some(position) = portfolio.positions.get(symbol) {
                    Ok(position.current_price)
                } else {
                    anyhow::bail!("No last known price available for {}", symbol)
                }
            }
            PriceFallbackStrategy::MarketClose => {
                // Try to get previous close price
                match self.data_module.get_previous_day_cached(symbol).await {
                    Ok(bar) => {
                        if let Some(close_price) = bar.close {
                            if close_price > 0.0 {
                                return Ok(close_price);
                            }
                        }
                        anyhow::bail!("Invalid market close price for {}", symbol)
                    }
                    Err(e) => anyhow::bail!("Failed to get market close price for {}: {}", symbol, e)
                }
            }
            PriceFallbackStrategy::UserConfigured(price) => {
                if *price > 0.0 {
                    Ok(*price)
                } else {
                    anyhow::bail!("Invalid user configured price: {}", price)
                }
            }
            PriceFallbackStrategy::Refuse => {
                anyhow::bail!("Price fallback strategy is set to refuse execution - no price available for {}", symbol)
            }
        }
    }

    /// Load active symbols from database watchlist
    async fn load_active_symbols_from_database(&self) -> Result<Vec<String>> {
        let rows = sqlx::query_as::<_, (String,)>(
            "SELECT symbol FROM watchlist_symbols WHERE active = 1 ORDER BY symbol"
        )
        .fetch_all(&*self.database)
        .await?;

        Ok(rows.into_iter().map(|(symbol,)| symbol).collect())
    }

    /// Log decision for audit trail and analysis
    async fn log_decision(&self, result: RuleEvaluationResult) -> Result<()> {
        // Only log if configured to do so
        if !self.config.log_all_decisions && !self.config.track_performance {
            return Ok(());
        }
        
        if self.config.log_all_decisions {
            info!("Decision logged: {} for {} - {:?} (confidence: {:.2})", 
                result.rule_id, result.symbol, result.action, result.confidence_score);
        } else {
            debug!("Decision logged: {} for {} - {:?} (confidence: {:.2})", 
                result.rule_id, result.symbol, result.action, result.confidence_score);
        }
        
        // Store in database if performance tracking is enabled and configured history retention
        if self.config.track_performance && self.config.decision_history_days.is_some() {
            // TODO: Implement database storage for decision history
            debug!("Performance tracking enabled - would store decision in database");
        }
        
        Ok(())
    }

    /// Update performance tracking metrics
    fn update_performance_metrics(&self, execution_time: std::time::Duration) {
        // Only track metrics if configured to do so
        if !self.config.track_performance {
            return;
        }
        
        if let (Ok(mut count), Ok(mut total_time)) = (
            self.decision_count.write(),
            self.total_decision_time_micros.write()
        ) {
            *count += 1;
            *total_time += execution_time.as_micros() as u64;
        }
    }

    /// Add a new trading rule
    pub async fn add_rule(&self, rule: EnhancedTradingRule) -> Result<()> {
        // First save to database
        self.save_rule_to_database(&rule).await?;
        
        // Then add to in-memory storage
        let mut rules = self.rules.write()
            .map_err(|e| anyhow!("Failed to acquire rules lock: {}", e))?;
        
        info!("Adding trading rule: {} ({})", rule.name, rule.id);
        rules.insert(rule.id.clone(), rule);
        
        Ok(())
    }

    /// Remove a trading rule
    pub async fn remove_rule(&self, rule_id: &str) -> Result<()> {
        // First remove from database
        self.delete_rule_from_database(rule_id).await?;
        
        // Then remove from in-memory storage
        let mut rules = self.rules.write()
            .map_err(|e| anyhow!("Failed to acquire rules lock: {}", e))?;
        
        if rules.remove(rule_id).is_some() {
            info!("Removed trading rule: {}", rule_id);
            Ok(())
        } else {
            Err(anyhow!("Rule not found: {}", rule_id))
        }
    }

    /// Get current portfolio state
    pub async fn get_portfolio(&self) -> Result<PortfolioState> {
        let portfolio = self.portfolio.read()
            .map_err(|e| anyhow!("Failed to acquire portfolio lock: {}", e))?;
        Ok(portfolio.clone())
    }

    /// Get all trading rules
    pub async fn get_all_rules(&self) -> Result<Vec<EnhancedTradingRule>> {
        let rules = self.rules.read()
            .map_err(|e| anyhow!("Failed to acquire rules lock: {}", e))?;
        Ok(rules.values().cloned().collect())
    }

    /// Get performance statistics
    pub async fn get_performance_stats(&self) -> Result<EnginePerformanceStats> {
        let (count, total_time) = {
            let count = self.decision_count.read()
                .map_err(|e| anyhow!("Failed to read decision count: {}", e))?;
            let total_time = self.total_decision_time_micros.read()
                .map_err(|e| anyhow!("Failed to read total time: {}", e))?;
            (*count, *total_time)
        };

        let average_time_micros = if count > 0 {
            total_time / count
        } else {
            0
        };

        Ok(EnginePerformanceStats {
            total_decisions: count,
            average_decision_time_micros: average_time_micros,
            cache_stats: self.indicator_cache.get_stats(),
        })
    }

    /// Load all rules from database on startup
    pub async fn load_rules_from_database(&self) -> Result<()> {
        info!("Loading trading rules from database");

        let rows = sqlx::query(
            r#"
            SELECT id, name, description, rule_type, conditions, entry_criteria, exit_criteria,
                   risk_management, symbols, active, priority, max_position_size,
                   stop_loss_percent, take_profit_percent, total_triggers, successful_trades,
                   failed_trades, total_pnl, win_rate, created_at, updated_at
            FROM trading_rules_config 
            ORDER BY priority DESC, created_at ASC
            "#
        )
        .fetch_all(&*self.database)
        .await?;

        let mut rules = self.rules.write()
            .map_err(|e| anyhow!("Failed to acquire rules lock: {}", e))?;

        let mut loaded_count = 0;
        for row in rows {
            // Get rule ID for error reporting before moving row
            let rule_id = match row.try_get::<String, _>("id") {
                Ok(id) => id,
                Err(_) => "unknown".to_string(),
            };

            match self.deserialize_rule_from_database(row).await {
                Ok(rule) => {
                    info!("Loaded rule: {} ({})", rule.name, rule.id);
                    rules.insert(rule.id.clone(), rule);
                    loaded_count += 1;
                }
                Err(e) => {
                    warn!("Failed to deserialize rule {}: {}", rule_id, e);
                }
            }
        }

        info!("✅ Loaded {} trading rules from database", loaded_count);
        Ok(())
    }

    /// Save a rule to the database
    async fn save_rule_to_database(&self, rule: &EnhancedTradingRule) -> Result<()> {
        let conditions_json = serde_json::to_string(&rule.entry_conditions)?;
        let risk_management_json = serde_json::json!({
            "position_size_percent": rule.position_size_percent,
            "stop_loss_percent": rule.stop_loss_percent,
            "take_profit_percent": rule.take_profit_percent,
            "stop_loss_dollars": rule.stop_loss_dollars,
            "max_position_value": rule.max_position_value,
            "trailing_stop_percent": rule.trailing_stop_percent
        }).to_string();

        let entry_criteria_json = serde_json::json!({
            "conditions": rule.entry_conditions
        }).to_string();

        let exit_criteria_json = serde_json::json!({
            "stop_loss_percent": rule.stop_loss_percent,
            "take_profit_percent": rule.take_profit_percent,
            "trailing_stop_percent": rule.trailing_stop_percent,
            "max_hold_time_minutes": rule.max_hold_time_minutes
        }).to_string();

        let current_time = chrono::Utc::now().timestamp();

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO trading_rules_config (
                id, session_id, name, description, rule_type, conditions, entry_criteria,
                exit_criteria, risk_management, symbols, active, priority, max_position_size,
                stop_loss_percent, take_profit_percent, created_at, updated_at,
                total_triggers, successful_trades, failed_trades, total_pnl, win_rate
            ) VALUES (
                ?, 'default-session', ?, ?, 'CUSTOM', ?, ?, ?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
            )
            "#
        )
        .bind(&rule.id)
        .bind(&rule.name)
        .bind(&rule.description)
        .bind(&conditions_json)
        .bind(&entry_criteria_json)
        .bind(&exit_criteria_json)
        .bind(&risk_management_json)
        .bind(rule.active as i32)
        .bind(rule.priority as i32)
        .bind(rule.max_position_value)
        .bind(rule.stop_loss_percent)
        .bind(rule.take_profit_percent)
        .bind(rule.created_at.timestamp())
        .bind(current_time)
        .bind(rule.times_triggered as i32)
        .bind(rule.successful_trades as i32)
        .bind(0i32) // failed_trades - not tracked yet
        .bind(rule.total_pnl)
        .bind(0.0f64) // win_rate - calculated field
        .execute(&*self.database)
        .await?;

        info!("✅ Saved rule {} to database", rule.id);
        Ok(())
    }

    /// Delete a rule from the database
    async fn delete_rule_from_database(&self, rule_id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM trading_rules_config WHERE id = ?")
            .bind(rule_id)
            .execute(&*self.database)
            .await?;

        if result.rows_affected() > 0 {
            info!("✅ Deleted rule {} from database", rule_id);
            Ok(())
        } else {
            Err(anyhow!("Rule {} not found in database", rule_id))
        }
    }

    /// Deserialize a rule from database row
    async fn deserialize_rule_from_database(&self, row: sqlx::sqlite::SqliteRow) -> Result<EnhancedTradingRule> {
        use sqlx::Row;
        
        let id: String = row.get("id");
        let name: String = row.get("name");
        let description: String = row.get("description");
        let conditions_json: String = row.get("conditions");
        let active: i32 = row.get("active");
        let priority: i32 = row.get("priority");
        let stop_loss_percent: Option<f64> = row.get("stop_loss_percent");
        let take_profit_percent: Option<f64> = row.get("take_profit_percent");
        let max_position_size: Option<f64> = row.get("max_position_size");
        let total_triggers: i32 = row.get("total_triggers");
        let successful_trades: i32 = row.get("successful_trades");
        let total_pnl: f64 = row.get("total_pnl");
        let created_at: i64 = row.get("created_at");
        let updated_at: i64 = row.get("updated_at");

        // Parse conditions from JSON
        let entry_conditions: crate::types::RuleConditions = serde_json::from_str(&conditions_json)
            .map_err(|e| anyhow!("Failed to parse conditions JSON: {}", e))?;

        // Get risk management data
        let risk_management_json: String = row.get("risk_management");
        let risk_data: serde_json::Value = serde_json::from_str(&risk_management_json)
            .unwrap_or_default();

        let position_size_percent = risk_data.get("position_size_percent")
            .and_then(|v| v.as_f64())
            .unwrap_or(10.0);

        let mut rule = EnhancedTradingRule::new(id, name, "CUSTOM".to_string());
        rule.description = description;
        rule.entry_conditions = entry_conditions;
        rule.active = active != 0;
        rule.priority = priority as u8;
        rule.position_size_percent = position_size_percent;
        rule.stop_loss_percent = stop_loss_percent;
        rule.take_profit_percent = take_profit_percent;
        rule.max_position_value = max_position_size;
        rule.times_triggered = total_triggers as u32;
        rule.successful_trades = successful_trades as u32;
        rule.total_pnl = total_pnl;
        rule.created_at = chrono::DateTime::from_timestamp(created_at, 0)
            .unwrap_or_else(chrono::Utc::now);
        rule.last_modified = chrono::DateTime::from_timestamp(updated_at, 0)
            .unwrap_or_else(chrono::Utc::now);

        Ok(rule)
    }
}

#[derive(Debug)]
pub struct EnginePerformanceStats {
    pub total_decisions: u64,
    pub average_decision_time_micros: u64,
    pub cache_stats: crate::rules::cache::CacheStats,
}