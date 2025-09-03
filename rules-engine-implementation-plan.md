# Rules Engine Implementation Plan

## Phase 1: Core Rules Engine Architecture (Days 1-2)

### 1.1 Enhanced Data Structures
- **Extend existing types** in `src/types/mod.rs`:
  - Add `PortfolioState` struct for position tracking
  - Add `TechnicalIndicators` struct for cached indicator data
  - Extend `TradingRule` with stop-loss and take-profit parameters
  - Add `RiskParameters` struct for portfolio-level constraints

### 1.2 Rules Engine Foundation  
- **Create `src/rules/engine.rs`** - Core decision engine with <5ms performance
- **Create `src/rules/conditions.rs`** - Rule condition evaluation logic
- **Create `src/rules/risk.rs`** - Risk management and position sizing
- **Create `src/rules/cache.rs`** - High-performance indicator data cache

## Phase 2: Decision Pipeline Implementation (Days 2-3)

### 2.1 Real-time Decision Logic
- **Market Data Processing**: Integration with existing Polygon cache
- **Technical Indicator Evaluation**: SMA, EMA, RSI, MACD threshold checking  
- **Price Action Analysis**: Momentum and breakout pattern detection
- **Volume Analysis**: Volume confirmation for signal strength

### 2.2 Risk Management Integration
- **Position Limits**: Max positions per portfolio (user-configurable)
- **Stop-Loss Logic**: Percentage/dollar-based automatic stop orders
- **Take-Profit Logic**: Momentum-based profit taking
- **Portfolio Risk**: Overall exposure and concentration limits

## Phase 3: Integration & Performance (Days 3-4)

### 3.1 Broker Integration
- **Order Generation**: Automatic buy/sell order creation
- **Stop-Loss Orders**: Integration with LightSpeed stop/limit orders
- **Position Tracking**: Real-time position monitoring and updates

### 3.2 Performance Optimization
- **Memory-First Architecture**: All decisions from cached data
- **Async Processing**: Non-blocking rule evaluation
- **Latency Monitoring**: Built-in <5ms performance tracking

## Phase 4: REST API & Testing (Day 4-5)

### 4.1 Rules Management API
- **CRUD Operations**: Create, read, update, delete trading rules
- **Rule Testing**: Backtest rules against historical data
- **Performance Metrics**: Rule success rates and profitability

### 4.2 Decision Logging & Persistence
- **Decision Database**: SQLite integration for decision audit trail
- **Trade Correlation**: Link decisions to actual trades
- **Performance Analytics**: Rule effectiveness tracking

## Key Design Principles

### Ultra-Low Latency (<5ms)
- **Memory-only decisions**: All rule evaluation from cached data
- **Pre-computed indicators**: Background refresh of technical indicators
- **Optimized data structures**: Arc/RwLock for thread-safe performance
- **Minimal allocations**: Reuse structures where possible

### Risk Management First
- **Position limits**: Configurable max concurrent positions
- **Stop-loss automation**: Immediate stop orders on position entry
- **Portfolio exposure**: Total risk monitoring across all positions
- **Capital preservation**: Conservative position sizing

### Momentum/Breakout Strategy Focus
- **News-driven triggers**: Integration with Polygon news sentiment
- **Volume confirmation**: Require volume support for signals
- **Technical confluence**: Multiple indicator agreement required
- **Quick exits**: Fast profit-taking on momentum exhaustion

## Success Metrics
- **Latency**: <5ms from data ingestion to decision output
- **Throughput**: Handle multiple symbols simultaneously  
- **Accuracy**: Robust rule evaluation with proper risk management
- **Reliability**: 99.9% uptime with proper error handling