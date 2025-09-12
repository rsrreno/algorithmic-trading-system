// src/market/mod.rs
// Branch: 9.11.25.1

use chrono::{DateTime, Utc, NaiveTime, Datelike, Weekday, Duration};
use chrono_tz::US::Eastern;
use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use std::collections::HashSet;
use tracing::{info, warn, debug};

/// Represents US stock market trading schedule and data source selection logic
/// 
/// Market Definition:
/// - "Today" = Full calendar day from pre-market open to after-hours close
/// - "Open" = Any time from 4:00 AM ET pre-market to 8:00 PM ET after-hours  
/// - "Closed" = Any time outside the open range
/// 
/// Data Source Logic:
/// - Market Open: Use real-time/current data from Polygon
/// - Market Closed: Use previous trading day's data from Polygon daily summary
/// 
/// # Examples
/// 
/// ```
/// // 8:30 PM ET on 9/11/25 (market closed) -> use 9/11/25 daily summary
/// // 3:00 AM ET on 9/12/25 (market closed) -> use 9/11/25 daily summary  
/// // Weekends -> use most recent trading day's daily summary
/// ```
#[derive(Debug, Clone)]
pub struct MarketSchedule {
    /// Current UTC time for calculations
    pub current_time: DateTime<Utc>,
    
    /// Cached market holidays for performance
    market_holidays: HashSet<String>, // YYYY-MM-DD format
    
    /// Last holiday calendar update
    holidays_last_updated: DateTime<Utc>,
}

/// Market status indicating whether markets are currently open
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MarketStatus {
    /// Market is open (pre-market, regular, or after-hours)
    Open,
    /// Market is closed (nights, weekends, holidays)
    Closed,
}

/// Defines which trading session is currently active
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TradingSession {
    /// 4:00 AM - 9:30 AM ET
    PreMarket,
    /// 9:30 AM - 4:00 PM ET  
    RegularHours,
    /// 4:00 PM - 8:00 PM ET
    AfterHours,
    /// Market is closed
    Closed,
}

/// Data source selection based on market status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataSourceStrategy {
    /// Use real-time/current data (market is open)
    RealTime {
        session: TradingSession,
        market_date: String, // YYYY-MM-DD
    },
    /// Use previous day's daily summary (market is closed) 
    PreviousDay {
        summary_date: String, // YYYY-MM-DD
        reason: ClosedReason,
    },
}

/// Reason why market is closed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClosedReason {
    /// Outside trading hours (nights)
    OutsideHours,
    /// Weekend (Saturday/Sunday)
    Weekend,
    /// Market holiday
    Holiday(String), // Holiday name
}

/// Market holiday information from Polygon API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketHoliday {
    pub date: String,      // YYYY-MM-DD
    pub name: String,      // Holiday name
    pub status: String,    // "closed" or "early_close"
    pub open: Option<String>,  // Opening time if early close
    pub close: Option<String>, // Closing time if early close
}

impl MarketSchedule {
    /// Create new MarketSchedule instance
    pub fn new() -> Self {
        Self {
            current_time: Utc::now(),
            market_holidays: HashSet::new(),
            holidays_last_updated: DateTime::from_timestamp(0, 0).unwrap(),
        }
    }

    /// Update current time (for testing or specific time calculations)
    pub fn with_time(mut self, time: DateTime<Utc>) -> Self {
        self.current_time = time;
        self
    }

    /// Get current market status (open or closed)
    pub fn get_market_status(&self) -> MarketStatus {
        let et_time = self.current_time.with_timezone(&Eastern);
        
        // Check if it's a weekend
        if self.is_weekend(&et_time) {
            return MarketStatus::Closed;
        }
        
        // Check if it's a holiday
        let date_str = et_time.format("%Y-%m-%d").to_string();
        if self.market_holidays.contains(&date_str) {
            return MarketStatus::Closed;
        }
        
        // Check trading hours (4:00 AM - 8:00 PM ET)
        let market_open = NaiveTime::from_hms_opt(4, 0, 0).unwrap();
        let market_close = NaiveTime::from_hms_opt(20, 0, 0).unwrap();
        let current_time = et_time.time();
        
        if current_time >= market_open && current_time <= market_close {
            MarketStatus::Open
        } else {
            MarketStatus::Closed
        }
    }

    /// Get current trading session
    pub fn get_trading_session(&self) -> TradingSession {
        if self.get_market_status() == MarketStatus::Closed {
            return TradingSession::Closed;
        }
        
        let et_time = self.current_time.with_timezone(&Eastern);
        let current_time = et_time.time();
        
        // Pre-market: 4:00 AM - 9:30 AM ET
        let pre_market_start = NaiveTime::from_hms_opt(4, 0, 0).unwrap();
        let regular_start = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
        
        // Regular hours: 9:30 AM - 4:00 PM ET
        let regular_end = NaiveTime::from_hms_opt(16, 0, 0).unwrap();
        
        // After hours: 4:00 PM - 8:00 PM ET
        let after_hours_end = NaiveTime::from_hms_opt(20, 0, 0).unwrap();
        
        if current_time >= pre_market_start && current_time < regular_start {
            TradingSession::PreMarket
        } else if current_time >= regular_start && current_time < regular_end {
            TradingSession::RegularHours
        } else if current_time >= regular_end && current_time <= after_hours_end {
            TradingSession::AfterHours
        } else {
            TradingSession::Closed
        }
    }

