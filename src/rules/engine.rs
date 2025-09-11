// src/rules/engine.rs  
// Branch: 9.2.25.1

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tokio::time::{interval, Duration};
use anyhow::{Result, anyhow};
use tracing::{info, warn, error, debug};
use uuid::Uuid;

use crate::types::{
    EnhancedTradingRule, PortfolioState, RiskParameters, 
    RuleEvaluationResult, RuleAction, TechnicalIndicators,
    RiskAssessment, Position, PositionSide, PositionStatus
};
use crate::rules::{ConditionEvaluator, RiskManager, IndicatorCache};
use crate::data::DataModule;
use crate::broker::BrokerModule;
use crate::database::Database;

/// Core rules engine for high-frequency algorithmic trading
/// Designed for <5ms decision latency with momentum/breakout strategy focus
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
    evaluation_interval_ms: u64,
}

impl RulesEngine {
    pub fn new(
        data_module: Arc<DataModule>,
        broker_module: Arc<tokio::sync::RwLock<BrokerModule>>,
        database: Arc<Database>,
        risk_parameters: RiskParameters,
        initial_cash: f64,
    ) -> Self {
        let indicator_cache = Arc::new(IndicatorCache::new(data_module.clone()));
        
        Self {
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
            evaluation_interval_ms: 1000, // 1 second default
        }
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

        // Start main evaluation loop
        self.start_evaluation_loop().await;
        
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
    async fn start_evaluation_loop(&self) {
        let mut interval = interval(Duration::from_millis(self.evaluation_interval_ms));
        
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

        // Track performance
        if total_time.as_millis() > 5 {
            warn!("Evaluation cycle took {}ms (target: <5ms)", total_time.as_millis());
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
        if evaluation_time.as_micros() > 5000 { // 5ms target
            warn!("Symbol {} evaluation took {}μs", symbol, evaluation_time.as_micros());
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
        let action = self.determine_action(rule, &indicators.symbol, portfolio)?;

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
    fn determine_action(
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
        let portfolio_state = portfolio;
        let quantity = self.risk_manager.calculate_position_size(
            portfolio_state, 
            rule, 
            portfolio.positions.get(symbol)
                .map(|p| p.current_price)
                .unwrap_or(100.0) // Default price if no position
        )?;

        if quantity == 0 {
            return Ok(RuleAction::Hold { 
                reason: "Insufficient funds or position size too small".to_string() 
            });
        }

        let entry_price = 100.0; // Would get from indicators or market data
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
            side: PositionSide::Long,
            quantity,
            entry_price: price,
            current_price: price,
            opened_at: chrono::Utc::now(),
            closed_at: None,
            status: PositionStatus::Open,
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
                warn!("Failed to load symbols from database, using fallback: {}", e);
                // Fallback to LightSpeed certification test symbols
                let fallback_symbols = vec![
                    "GOOGL",   // Immediate fill
                    "AMZN",    // Partial fill
                    "TSLA",    // No fill
                    "MSFT",    // Rejection
                    "CHWY",    // Multiple partial fills
                    "F",       // Multiple partial fills
                    "GE",      // Multiple partial fills
                    "ORCL"     // Cancel testing
                ];
                
                for symbol in fallback_symbols {
                    symbols.insert(symbol.to_string());
                }
            }
        }
        
        symbols.into_iter().collect()
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
        // In production, this would write to database
        debug!("Decision logged: {} for {} - {:?} (confidence: {:.2})", 
            result.rule_id, result.symbol, result.action, result.confidence_score);
        Ok(())
    }

    /// Update performance tracking metrics
    fn update_performance_metrics(&self, execution_time: std::time::Duration) {
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
        let mut rules = self.rules.write()
            .map_err(|e| anyhow!("Failed to acquire rules lock: {}", e))?;
        
        info!("Adding trading rule: {} ({})", rule.name, rule.id);
        rules.insert(rule.id.clone(), rule);
        
        Ok(())
    }

    /// Remove a trading rule
    pub async fn remove_rule(&self, rule_id: &str) -> Result<()> {
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
}

#[derive(Debug)]
pub struct EnginePerformanceStats {
    pub total_decisions: u64,
    pub average_decision_time_micros: u64,
    pub cache_stats: crate::rules::cache::CacheStats,
}