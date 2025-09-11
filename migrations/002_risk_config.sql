-- Risk Configuration and Symbol Management
-- Migration to support dynamic risk parameters and symbol watchlists

-- Risk parameters configuration table
CREATE TABLE IF NOT EXISTS risk_config (
    id INTEGER PRIMARY KEY DEFAULT 1,
    max_positions INTEGER NOT NULL DEFAULT 5,
    max_portfolio_exposure_percent REAL NOT NULL DEFAULT 95.0,
    max_single_position_percent REAL NOT NULL DEFAULT 20.0,
    default_stop_loss_percent REAL NOT NULL DEFAULT 10.0,
    max_loss_per_trade_dollars REAL,
    max_daily_loss_dollars REAL,
    require_volume_confirmation BOOLEAN NOT NULL DEFAULT 1,
    min_volume_ratio REAL NOT NULL DEFAULT 1.5,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Insert default risk configuration
INSERT OR IGNORE INTO risk_config (
    id,
    max_positions,
    max_portfolio_exposure_percent,
    max_single_position_percent,
    default_stop_loss_percent,
    max_loss_per_trade_dollars,
    max_daily_loss_dollars,
    require_volume_confirmation,
    min_volume_ratio
) VALUES (
    1,
    5,
    95.0,
    20.0,
    10.0,
    NULL,
    NULL,
    1,
    1.5
);

-- Symbol watchlist table for dynamic symbol management
CREATE TABLE IF NOT EXISTS watchlist_symbols (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL UNIQUE,
    description TEXT,
    active BOOLEAN NOT NULL DEFAULT 1,
    added_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Insert LightSpeed certification test symbols as defaults
INSERT OR IGNORE INTO watchlist_symbols (symbol, description, active) VALUES
    ('GOOGL', 'Google - Immediate fill test symbol', 1),
    ('AMZN', 'Amazon - Partial fill test symbol', 1),
    ('TSLA', 'Tesla - No fill test symbol', 1),
    ('MSFT', 'Microsoft - Rejection test symbol', 1),
    ('CHWY', 'Chewy - Multiple partial fills test symbol', 1),
    ('F', 'Ford - Multiple partial fills test symbol', 1),
    ('GE', 'General Electric - Multiple partial fills test symbol', 1),
    ('ORCL', 'Oracle - Cancel test symbol', 1);

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_watchlist_symbols_active ON watchlist_symbols(active);
CREATE INDEX IF NOT EXISTS idx_watchlist_symbols_symbol ON watchlist_symbols(symbol);

-- Trigger to update updated_at timestamp for risk_config
CREATE TRIGGER IF NOT EXISTS risk_config_updated_at
    AFTER UPDATE ON risk_config
    FOR EACH ROW
BEGIN
    UPDATE risk_config SET updated_at = strftime('%s', 'now') WHERE id = NEW.id;
END;

-- Trigger to update updated_at timestamp for watchlist_symbols
CREATE TRIGGER IF NOT EXISTS watchlist_symbols_updated_at
    AFTER UPDATE ON watchlist_symbols
    FOR EACH ROW
BEGIN
    UPDATE watchlist_symbols SET updated_at = strftime('%s', 'now') WHERE id = NEW.id;
END;