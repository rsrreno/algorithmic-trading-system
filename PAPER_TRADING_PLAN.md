# Paper Trading Implementation Plan

**Project Version:** v9.2.25.1  
**Last Updated:** 2025-09-05  
**Related Files:** `CLAUDE.md`, `README.md`, `STATUS.md`

## Overview

Implementation plan for adding paper trading functionality to the algorithmic trading system. The system will use the existing rules engine and Polygon data feeds to generate trades but simulate execution internally rather than sending to the LightSpeed broker.

## Current System Analysis

### ✅ Ready Components
- **Rules Engine**: Fully implemented with technical indicators (RSI, MACD, EMA, SMA)
- **Market Data**: Complete Polygon.io integration with WebSocket streaming
- **Portfolio Management**: Live position tracking with LightSpeed integration  
- **Performance Architecture**: <5ms decision pipeline already in place
- **Database**: SQLite with basic schema and migrations

### ⚠️ Issues Identified

#### Environment File Discrepancies
- `.env.example` has more comprehensive WebSocket configuration documentation
- Current `.env` uses different default symbols (`NVDA,TSLA,MSFT,GOOGL,AMZN` vs recommended LightSpeed test symbols)
- Missing commission configuration in both files

#### Critical Hardcoded Values Found

1. **Hard-coded symbols** in `src/rules/engine.rs:486-497`:
   ```rust
   let default_symbols = vec![
       "GOOGL", "AMZN", "TSLA", "MSFT", "CHWY", "F", "GE", "ORCL"
   ];
   ```

2. **Hard-coded prices and values**:
   - Default price: `100.0` (engine.rs:313, 322)
   - Risk thresholds: `1000.0` dollars (risk.rs:187)
   - Test portfolio: `10000.0` initial cash (risk.rs:351)
   - Evaluation interval: `1000ms` (engine.rs:71)

3. **Hard-coded risk parameters** in `RiskParameters::default()`:
   ```rust
   max_positions: 5,
   max_portfolio_exposure_percent: 95.0,
   max_single_position_percent: 20.0,
   default_stop_loss_percent: 10.0,
   ```

## Implementation Requirements

### Core Requirements
- **Trading Mode**: Paper trading mode via `TRADING_MODE=PAPER` configuration
- **No Simulation**: No slippage, partial fills, or delays - instant fills at market price
- **Commission Tracking**: Configurable commission per share via environment
- **Web UI**: Rule configuration, risk settings, and P&L tracking interface
- **Database Tracking**: All paper trades logged to database for analysis

### User Interface Requirements
- Web UI for rule configuration including risk settings
- P&L tracking available on web UI
- Remove all hardcoded risk-related entries and make available via web UI

## Implementation Plan

### Phase 1A: Remove Hardcoded Risk Parameters (CRITICAL)

**Objective**: Replace all hardcoded risk values with database-driven configuration

**Tasks**:
1. **Create Risk Configuration Table**:
   ```sql
   CREATE TABLE risk_config (
       id INTEGER PRIMARY KEY DEFAULT 1,
       max_positions INTEGER NOT NULL DEFAULT 5,
       max_portfolio_exposure_percent REAL NOT NULL DEFAULT 95.0,
       max_single_position_percent REAL NOT NULL DEFAULT 20.0,
       default_stop_loss_percent REAL NOT NULL DEFAULT 10.0,
       max_loss_per_trade_dollars REAL,
       max_daily_loss_dollars REAL,
       require_volume_confirmation BOOLEAN NOT NULL DEFAULT 1,
       min_volume_ratio REAL NOT NULL DEFAULT 1.5,
       updated_at INTEGER NOT NULL
   );
   ```

2. **Modify RiskParameters Loading**:
   - Remove `RiskParameters::default()` hardcoded implementation
   - Load risk parameters from database on startup
   - Create `RiskManager::load_from_database()` method

