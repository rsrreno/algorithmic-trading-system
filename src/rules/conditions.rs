// src/rules/conditions.rs
// Branch: 9.2.25.1

use std::time::Instant;
use anyhow::Result;
use tracing::{debug, warn};
use chrono::{Utc, Datelike};

use crate::types::{TechnicalIndicators, EnhancedTradingRule};

/// High-performance condition evaluator for trading rules
/// Designed for <1ms evaluation time to meet <5ms decision pipeline requirement
#[derive(Debug)]
pub struct ConditionEvaluator;

#[derive(Debug)]
pub struct EvaluationResult {
    pub passed: bool,
    pub confidence_score: f64, // 0.0 to 1.0
    pub conditions_met: Vec<String>,
    pub conditions_failed: Vec<String>,
    pub evaluation_time_micros: u64,
}

impl ConditionEvaluator {
    pub fn new() -> Self {
        Self
    }

    /// Evaluate all conditions for a trading rule against current market data
    /// Target: <1ms execution time for optimal performance
    pub fn evaluate_rule(
        &self,
        rule: &EnhancedTradingRule,
        indicators: &TechnicalIndicators,
    ) -> Result<EvaluationResult> {
        let start_time = Instant::now();
        
        let mut conditions_met = Vec::new();
        let mut conditions_failed = Vec::new();
        let mut total_conditions = 0;
        let mut confidence_score = 0.0;

        let conditions = &rule.entry_conditions;

        // Price Range Conditions
        if let Some(min_price) = conditions.min_price {
            total_conditions += 1;
            if indicators.price >= min_price {
                conditions_met.push(format!("Price ${:.2} >= min ${:.2}", indicators.price, min_price));
                confidence_score += 0.1;
            } else {
                conditions_failed.push(format!("Price ${:.2} < min ${:.2}", indicators.price, min_price));
            }
        }

        if let Some(max_price) = conditions.max_price {
            total_conditions += 1;
            if indicators.price <= max_price {
                conditions_met.push(format!("Price ${:.2} <= max ${:.2}", indicators.price, max_price));
                confidence_score += 0.1;
            } else {
                conditions_failed.push(format!("Price ${:.2} > max ${:.2}", indicators.price, max_price));
            }
        }

        // Volume Conditions
        if let Some(min_volume) = conditions.min_volume {
            total_conditions += 1;
            if indicators.volume >= min_volume {
                conditions_met.push(format!("Volume {} >= min {}", indicators.volume, min_volume));
                confidence_score += 0.15;
            } else {
                conditions_failed.push(format!("Volume {} < min {}", indicators.volume, min_volume));
            }
        }

        if let Some(min_volume_ratio) = conditions.volume_ratio_min {
            total_conditions += 1;
            if let Some(volume_ratio) = indicators.volume_ratio {
                if volume_ratio >= min_volume_ratio {
                    conditions_met.push(format!("Volume ratio {:.2} >= min {:.2}", volume_ratio, min_volume_ratio));
                    confidence_score += 0.2; // High importance for momentum trading
                } else {
                    conditions_failed.push(format!("Volume ratio {:.2} < min {:.2}", volume_ratio, min_volume_ratio));
                }
            } else {
                conditions_failed.push("Volume ratio not available".to_string());
            }
        }

        // RSI Conditions
        if let Some(rsi_min) = conditions.rsi_min {
            total_conditions += 1;
            if let Some(rsi) = indicators.rsi_14 {
                if rsi >= rsi_min {
                    conditions_met.push(format!("RSI {:.2} >= min {:.2}", rsi, rsi_min));
                    confidence_score += 0.15;
                } else {
                    conditions_failed.push(format!("RSI {:.2} < min {:.2}", rsi, rsi_min));
                }
            } else {
                conditions_failed.push("RSI not available".to_string());
            }
        }

        if let Some(rsi_max) = conditions.rsi_max {
            total_conditions += 1;
            if let Some(rsi) = indicators.rsi_14 {
                if rsi <= rsi_max {
                    conditions_met.push(format!("RSI {:.2} <= max {:.2}", rsi, rsi_max));
                    confidence_score += 0.15;
                } else {
                    conditions_failed.push(format!("RSI {:.2} > max {:.2}", rsi, rsi_max));
                }
            } else {
                conditions_failed.push("RSI not available".to_string());
            }
        }

        // MACD Conditions
        if let Some(macd_positive) = conditions.macd_positive {
            total_conditions += 1;
            if let Some(macd) = indicators.macd_value {
                let is_positive = macd > 0.0;
                if is_positive == macd_positive {
                    conditions_met.push(format!("MACD {:.4} positive: {}", macd, is_positive));
                    confidence_score += 0.2; // High importance for momentum
                } else {
                    conditions_failed.push(format!("MACD {:.4} positive: {} (expected {})", macd, is_positive, macd_positive));
                }
            } else {
                conditions_failed.push("MACD not available".to_string());
            }
        }

        if let Some(macd_above_signal) = conditions.macd_above_signal {
            total_conditions += 1;
            if let (Some(macd), Some(signal)) = (indicators.macd_value, indicators.macd_signal) {
                let is_above = macd > signal;
                if is_above == macd_above_signal {
                    conditions_met.push(format!("MACD {:.4} above signal {:.4}: {}", macd, signal, is_above));
                    confidence_score += 0.25; // Very high importance for momentum
                } else {
                    conditions_failed.push(format!("MACD {:.4} above signal {:.4}: {} (expected {})", 
                        macd, signal, is_above, macd_above_signal));
                }
            } else {
                conditions_failed.push("MACD or signal not available".to_string());
            }
        }

        // EMA Conditions
        if let Some(price_above_ema9) = conditions.price_above_ema9 {
            total_conditions += 1;
            let is_above = indicators.price_above_ema9;
            if is_above == price_above_ema9 {
                if let Some(ema9) = indicators.ema_9 {
                    conditions_met.push(format!("Price ${:.2} above EMA9 ${:.2}: {}", 
                        indicators.price, ema9, is_above));
                } else {
                    conditions_met.push(format!("Price above EMA9: {}", is_above));
                }
                confidence_score += 0.2;
            } else {
                if let Some(ema9) = indicators.ema_9 {
                    conditions_failed.push(format!("Price ${:.2} above EMA9 ${:.2}: {} (expected {})", 
                        indicators.price, ema9, is_above, price_above_ema9));
                } else {
                    conditions_failed.push(format!("Price above EMA9: {} (expected {})", is_above, price_above_ema9));
                }
            }
        }

        if let Some(ema9_increasing) = conditions.ema9_increasing {
            total_conditions += 1;
            if let Some(ema9_slope) = indicators.ema9_slope {
                let is_increasing = ema9_slope > 0.0;
                if is_increasing == ema9_increasing {
                    conditions_met.push(format!("EMA9 slope {:.4} increasing: {}", ema9_slope, is_increasing));
                    confidence_score += 0.15;
                } else {
                    conditions_failed.push(format!("EMA9 slope {:.4} increasing: {} (expected {})", 
                        ema9_slope, is_increasing, ema9_increasing));
                }
            } else {
                conditions_failed.push("EMA9 slope not available".to_string());
            }
        }

        // Market Hours Check (always evaluated if enabled)
        if conditions.market_hours_only {
            total_conditions += 1;
            if self.is_market_hours() {
                conditions_met.push("Market hours check passed".to_string());
                confidence_score += 0.05;
            } else {
                conditions_failed.push("Market is closed".to_string());
            }
        }

        // Calculate final results
        let all_passed = conditions_failed.is_empty() && !conditions_met.is_empty();
        let normalized_confidence = if total_conditions > 0 {
            confidence_score / total_conditions as f64
        } else {
            0.0
        };

        let evaluation_time = start_time.elapsed();
        let evaluation_time_micros = evaluation_time.as_micros() as u64;

        // Log performance warning if evaluation took too long
        if evaluation_time_micros > 1000 {
            warn!("Rule evaluation took {}μs for rule {} (target: <1000μs)", 
                evaluation_time_micros, rule.name);
        } else {
            debug!("Rule evaluation completed in {}μs for rule {}", 
                evaluation_time_micros, rule.name);
        }

        Ok(EvaluationResult {
            passed: all_passed,
            confidence_score: normalized_confidence.min(1.0),
            conditions_met,
            conditions_failed,
            evaluation_time_micros,
        })
    }

