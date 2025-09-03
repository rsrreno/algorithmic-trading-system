// src/rules/risk.rs
// Branch: 9.2.25.1

use std::time::Instant;
use anyhow::{Result, anyhow};
use tracing::{info, warn};

use crate::types::{
    PortfolioState, RiskParameters, RiskAssessment, RiskLevel, 
    EnhancedTradingRule, TechnicalIndicators, RuleAction
};

/// Risk management system for trading decisions
/// Ensures portfolio safety and position sizing according to risk parameters
#[derive(Debug)]
pub struct RiskManager {
    risk_params: RiskParameters,
}

impl RiskManager {
    pub fn new(risk_params: RiskParameters) -> Self {
        Self { risk_params }
    }

    /// Assess risk for a potential trade
    /// Returns risk assessment with position sizing and safety checks
    pub fn assess_trade_risk(
        &self,
        portfolio: &PortfolioState,
        rule: &EnhancedTradingRule,
        indicators: &TechnicalIndicators,
        intended_action: &RuleAction,
    ) -> Result<RiskAssessment> {
        let start_time = Instant::now();

        match intended_action {
            RuleAction::Buy { .. } => self.assess_buy_risk(portfolio, rule, indicators),
            RuleAction::Sell { .. } => self.assess_sell_risk(portfolio, rule, indicators),
            RuleAction::Hold { .. } => self.assess_hold_risk(portfolio),
        }
        .map(|mut assessment| {
            assessment.warnings.push(format!(
                "Risk assessment completed in {}μs", 
                start_time.elapsed().as_micros()
            ));
            assessment
        })
    }

    /// Assess risk for a buy order
    fn assess_buy_risk(
        &self,
        portfolio: &PortfolioState,
        rule: &EnhancedTradingRule,
        indicators: &TechnicalIndicators,
    ) -> Result<RiskAssessment> {
        let mut warnings = Vec::new();
        
        // Check if we can open more positions
        if !portfolio.can_open_position() {
            return Ok(RiskAssessment {
                position_size_dollars: 0.0,
                max_loss_dollars: 0.0,
                portfolio_exposure_after: portfolio.get_exposure_percentage(),
                risk_reward_ratio: None,
                risk_level: RiskLevel::Critical,
                warnings: vec!["Maximum positions reached, cannot open new position".to_string()],
            });
        }

        // Calculate position size based on rule and risk parameters
        let available_cash = portfolio.available_cash;
        let rule_position_percent = rule.position_size_percent;
        let max_single_position_percent = self.risk_params.max_single_position_percent;
        
        // Use the more conservative of rule setting or risk parameter
        let effective_position_percent = rule_position_percent.min(max_single_position_percent);
        let position_size_dollars = (available_cash * effective_position_percent / 100.0).min(
            rule.max_position_value.unwrap_or(f64::MAX)
        );

        if position_size_dollars < indicators.price {
            warnings.push("Position size too small to buy even 1 share".to_string());
        }

        // Calculate stop loss
        let stop_loss_percent = rule.stop_loss_percent
            .unwrap_or(self.risk_params.default_stop_loss_percent);
        let max_loss_dollars = rule.stop_loss_dollars
            .unwrap_or(position_size_dollars * stop_loss_percent / 100.0);

        // Check against maximum loss limits
        if let Some(max_trade_loss) = self.risk_params.max_loss_per_trade_dollars {
            if max_loss_dollars > max_trade_loss {
                warnings.push(format!(
                    "Calculated max loss ${:.2} exceeds limit ${:.2}", 
                    max_loss_dollars, max_trade_loss
                ));
            }
        }

        // Calculate portfolio exposure after this trade
        let new_exposure = portfolio.total_exposure + position_size_dollars;
        let exposure_percent_after = (new_exposure / portfolio.total_value) * 100.0;
        
        if exposure_percent_after > self.risk_params.max_portfolio_exposure_percent {
            warnings.push(format!(
                "Portfolio exposure would be {:.1}%, exceeds limit {:.1}%",
                exposure_percent_after, self.risk_params.max_portfolio_exposure_percent
            ));
        }

        // Volume confirmation check
        if self.risk_params.require_volume_confirmation {
            if let Some(volume_ratio) = indicators.volume_ratio {
                if volume_ratio < self.risk_params.min_volume_ratio {
                    warnings.push(format!(
                        "Volume ratio {:.2} below required {:.2}",
                        volume_ratio, self.risk_params.min_volume_ratio
                    ));
                }
            } else {
                warnings.push("Volume ratio not available for confirmation".to_string());
            }
        }

        // Assess risk level
        let risk_level = self.calculate_risk_level(
            exposure_percent_after,
            max_loss_dollars,
            &warnings,
        );

        Ok(RiskAssessment {
            position_size_dollars,
            max_loss_dollars,
            portfolio_exposure_after: exposure_percent_after,
            risk_reward_ratio: self.calculate_risk_reward_ratio(rule, indicators.price),
            risk_level,
            warnings,
        })
    }

