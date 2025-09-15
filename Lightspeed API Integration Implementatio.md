Lightspeed API Integration Implementation Plan

Based on my analysis of the codebase, here's a comprehensive plan to enable the Lightspeed API for trading while maintaining paper broker
functionality:

Current State Analysis

✅ Already Implemented:
- Lightspeed broker connection and WebSocket handling
- Paper broker with complex position tracking
- Configuration system supports both brokers
- Trading mode switching via TRADING_MODE env var
- Database schema for paper trading

❌ Missing Components:
- Non-paper broker database tables
- Broker-specific position tracking
- Dashboard UI for broker positions
- Order placement widget
- Live broker P&L recording

Implementation Plan

Phase 1: Database Schema for Live Brokers

Goal: Create separate tables for non-paper broker data

1. Create new migration: 006_live_broker_tables.sql
- live_positions table (Lightspeed positions)
- live_trades table (executed trades from live brokers)
- live_pnl table (realized P&L from live brokers)
- Indexes for performance
2. Update version scheme: Change file headers from old scheme to new DD.MM.YY format

Phase 2: Broker Abstraction Enhancement

Goal: Make broker switching seamless and extensible

1. Enhance BrokerModule:
- Add trait for common broker operations
- Implement position fetching from live brokers
- Add P&L recording for non-paper brokers
2. Configuration Updates:
- Ensure easy switching between PAPER/LIVE modes
- Validate broker availability on startup

Phase 3: Live Broker Position Management

Goal: Handle positions from Lightspeed API

1. Position Synchronization:
- Fetch positions from Lightspeed on startup
- Store in live_positions table
- Update positions on trade executions
- Handle position updates from broker
2. P&L Tracking:
- Record realized P&L from broker API
- Store in live_pnl table
- Calculate unrealized P&L using current market prices

Phase 4: Dashboard UI for Broker Positions

Goal: Create broker-specific position pages

1. New Route: /positions/<broker_name> (e.g., /positions/lightspeed)
2. Position Widget: Shows ticker, quantity, basis, realized/unrealized P&L
3. Order Placement Widget: Place orders and get quotes
4. Responsive Design: Leave room for future enhancements

Phase 5: Testing and Validation

Goal: Ensure container starts and features work

1. Docker Testing:
- Container builds and starts successfully
- All brokers initialize correctly
- Mode switching works via env vars
2. Feature Validation:
- Position fetching from Lightspeed
- Order placement through live broker
- P&L recording and display
- Dashboard navigation

Questions for Clarification

1. Database Location: Should live broker tables be in the same SQLite database or separate?
2. P&L Source: For unrealized P&L calculation, should we use:
- Lightspeed position data + Polygon market prices?
- Pure Lightspeed data?
- Hybrid approach?
3. Error Handling: How should we handle broker disconnections?
- Fall back to paper mode?
- Show error state?
- Retry connection?
4. Historical Data: Do you want historical trade data imported from Lightspeed, or start fresh?
5. UI Layout: Should the broker position page be:
- Completely separate from paper trading dashboard?
- Integrated tabs on main dashboard?
- Separate navigation item?

This plan ensures we don't break paper trading while adding robust live broker support with easy mode switching via environment variables.

#1- use the same database but create new tables, views, indexes etc. as needed.  #2 - I'm not sure yet, we will have to do some testing
during market hours.  #3 Never switch modes unless a user specifies it.  if a broker is disconnected, show an error and try reconnection.  #4
fresh start.  #5 sperate page off the exisisting page.
⎿  Good to know.