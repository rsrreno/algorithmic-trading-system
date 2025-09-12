-- Migration: 005_positions_table.sql
-- Enhanced Position Tracking System
-- Branch: 9.11.25.1
-- Created: 2025-09-12

-- Create positions table for proper cost basis tracking
CREATE TABLE IF NOT EXISTS positions (
    id TEXT PRIMARY KEY,
    symbol TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    avg_cost_basis REAL NOT NULL,    -- Weighted average cost per share
    total_cost REAL NOT NULL,        -- Total dollars invested in position
    current_price REAL NOT NULL,     -- Latest market price
    realized_pnl REAL NOT NULL DEFAULT 0.0,  -- P&L from closed trades
    opened_at INTEGER NOT NULL,
    closed_at INTEGER,
    status TEXT NOT NULL DEFAULT 'OPEN',  -- 'OPEN' or 'CLOSED'
    session_id TEXT NOT NULL
);

-- Add index for faster queries
CREATE INDEX IF NOT EXISTS idx_positions_symbol ON positions(symbol);
CREATE INDEX IF NOT EXISTS idx_positions_status ON positions(status);
CREATE INDEX IF NOT EXISTS idx_positions_session ON positions(session_id);

-- Add position_id reference to paper_trades table (only if it doesn't exist)
-- SQLite doesn't have proper IF NOT EXISTS for ALTER TABLE, so we need to handle this differently
-- We'll check in the application code instead of trying to alter here

-- Add index for position_id lookups (will only create if table has the column)
-- CREATE INDEX IF NOT EXISTS idx_paper_trades_position_id ON paper_trades(position_id);