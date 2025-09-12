-- Market holidays table for tracking US stock market closures and early closes
-- Used by MarketSchedule module to determine data source strategy

CREATE TABLE IF NOT EXISTS market_holidays (
    date TEXT PRIMARY KEY,           -- YYYY-MM-DD format
    name TEXT NOT NULL,              -- Holiday name (e.g., "Independence Day")
    status TEXT NOT NULL,            -- "closed" or "early_close" 
    open_time TEXT NULL,             -- Opening time for early close days (HH:MM format)
    close_time TEXT NULL,            -- Closing time for early close days (HH:MM format)
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_market_holidays_date ON market_holidays(date);
CREATE INDEX IF NOT EXISTS idx_market_holidays_status ON market_holidays(status);