    /// Get data source strategy based on current market status
    pub fn get_data_source_strategy(&self) -> DataSourceStrategy {
        let status = self.get_market_status();
        let et_time = self.current_time.with_timezone(&Eastern);
        
        match status {
            MarketStatus::Open => {
                let session = self.get_trading_session();
                let market_date = et_time.format("%Y-%m-%d").to_string();
                DataSourceStrategy::RealTime { session, market_date }
            },
            MarketStatus::Closed => {
                let (summary_date, reason) = self.get_previous_trading_day_info(&et_time);
                DataSourceStrategy::PreviousDay { summary_date, reason }
            }
        }
    }

    /// Get the most recent trading day and reason for closure
    fn get_previous_trading_day_info(&self, et_time: &DateTime<chrono_tz::Tz>) -> (String, ClosedReason) {
        let date_str = et_time.format("%Y-%m-%d").to_string();
        
        // Determine closure reason
        let reason = if self.is_weekend(et_time) {
            ClosedReason::Weekend
        } else if self.market_holidays.contains(&date_str) {
            ClosedReason::Holiday("Market Holiday".to_string())
        } else {
            ClosedReason::OutsideHours
        };
        
        // Find most recent trading day
        let mut check_date = et_time.clone();
        
        // If currently outside trading hours but same day, use current date
        if matches!(reason, ClosedReason::OutsideHours) && 
           et_time.time() > NaiveTime::from_hms_opt(20, 0, 0).unwrap() {
            return (date_str, reason);
        }
        
        // For early morning hours (before 4 AM), weekends, or holidays - look backwards
        loop {
            check_date = check_date - Duration::days(1);
            let check_date_str = check_date.format("%Y-%m-%d").to_string();
            
            // Skip weekends and holidays
            if !self.is_weekend(&check_date) && !self.market_holidays.contains(&check_date_str) {
                return (check_date_str, reason);
            }
            
            // Safety check - don't go back more than 10 days
            if (et_time.clone() - check_date).num_days() > 10 {
                warn!("Could not find recent trading day within 10 days");
                return (et_time.format("%Y-%m-%d").to_string(), reason);
            }
        }
    }

    /// Check if given date is a weekend
    fn is_weekend(&self, dt: &DateTime<chrono_tz::Tz>) -> bool {
        matches!(dt.weekday(), Weekday::Sat | Weekday::Sun)
    }

    /// Load market holidays from database cache
    pub async fn load_holidays_from_cache(&mut self, db: &crate::database::Database) -> Result<()> {
        let rows = sqlx::query_as::<_, (String, String, String)>(
            "SELECT date, name, status FROM market_holidays ORDER BY date"
        )
        .fetch_all(db)
        .await
        .context("Failed to load market holidays from database")?;

        self.market_holidays.clear();
        for (date, _name, status) in rows {
            if status == "closed" {
                self.market_holidays.insert(date);
            }
        }

        self.holidays_last_updated = Utc::now();
        info!("Loaded {} market holidays from database cache", self.market_holidays.len());
        Ok(())
    }

    /// Check if holiday cache needs refresh (daily)
    pub fn should_refresh_holidays(&self) -> bool {
        let hours_since_update = (Utc::now() - self.holidays_last_updated).num_hours();
        hours_since_update > 24 || self.market_holidays.is_empty()
    }

    /// Fetch latest market holidays from Polygon API and cache in database
    pub async fn refresh_holidays_from_api(
        &mut self, 
        db: &crate::database::Database,
        polygon_api_key: &str
    ) -> Result<()> {
        use reqwest;

        let client = reqwest::Client::new();
        let current_year = self.current_time.year();
        
        // Fetch this year and next year to ensure we have upcoming holidays
        for year in [current_year, current_year + 1] {
            let url = format!(
                "https://api.polygon.io/v1/marketstatus/upcoming?apikey={}",
                polygon_api_key
            );
            
            debug!("Fetching market holidays from Polygon API for year {}", year);
            
            let response = client
                .get(&url)
                .send()
                .await
                .context("Failed to fetch market holidays from Polygon API")?;
                
            if !response.status().is_success() {
                warn!("Polygon API returned non-success status: {}", response.status());
                continue;
            }
            
            let holidays: Vec<MarketHoliday> = response
                .json()
                .await
                .context("Failed to parse market holidays JSON response")?;
                
            // Save to database and update cache
            for holiday in holidays {
                self.save_holiday_to_db(db, &holiday).await?;
                
                if holiday.status == "closed" {
                    self.market_holidays.insert(holiday.date.clone());
                }
            }
        }
        
        self.holidays_last_updated = Utc::now();
        info!("Updated market holidays cache with {} holidays", self.market_holidays.len());
        Ok(())
    }