3. **Add Risk Configuration API**:
   ```
   GET  /api/risk      # Get current risk configuration
   POST /api/risk      # Update risk parameters
   ```

**Files to Modify**:
- `migrations/002_risk_config.sql` (new)
- `src/types/mod.rs` (remove Default impl)
- `src/rules/risk.rs` (add database loading)
- `src/web/mod.rs` (add API endpoints)

### Phase 1B: Remove Hardcoded Symbols (CRITICAL)

**Objective**: Replace hardcoded symbol lists with dynamic watchlist management

**Tasks**:
1. **Create Symbol Watchlist Table**:
   ```sql
   CREATE TABLE watchlist_symbols (
       id INTEGER PRIMARY KEY AUTOINCREMENT,
       symbol TEXT NOT NULL UNIQUE,
       description TEXT,
       active BOOLEAN NOT NULL DEFAULT 1,
       added_at INTEGER NOT NULL
   );
   ```

2. **Modify Symbol Evaluation Logic**:
   - Replace `get_evaluation_symbols()` hardcoded list
   - Load active symbols from database
   - Add fallback to default LightSpeed test symbols if watchlist empty

3. **Add Symbol Management API**:
   ```
   GET    /api/symbols           # Get watchlist
   POST   /api/symbols           # Add symbol to watchlist  
   DELETE /api/symbols/{symbol}  # Remove symbol from watchlist
   ```

**Files to Modify**:
- `migrations/002_risk_config.sql` (extend)
- `src/rules/engine.rs` (modify get_evaluation_symbols)
- `src/web/mod.rs` (add symbol management endpoints)

### Phase 1C: Remove Other Hardcoded Values

**Objective**: Make all system parameters configurable

**Tasks**:
1. **Environment Configuration Updates**:
   ```env
   # Add to .env.example
   RULES_EVALUATION_INTERVAL_MS=1000
   DEFAULT_ENTRY_PRICE_FALLBACK=100.0
   RISK_THRESHOLD_DOLLARS=1000.0
   ```

2. **Configuration Loading**:
   - Move hardcoded values to config module
   - Load from environment variables with sensible defaults
   - Update all references throughout codebase

**Files to Modify**:
- `.env.example` (add new configuration options)
- `src/config/mod.rs` (add new config fields)
- `src/rules/engine.rs` (use config values)
- `src/rules/risk.rs` (use config values)

### Phase 2: Paper Trading Core Infrastructure

**Objective**: Implement paper trading mode with simulated execution

**Tasks**:
1. **Environment Configuration**:
   ```env
   # Paper Trading Configuration
   TRADING_MODE=PAPER  # PAPER, LIVE, SIMULATION
   PAPER_INITIAL_CASH=100000.0
   PAPER_COMMISSION_PER_SHARE=0.005
   PAPER_ENABLE_COMMISSION=true
   ```

2. **Database Schema for Paper Trading**:
   ```sql
   -- Paper trading sessions
   CREATE TABLE paper_sessions (
       id TEXT PRIMARY KEY,
       name TEXT NOT NULL,
       initial_cash REAL NOT NULL,
       current_cash REAL NOT NULL,
       total_pnl REAL NOT NULL DEFAULT 0.0,
       created_at INTEGER NOT NULL,
       updated_at INTEGER NOT NULL,
       status TEXT NOT NULL DEFAULT 'ACTIVE'
   );

   -- Paper trades (simulated executions)
   CREATE TABLE paper_trades (
       id TEXT PRIMARY KEY,
       session_id TEXT NOT NULL REFERENCES paper_sessions(id),
       symbol TEXT NOT NULL,
       side TEXT NOT NULL,  -- BUY, SELL
       quantity INTEGER NOT NULL,
       entry_price REAL NOT NULL,
       exit_price REAL,
       commission REAL NOT NULL DEFAULT 0.0,
       pnl REAL,
       opened_at INTEGER NOT NULL,
       closed_at INTEGER,
       rule_id TEXT,
       rule_name TEXT,
       status TEXT NOT NULL DEFAULT 'OPEN'  -- OPEN, CLOSED
   );

   -- Trading rules configuration
   CREATE TABLE trading_rules (
       id TEXT PRIMARY KEY,
       name TEXT NOT NULL UNIQUE,
       description TEXT,
       config_json TEXT NOT NULL,  -- JSON blob of EnhancedTradingRule
       active BOOLEAN NOT NULL DEFAULT 1,
       created_at INTEGER NOT NULL,
       updated_at INTEGER NOT NULL
   );
   ```

