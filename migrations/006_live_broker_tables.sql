-- Migration: 006_live_broker_tables.sql
-- Live Broker Support Tables
-- Branch: 14.9.25
-- Created: 2025-09-14

-- Live positions table for non-paper brokers (Lightspeed, etc.)
CREATE TABLE IF NOT EXISTS live_positions (
    id TEXT PRIMARY KEY,
    broker_name TEXT NOT NULL,           -- 'lightspeed', 'interactive_brokers', etc.
    symbol TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    avg_cost_basis REAL NOT NULL,        -- Cost basis from broker
    current_price REAL NOT NULL,         -- Latest market price
    market_value REAL NOT NULL,          -- Current market value
    unrealized_pnl REAL NOT NULL DEFAULT 0.0,  -- Unrealized P&L
    realized_pnl REAL NOT NULL DEFAULT 0.0,    -- Cumulative realized P&L
    opened_at INTEGER NOT NULL,
    last_updated INTEGER NOT NULL,       -- Last sync with broker
    status TEXT NOT NULL DEFAULT 'OPEN', -- 'OPEN' or 'CLOSED'
    account_id TEXT NOT NULL,            -- Broker account identifier
    position_id TEXT,                    -- Broker's position ID if available

    -- Metadata
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Live trades table for executed trades from live brokers
CREATE TABLE IF NOT EXISTS live_trades (
    id TEXT PRIMARY KEY,
    broker_name TEXT NOT NULL,
    order_id TEXT,                       -- Broker's order ID
    client_order_id TEXT,                -- Our client order ID
    symbol TEXT NOT NULL,
    side TEXT NOT NULL,                  -- 'BUY', 'SELL', 'SELL_SHORT'
    quantity INTEGER NOT NULL,
    executed_price REAL NOT NULL,
    execution_time INTEGER NOT NULL,
    commission REAL NOT NULL DEFAULT 0.0,
    fees REAL NOT NULL DEFAULT 0.0,
    net_amount REAL NOT NULL,            -- Total trade value including fees
    account_id TEXT NOT NULL,

    -- P&L calculation fields
    realized_pnl REAL,                   -- Realized P&L if closing position
    position_id TEXT,                    -- Link to live_positions

    -- Metadata
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    notes TEXT
);

-- Live P&L records for realized gains/losses from live brokers
CREATE TABLE IF NOT EXISTS live_pnl (
    id TEXT PRIMARY KEY,
    broker_name TEXT NOT NULL,
    symbol TEXT NOT NULL,
    trade_id TEXT NOT NULL,              -- References live_trades.id
    pnl_amount REAL NOT NULL,            -- Realized P&L amount
    pnl_type TEXT NOT NULL,              -- 'REALIZED', 'DIVIDEND', 'FEE_ADJUSTMENT'
    calculation_method TEXT,             -- 'FIFO', 'LIFO', 'SPECIFIC_LOT', 'BROKER_CALCULATED'
    tax_lot_info TEXT,                   -- JSON with tax lot details if available
    recorded_at INTEGER NOT NULL,
    account_id TEXT NOT NULL,

    -- Metadata
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Broker connection status tracking
CREATE TABLE IF NOT EXISTS broker_status (
    broker_name TEXT PRIMARY KEY,
    is_connected BOOLEAN NOT NULL DEFAULT 0,
    last_connected INTEGER,
    last_disconnected INTEGER,
    connection_attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    account_id TEXT,

    -- Configuration snapshot
    config_hash TEXT,                    -- Hash of configuration for change detection

    -- Metadata
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_live_positions_broker ON live_positions(broker_name);
CREATE INDEX IF NOT EXISTS idx_live_positions_symbol ON live_positions(symbol);
CREATE INDEX IF NOT EXISTS idx_live_positions_status ON live_positions(status);
CREATE INDEX IF NOT EXISTS idx_live_positions_account ON live_positions(account_id);

CREATE INDEX IF NOT EXISTS idx_live_trades_broker ON live_trades(broker_name);
CREATE INDEX IF NOT EXISTS idx_live_trades_symbol ON live_trades(symbol);
CREATE INDEX IF NOT EXISTS idx_live_trades_execution_time ON live_trades(execution_time);
CREATE INDEX IF NOT EXISTS idx_live_trades_order_id ON live_trades(order_id);
CREATE INDEX IF NOT EXISTS idx_live_trades_account ON live_trades(account_id);

CREATE INDEX IF NOT EXISTS idx_live_pnl_broker ON live_pnl(broker_name);
CREATE INDEX IF NOT EXISTS idx_live_pnl_symbol ON live_pnl(symbol);
CREATE INDEX IF NOT EXISTS idx_live_pnl_trade_id ON live_pnl(trade_id);
CREATE INDEX IF NOT EXISTS idx_live_pnl_recorded_at ON live_pnl(recorded_at);

CREATE INDEX IF NOT EXISTS idx_broker_status_connected ON broker_status(is_connected);

-- Views for easier querying

-- View: Current live positions with unrealized P&L
CREATE VIEW IF NOT EXISTS v_live_positions_current AS
SELECT
    lp.*,
    (lp.market_value - (lp.quantity * lp.avg_cost_basis)) as calculated_unrealized_pnl,
    (lp.current_price - lp.avg_cost_basis) as per_share_pnl,
    ((lp.current_price - lp.avg_cost_basis) / lp.avg_cost_basis * 100) as pnl_percentage
FROM live_positions lp
WHERE lp.status = 'OPEN' AND lp.quantity != 0;

-- View: Aggregated P&L by broker and symbol
CREATE VIEW IF NOT EXISTS v_live_pnl_summary AS
SELECT
    broker_name,
    symbol,
    SUM(pnl_amount) as total_realized_pnl,
    COUNT(*) as total_trades,
    MIN(recorded_at) as first_trade_date,
    MAX(recorded_at) as last_trade_date
FROM live_pnl
GROUP BY broker_name, symbol;

-- View: Daily P&L summary
CREATE VIEW IF NOT EXISTS v_live_pnl_daily AS
SELECT
    broker_name,
    DATE(recorded_at, 'unixepoch') as trade_date,
    SUM(pnl_amount) as daily_pnl,
    COUNT(*) as daily_trades
FROM live_pnl
GROUP BY broker_name, DATE(recorded_at, 'unixepoch')
ORDER BY trade_date DESC;

-- Triggers for maintaining updated_at timestamps
CREATE TRIGGER IF NOT EXISTS live_positions_updated_at
    AFTER UPDATE ON live_positions
BEGIN
    UPDATE live_positions SET updated_at = strftime('%s', 'now') WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS broker_status_updated_at
    AFTER UPDATE ON broker_status
BEGIN
    UPDATE broker_status SET updated_at = strftime('%s', 'now') WHERE broker_name = NEW.broker_name;
END;