    /// Assess risk for a sell order
    fn assess_sell_risk(
        &self,
        portfolio: &PortfolioState,
        _rule: &EnhancedTradingRule,
        _indicators: &TechnicalIndicators,
    ) -> Result<RiskAssessment> {
        let warnings = Vec::new();
        
        // For sell orders, risk is generally lower as we're reducing exposure
        let exposure_after = portfolio.get_exposure_percentage(); // Will be lower after sell
        
        Ok(RiskAssessment {
            position_size_dollars: 0.0, // Selling reduces position
            max_loss_dollars: 0.0,      // Selling typically reduces risk
            portfolio_exposure_after: exposure_after,
            risk_reward_ratio: None,
            risk_level: RiskLevel::Low,
            warnings,
        })
    }

    /// Assess risk for hold decision
    fn assess_hold_risk(&self, portfolio: &PortfolioState) -> Result<RiskAssessment> {
        Ok(RiskAssessment {
            position_size_dollars: 0.0,
            max_loss_dollars: 0.0,
            portfolio_exposure_after: portfolio.get_exposure_percentage(),
            risk_reward_ratio: None,
            risk_level: RiskLevel::Low,
            warnings: vec!["Holding position - no new risk".to_string()],
        })
    }

    /// Calculate risk level based on various factors
    fn calculate_risk_level(
        &self,
        exposure_percent: f64,
        max_loss_dollars: f64,
        warnings: &[String],
    ) -> RiskLevel {
        let high_exposure = exposure_percent > 80.0;
        let has_warnings = !warnings.is_empty();
        let large_loss_potential = max_loss_dollars > 1000.0; // Configurable threshold

        match (high_exposure, has_warnings, large_loss_potential) {
            (true, true, true) => RiskLevel::Critical,
            (true, _, _) | (_, true, true) => RiskLevel::High,
            (_, true, _) | (_, _, true) => RiskLevel::Medium,
            _ => RiskLevel::Low,
        }
    }

    /// Calculate risk-reward ratio if take profit is set
    fn calculate_risk_reward_ratio(
        &self, 
        rule: &EnhancedTradingRule, 
        _current_price: f64
    ) -> Option<f64> {
        if let (Some(take_profit_percent), Some(stop_loss_percent)) = 
            (rule.take_profit_percent, rule.stop_loss_percent) {
            Some(take_profit_percent / stop_loss_percent)
        } else {
            None
        }
    }

    /// Calculate optimal position size based on Kelly Criterion (simplified)
    pub fn calculate_kelly_position_size(
        &self,
        win_rate: f64,
        average_win: f64,
        average_loss: f64,
        available_capital: f64,
    ) -> f64 {
        if average_loss <= 0.0 {
            return 0.0;
        }
        
        let win_loss_ratio = average_win / average_loss;
        let kelly_fraction = (win_rate * win_loss_ratio - (1.0 - win_rate)) / win_loss_ratio;
        
        // Cap Kelly fraction for safety (never risk more than 25% of capital)
        let safe_kelly = kelly_fraction.max(0.0).min(0.25);
        
        available_capital * safe_kelly
    }

    /// Update risk parameters
    pub fn update_risk_parameters(&mut self, new_params: RiskParameters) {
        info!("Updating risk parameters");
        self.risk_params = new_params;
    }

    /// Get current risk parameters
    pub fn get_risk_parameters(&self) -> &RiskParameters {
        &self.risk_params
    }