    /// Save individual holiday to database
    async fn save_holiday_to_db(
        &self, 
        db: &crate::database::Database, 
        holiday: &MarketHoliday
    ) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO market_holidays 
             (date, name, status, open_time, close_time, updated_at)
             VALUES (?, ?, ?, ?, ?, strftime('%s', 'now'))"
        )
        .bind(&holiday.date)
        .bind(&holiday.name)
        .bind(&holiday.status)
        .bind(&holiday.open)
        .bind(&holiday.close)
        .execute(db)
        .await
        .context("Failed to save market holiday to database")?;
        
        Ok(())
    }

    /// Get human-readable description of current market status
    pub fn get_status_description(&self) -> String {
        let strategy = self.get_data_source_strategy();
        let et_time = self.current_time.with_timezone(&Eastern);
        
        match strategy {
            DataSourceStrategy::RealTime { session, market_date } => {
                let session_desc = match session {
                    TradingSession::PreMarket => "pre-market",
                    TradingSession::RegularHours => "regular hours", 
                    TradingSession::AfterHours => "after-hours",
                    TradingSession::Closed => "closed", // shouldn't happen
                };
                format!(
                    "Market OPEN - {} session on {} (ET: {})",
                    session_desc,
                    market_date,
                    et_time.format("%Y-%m-%d %H:%M:%S %Z")
                )
            },
            DataSourceStrategy::PreviousDay { summary_date, reason } => {
                let reason_desc = match reason {
                    ClosedReason::OutsideHours => "outside trading hours",
                    ClosedReason::Weekend => "weekend",
                    ClosedReason::Holiday(name) => &format!("holiday ({})", name),
                };
                format!(
                    "Market CLOSED - {} (ET: {}) | Using summary data from {}",
                    reason_desc,
                    et_time.format("%Y-%m-%d %H:%M:%S %Z"),
                    summary_date
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone};

    #[test]
    fn test_regular_hours_open() {
        // Tuesday 2:00 PM ET = 6:00 PM UTC (during regular hours)
        let test_time = Utc.with_ymd_and_hms(2025, 9, 9, 18, 0, 0).unwrap(); // Tuesday 
        let schedule = MarketSchedule::new().with_time(test_time);
        
        assert_eq!(schedule.get_market_status(), MarketStatus::Open);
        assert_eq!(schedule.get_trading_session(), TradingSession::RegularHours);
    }

    #[test]
    fn test_after_hours_open() {
        // Tuesday 6:00 PM ET = 10:00 PM UTC (after hours)
        let test_time = Utc.with_ymd_and_hms(2025, 9, 9, 22, 0, 0).unwrap();
        let schedule = MarketSchedule::new().with_time(test_time);
        
        assert_eq!(schedule.get_market_status(), MarketStatus::Open);
        assert_eq!(schedule.get_trading_session(), TradingSession::AfterHours);
    }

    #[test]  
    fn test_weekend_closed() {
        // Saturday 2:00 PM ET = 6:00 PM UTC (weekend)
        let test_time = Utc.with_ymd_and_hms(2025, 9, 6, 18, 0, 0).unwrap(); // Saturday
        let schedule = MarketSchedule::new().with_time(test_time);
        
        assert_eq!(schedule.get_market_status(), MarketStatus::Closed);
        assert_eq!(schedule.get_trading_session(), TradingSession::Closed);
    }

    #[test]
    fn test_night_closed() {
        // Tuesday 11:00 PM ET = 3:00 AM+1 UTC (outside hours)
        let test_time = Utc.with_ymd_and_hms(2025, 9, 10, 3, 0, 0).unwrap();
        let schedule = MarketSchedule::new().with_time(test_time);
        
        assert_eq!(schedule.get_market_status(), MarketStatus::Closed);
        assert_eq!(schedule.get_trading_session(), TradingSession::Closed);
    }

    #[test]
    fn test_data_source_strategy() {
        // Test real-time during market hours
        let market_open_time = Utc.with_ymd_and_hms(2025, 9, 9, 18, 0, 0).unwrap();
        let schedule = MarketSchedule::new().with_time(market_open_time);
        
        match schedule.get_data_source_strategy() {
            DataSourceStrategy::RealTime { session, market_date } => {
                assert_eq!(session, TradingSession::RegularHours);
                assert_eq!(market_date, "2025-09-09");
            },
            _ => panic!("Expected RealTime strategy during market hours"),
        }

        // Test previous day during closed hours
        let market_closed_time = Utc.with_ymd_and_hms(2025, 9, 10, 3, 0, 0).unwrap();
        let schedule = MarketSchedule::new().with_time(market_closed_time);
        
        match schedule.get_data_source_strategy() {
            DataSourceStrategy::PreviousDay { summary_date, reason } => {
                assert_eq!(summary_date, "2025-09-09"); // Previous trading day
                assert!(matches!(reason, ClosedReason::OutsideHours));
            },
            _ => panic!("Expected PreviousDay strategy during closed hours"),
        }
    }
}