3. **Paper Broker Implementation** (`src/broker/paper.rs`):
   - Implement `PaperBroker` that simulates LightSpeed API
   - Instant fills at current market price (no delays/partials)
   - Commission calculation and application  
   - P&L tracking and position management
   - Integration with existing `BrokerModule` interface

4. **Trading Mode Switching**:
   - Modify broker module initialization to choose Paper vs Live broker
   - Conditional broker selection based on `TRADING_MODE` environment variable
   - Ensure rules engine works transparently with both brokers

**Files to Create/Modify**:
- `migrations/003_paper_trading.sql` (new)
- `src/broker/paper.rs` (new)
- `src/broker/mod.rs` (add paper broker integration)
- `src/config/mod.rs` (add paper trading config)
- `.env.example` (add paper trading variables)

### Phase 3: Web UI for Rule and Risk Management

**Objective**: Create web interface for trading rule configuration and monitoring

**Tasks**:
1. **Rule Management UI** (`/ui/rules`):
   - Create, edit, delete trading rules
   - Rule condition builder interface
   - Rule testing and validation
   - Import/export rule configurations

2. **Risk Configuration UI** (`/ui/risk`):
   - Risk parameter adjustment interface
   - Real-time risk utilization display
   - Risk scenario testing
   - Portfolio exposure visualization

3. **Symbol Management UI** (`/ui/symbols`):
   - Watchlist symbol management
   - Symbol search and addition
   - Symbol performance tracking
   - Market data preview

4. **Paper Trading Dashboard** (`/ui/paper`):
   - Start/stop paper trading sessions
   - Real-time P&L tracking
   - Position monitoring
   - Trade history and analytics
   - Performance charts and metrics

5. **REST API Extensions**:
   ```
   # Paper Trading Session Management
   POST /api/paper/sessions          # Create new paper session
   GET  /api/paper/sessions          # List all sessions
   GET  /api/paper/sessions/{id}     # Get session details
   PUT  /api/paper/sessions/{id}     # Update session (start/stop)
   DELETE /api/paper/sessions/{id}   # Delete session

   # Trading Rules Management  
   GET    /api/rules                 # List all rules
   POST   /api/rules                 # Create new rule
   GET    /api/rules/{id}            # Get rule details
   PUT    /api/rules/{id}            # Update rule
   DELETE /api/rules/{id}            # Delete rule
   POST   /api/rules/{id}/activate   # Activate rule
   POST   /api/rules/{id}/deactivate # Deactivate rule

   # Paper Trade Tracking
   GET  /api/paper/trades            # List trades (with filtering)
   GET  /api/paper/trades/{id}       # Get trade details
   GET  /api/paper/portfolio         # Current paper portfolio status
   GET  /api/paper/performance       # Performance metrics and analytics
   ```

**Files to Create/Modify**:
- `src/web/handlers/` (new directory structure)
- `src/web/handlers/rules.rs` (new)
- `src/web/handlers/paper.rs` (new)
- `src/web/handlers/risk.rs` (new)
- `src/web/static/` (new - HTML/CSS/JS files)
- `src/web/mod.rs` (extend with new routes)

### Phase 4: Advanced Features and Polish

**Objective**: Enhanced functionality and user experience improvements