    /// Check if portfolio is within risk limits
    pub fn validate_portfolio_risk(&self, portfolio: &PortfolioState) -> Result<Vec<String>> {
        let mut issues = Vec::new();

        // Check position count
        if portfolio.current_position_count > self.risk_params.max_positions {
            issues.push(format!(
                "Position count {} exceeds limit {}", 
                portfolio.current_position_count, 
                self.risk_params.max_positions
            ));
        }

        // Check portfolio exposure
        let exposure_percent = portfolio.get_exposure_percentage();
        if exposure_percent > self.risk_params.max_portfolio_exposure_percent {
            issues.push(format!(
                "Portfolio exposure {:.1}% exceeds limit {:.1}%",
                exposure_percent, 
                self.risk_params.max_portfolio_exposure_percent
            ));
        }

        // Check available cash
        if portfolio.available_cash < 0.0 {
            issues.push("Negative available cash detected".to_string());
        }

        Ok(issues)
    }

    /// Calculate position size for a given trade
    pub fn calculate_position_size(
        &self,
        portfolio: &PortfolioState,
        rule: &EnhancedTradingRule,
        stock_price: f64,
    ) -> Result<u64> {
        let available_cash = portfolio.available_cash;
        let position_percent = rule.position_size_percent.min(
            self.risk_params.max_single_position_percent
        );
        
        let position_value = available_cash * position_percent / 100.0;
        let max_position_value = rule.max_position_value.unwrap_or(f64::MAX);
        let final_position_value = position_value.min(max_position_value);
        
        if stock_price <= 0.0 {
            return Err(anyhow!("Invalid stock price: {}", stock_price));
        }
        
        let shares = (final_position_value / stock_price).floor() as u64;
        
        if shares == 0 {
            warn!("Calculated 0 shares for ${:.2} stock with ${:.2} position value", 
                stock_price, final_position_value);
        }
        
        Ok(shares)
    }

    /// Calculate stop loss price
    pub fn calculate_stop_loss_price(
        &self,
        rule: &EnhancedTradingRule,
        entry_price: f64,
    ) -> Result<f64> {
        let stop_loss_percent = rule.stop_loss_percent
            .unwrap_or(self.risk_params.default_stop_loss_percent);
            
        let stop_loss_price = entry_price * (1.0 - stop_loss_percent / 100.0);
        
        if stop_loss_price <= 0.0 {
            return Err(anyhow!("Invalid stop loss price calculated: {}", stop_loss_price));
        }
        
        Ok(stop_loss_price)
    }

    /// Get risk management statistics
    pub fn get_risk_stats(&self, portfolio: &PortfolioState) -> RiskStats {
        RiskStats {
            current_exposure_percent: portfolio.get_exposure_percentage(),
            available_cash: portfolio.available_cash,
            position_count: portfolio.current_position_count,
            max_positions: self.risk_params.max_positions,
            risk_utilization: portfolio.current_position_count as f64 / self.risk_params.max_positions as f64 * 100.0,
        }
    }
}

#[derive(Debug)]
pub struct RiskStats {
    pub current_exposure_percent: f64,
    pub available_cash: f64,
    pub position_count: u32,
    pub max_positions: u32,
    pub risk_utilization: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    #[test]
    fn test_position_size_calculation() {
        let risk_manager = RiskManager::new(RiskParameters::default());
        let portfolio = PortfolioState::new(10000.0, 5);
        let rule = EnhancedTradingRule::new(
            "test".to_string(), 
            "Test Rule".to_string(), 
            "Test".to_string()
        );
        
        let shares = risk_manager.calculate_position_size(&portfolio, &rule, 50.0).unwrap();
        assert!(shares > 0);
        
        // Should be around 10% of $10,000 = $1,000 / $50 = 20 shares
        assert!(shares <= 20);
    }

    #[test]
    fn test_stop_loss_calculation() {
        let risk_manager = RiskManager::new(RiskParameters::default());
        let mut rule = EnhancedTradingRule::new(
            "test".to_string(), 
            "Test Rule".to_string(), 
            "Test".to_string()
        );
        rule.stop_loss_percent = Some(10.0);
        
        let stop_price = risk_manager.calculate_stop_loss_price(&rule, 100.0).unwrap();
        assert_eq!(stop_price, 90.0); // 10% below entry price
    }
}