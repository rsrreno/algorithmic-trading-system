-- Paper Trading Infrastructure
-- Tables for paper trading sessions, simulated trades, and rule configurations

-- Paper trading sessions for tracking different paper trading runs
CREATE TABLE IF NOT EXISTS paper_sessions (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    initial_cash REAL NOT NULL,
    current_cash REAL NOT NULL,
    total_pnl REAL NOT NULL DEFAULT 0.0,
    realized_pnl REAL NOT NULL DEFAULT 0.0,
    unrealized_pnl REAL NOT NULL DEFAULT 0.0,
    open_positions INTEGER NOT NULL DEFAULT 0,
    total_trades INTEGER NOT NULL DEFAULT 0,
    winning_trades INTEGER NOT NULL DEFAULT 0,
    losing_trades INTEGER NOT NULL DEFAULT 0,
    commission_paid REAL NOT NULL DEFAULT 0.0,
    max_drawdown REAL NOT NULL DEFAULT 0.0,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    status TEXT NOT NULL DEFAULT 'ACTIVE'  -- ACTIVE, PAUSED, COMPLETED
);

-- Paper trades for simulated trade execution
CREATE TABLE IF NOT EXISTS paper_trades (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES paper_sessions(id) ON DELETE CASCADE,
    symbol TEXT NOT NULL,
    side TEXT NOT NULL,  -- BUY, SELL
    quantity INTEGER NOT NULL,
    entry_price REAL NOT NULL,
    exit_price REAL,
    current_price REAL,
    commission REAL NOT NULL DEFAULT 0.0,
    pnl REAL,
    pnl_percent REAL,
    opened_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    closed_at INTEGER,
    rule_id TEXT,
    rule_name TEXT,
    rule_trigger TEXT,  -- JSON of rule conditions that triggered
    stop_loss_price REAL,
    take_profit_price REAL,
    status TEXT NOT NULL DEFAULT 'OPEN',  -- OPEN, CLOSED, STOPPED
    notes TEXT
);

-- Trading rules configuration for paper trading
CREATE TABLE IF NOT EXISTS trading_rules_config (
    id TEXT PRIMARY KEY,
    session_id TEXT REFERENCES paper_sessions(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    rule_type TEXT NOT NULL,  -- MOMENTUM, BREAKOUT, MEAN_REVERSION, CUSTOM
    conditions JSON NOT NULL,  -- Rule conditions as JSON
    entry_criteria JSON NOT NULL,
    exit_criteria JSON NOT NULL,
    risk_management JSON NOT NULL,
    symbols JSON,  -- Target symbols (null = all watchlist symbols)
    active BOOLEAN NOT NULL DEFAULT 1,
    priority INTEGER NOT NULL DEFAULT 0,
    max_position_size REAL,
    stop_loss_percent REAL,
    take_profit_percent REAL,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    -- Performance tracking
    total_triggers INTEGER NOT NULL DEFAULT 0,
    successful_trades INTEGER NOT NULL DEFAULT 0,
    failed_trades INTEGER NOT NULL DEFAULT 0,
    total_pnl REAL NOT NULL DEFAULT 0.0,
    win_rate REAL NOT NULL DEFAULT 0.0
);

-- Paper trading portfolio snapshots for performance tracking
CREATE TABLE IF NOT EXISTS paper_portfolio_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES paper_sessions(id) ON DELETE CASCADE,
    timestamp INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    total_value REAL NOT NULL,
    cash_value REAL NOT NULL,
    positions_value REAL NOT NULL,
    total_pnl REAL NOT NULL,
    daily_pnl REAL NOT NULL,
    open_positions INTEGER NOT NULL,
    snapshot_type TEXT NOT NULL DEFAULT 'HOURLY'  -- HOURLY, DAILY, TRADE_EVENT
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_paper_trades_session_id ON paper_trades(session_id);
CREATE INDEX IF NOT EXISTS idx_paper_trades_symbol ON paper_trades(symbol);
CREATE INDEX IF NOT EXISTS idx_paper_trades_status ON paper_trades(status);
CREATE INDEX IF NOT EXISTS idx_paper_trades_opened_at ON paper_trades(opened_at);

CREATE INDEX IF NOT EXISTS idx_trading_rules_session_id ON trading_rules_config(session_id);
CREATE INDEX IF NOT EXISTS idx_trading_rules_active ON trading_rules_config(active);
CREATE INDEX IF NOT EXISTS idx_trading_rules_priority ON trading_rules_config(priority);

CREATE INDEX IF NOT EXISTS idx_paper_sessions_status ON paper_sessions(status);
CREATE INDEX IF NOT EXISTS idx_paper_sessions_created_at ON paper_sessions(created_at);

CREATE INDEX IF NOT EXISTS idx_portfolio_snapshots_session_id ON paper_portfolio_snapshots(session_id);
CREATE INDEX IF NOT EXISTS idx_portfolio_snapshots_timestamp ON paper_portfolio_snapshots(timestamp);

-- Triggers for automatic timestamp updates
CREATE TRIGGER IF NOT EXISTS paper_sessions_updated_at
    AFTER UPDATE ON paper_sessions
    FOR EACH ROW
BEGIN
    UPDATE paper_sessions SET updated_at = strftime('%s', 'now') WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS trading_rules_config_updated_at
    AFTER UPDATE ON trading_rules_config
    FOR EACH ROW
BEGIN
    UPDATE trading_rules_config SET updated_at = strftime('%s', 'now') WHERE id = NEW.id;
END;

-- Insert a default paper trading session for immediate testing
INSERT OR IGNORE INTO paper_sessions (
    id,
    name,
    description,
    initial_cash,
    current_cash
) VALUES (
    'default-session',
    'Default Paper Trading Session',
    'Default session created for immediate paper trading testing',
    100000.0,
    100000.0
);