**Tasks**:
1. **Real-time WebSocket Updates**:
   - Live P&L updates in web UI
   - Real-time trade notifications
   - Market data streaming to dashboard

2. **Advanced Analytics**:
   - Rule performance comparison
   - Risk-adjusted returns calculation
   - Drawdown analysis
   - Sharpe ratio and other metrics

3. **Configuration Import/Export**:
   - JSON export of rule configurations
   - Bulk rule import capability
   - Configuration versioning and backup

4. **Performance Optimizations**:
   - Database indexing for trade queries
   - Caching for frequently accessed data
   - API response optimization

## Implementation Timeline

### Week 1: Foundation (Phase 1A-1C)
- **Day 1-2**: Remove hardcoded risk parameters
- **Day 3-4**: Remove hardcoded symbols and implement watchlist
- **Day 5-7**: Remove other hardcoded values and configuration cleanup

### Week 2: Paper Trading Core (Phase 2)
- **Day 1-3**: Database schema and paper broker implementation
- **Day 4-5**: Trading mode switching and integration
- **Day 6-7**: Testing and validation

### Week 3: Web UI (Phase 3)
- **Day 1-3**: Rule and risk management interfaces
- **Day 4-5**: Paper trading dashboard and monitoring
- **Day 6-7**: API endpoints and integration testing

### Week 4: Polish and Advanced Features (Phase 4)
- **Day 1-3**: Real-time updates and advanced analytics
- **Day 4-5**: Import/export functionality
- **Day 6-7**: Performance optimization and final testing

## Key Design Decisions

### Trading Execution
- **No Simulation Complexity**: Instant fills at market price, no delays or partial fills
- **Real Market Data**: Use actual Polygon.io prices for realistic execution
- **Commission Tracking**: Configurable per-share commission applied to all trades

### Data Persistence
- **Session-based Trading**: Each paper trading run is a separate session
- **Complete Audit Trail**: All trades, rule evaluations, and configuration changes logged
- **Historical Analysis**: Support for comparing performance across sessions

### User Experience
- **Web-First Interface**: Primary configuration and monitoring via web UI
- **Real-time Updates**: Live P&L and position updates
- **Responsive Design**: Works on desktop and mobile devices

## Questions for Clarification

1. **Multiple Sessions**: Should the system support multiple concurrent paper trading sessions?

2. **Rule Conflicts**: How should conflicting rules be handled (first match, highest priority, or all execute)?

3. **Symbol Universe**: Default to LightSpeed certification symbols or start with empty watchlist?

4. **Historical Data**: Should paper trading sessions persist across system restarts?

5. **Performance Metrics**: Which specific trading metrics should be calculated and displayed?

6. **Rule Templates**: Should we provide pre-configured rule templates for common strategies?

## Success Criteria

### Phase 1 Complete When:
- ✅ No hardcoded values remain in rules engine
- ✅ All risk parameters configurable via database
- ✅ Symbol watchlist fully dynamic
- ✅ Configuration loading from environment/database works

### Phase 2 Complete When:
- ✅ Paper trading mode functional with instant fills
- ✅ Commission calculation working correctly
- ✅ All paper trades logged to database
- ✅ P&L tracking accurate
- ✅ Rules engine works transparently with paper broker

### Phase 3 Complete When:
- ✅ Web UI for rule creation/editing functional
- ✅ Risk parameter configuration via web interface working
- ✅ Real-time P&L display operational
- ✅ Symbol watchlist management via web UI
- ✅ Paper trading session management complete

### Final Success Criteria:
- ✅ User can configure trading rules entirely via web UI
- ✅ User can specify symbols and risk settings via web UI
- ✅ System operates in paper mode with real market data
- ✅ All trades tracked in database with full audit trail
- ✅ P&L visible and accurate on web dashboard
- ✅ No hardcoded trading parameters remain in system

---
**Next Steps**: Begin with Phase 1A - removing hardcoded risk parameters and implementing database-driven risk configuration.