    /// Fast market hours check (US Eastern Time)
    /// This is a simplified implementation - in production, you'd want more accurate market calendar
    fn is_market_hours(&self) -> bool {
        use chrono::{Timelike, Weekday};
        
        let now_utc = Utc::now();
        
        // Convert to US Eastern Time (approximate)
        let et_offset = if self.is_dst() { -4 } else { -5 };
        let et_time = now_utc + chrono::Duration::hours(et_offset);
        
        // Check if weekday
        match et_time.weekday() {
            Weekday::Sat | Weekday::Sun => return false,
            _ => {}
        }
        
        // Check if between 9:30 AM and 4:00 PM ET
        let hour = et_time.hour();
        let minute = et_time.minute();
        
        let minutes_since_midnight = hour * 60 + minute;
        let market_open = 9 * 60 + 30;  // 9:30 AM
        let market_close = 16 * 60;     // 4:00 PM
        
        minutes_since_midnight >= market_open && minutes_since_midnight < market_close
    }

    /// Simple DST check (approximate)
    fn is_dst(&self) -> bool {
        let now = Utc::now();
        let month = now.month();
        
        // Rough DST period: March through October
        month >= 3 && month <= 10
    }

    /// Evaluate a single condition for testing purposes
    pub fn evaluate_condition(
        &self,
        _condition_name: &str,
        expected_value: Option<f64>,
        actual_value: Option<f64>,
    ) -> bool {
        match (expected_value, actual_value) {
            (Some(expected), Some(actual)) => actual >= expected,
            _ => false,
        }
    }

    /// Get evaluation performance statistics
    pub fn get_performance_stats(&self) -> PerformanceStats {
        // In a real implementation, you'd track these metrics
        PerformanceStats {
            average_evaluation_time_micros: 500, // Placeholder
            max_evaluation_time_micros: 1200,
            total_evaluations: 0,
            conditions_evaluated_per_second: 0.0,
        }
    }
}

#[derive(Debug, Default)]
pub struct PerformanceStats {
    pub average_evaluation_time_micros: u64,
    pub max_evaluation_time_micros: u64,
    pub total_evaluations: u64,
    pub conditions_evaluated_per_second: f64,
}

impl Default for ConditionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    #[test]
    fn test_basic_condition_evaluation() {
        let evaluator = ConditionEvaluator::new();
        
        // Test simple price condition
        assert!(evaluator.evaluate_condition("min_price", Some(10.0), Some(15.0)));
        assert!(!evaluator.evaluate_condition("min_price", Some(20.0), Some(15.0)));
    }

    #[test]
    fn test_market_hours() {
        let evaluator = ConditionEvaluator::new();
        // Note: This test will depend on current time
        let _is_market_hours = evaluator.is_market_hours();
        // Just ensure it doesn't panic
    }
}