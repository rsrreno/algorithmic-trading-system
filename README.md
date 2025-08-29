# Algorithmic Trading System

**Project Version:** v8.28.25.1  
**Last Updated:** 2025-08-28 - **NORMAL MODE VERIFIED**: System working with both modules - Polygon APIs need implementation  
**Related Files:** `CLAUDE.md`, `STATUS.md`

## Overview
High-frequency algorithmic trading system built in Rust for momentum and breakout trading strategies. Features <5ms decision latency and integrations with Polygon.io data feeds and LightSpeed brokerage.

**Current Status**: Phase 3 Development - Rules Engine Implementation (BLOCKED - data APIs documented but not implemented)

## Quick Start

### Prerequisites
- Docker v28+ with Compose v2+
- Polygon.io API account (**Stock Starter plan** - provides comprehensive market data and technical indicators)
- LightSpeed trading account (Sandbox for development)

**📋 API Documentation:** See [`ENDPOINTS.md`](ENDPOINTS.md) for complete endpoint reference and testing status.

### Environment Setup

1. **Clone and navigate to project:**
```bash
cd trading-system
```

2. **Create environment configuration:**
```bash
cp .env.example .env
```

3. **Configure required environment variables:**
```env
# Polygon.io Data Feed Configuration
POLYGON_API_KEY=your_polygon_api_key_here

# LightSpeed Broker Configuration  
LIGHTSPEED_API_KEY=your_lightspeed_api_key_here
LIGHTSPEED_ACCOUNT_ID=your_lightspeed_account_id_here
LIGHTSPEED_CONNECTION_URL=wss://onboarding.connecttrade.com:28052
LIGHTSPEED_SANDBOX=true

# Application Configuration
DATABASE_URL=sqlite:./data/trading.db
MAX_MEMORY_MB=1024
BIND_ADDRESS=127.0.0.1:8080
METRICS_ADDRESS=127.0.0.1:9090
RUST_LOG=info
```

4. **Start the application:**
```bash
docker compose up --build
```

## API Testing

### Available Endpoints

#### System Health
```bash
# Basic health check
curl http://localhost:8080/health

# System status with broker connection
curl http://localhost:8080/api/status
```

#### Market Data (Polygon.io Integration)
```bash
# Symbol lookup with daily data
curl -X POST http://localhost:8080/api/symbol \
  -H "Content-Type: application/json" \
  -d '{"symbol":"AAPL"}'

# Example response:
{
  "success": true,
  "data": {
    "symbol": "AAPL",
    "date": "2025-08-26",
    "open": 226.48,
    "high": 229.30,
    "low": 226.23,
    "close": 227.16,
    "volume": 30982024,
    "change": -0.69,
    "change_percent": -0.30
  }
}
```

#### Order Management (LightSpeed Integration - Sandbox)
```bash
# Place buy order (TESTED AND WORKING)
curl -X POST http://localhost:8080/api/order \
  -H "Content-Type: application/json" \
  -d '{
    "symbol": "AAPL",
    "side": "BUY",
    "order_type": "LIMIT",
    "quantity": 1,
    "price": 150.00
  }'

# Example response:
{
  "success": true,
  "order_id": "LS1A2B3C4D5E6F7G8H9I0J",
  "error": null
}
```

### Current API Capabilities & Limitations
- **Technical Indicators**: ⚠️ **DOCUMENTED** - RSI, MACD, EMA, SMA endpoints tested but **NOT IMPLEMENTED** in application
- **Historical Data**: ⚠️ **DOCUMENTED** - 5 years historical data available but **NOT IMPLEMENTED** in application  
- **Market Data**: ⚠️ **DOCUMENTED** - Real-time snapshots, minute aggregates, market movers **NOT IMPLEMENTED** in application
- **Fundamentals**: ⚠️ **DOCUMENTED** - Financial statements, news with sentiment analysis **NOT IMPLEMENTED** in application
- **Order Types**: ✅ BUY orders tested and working (SELL/SELL_SHORT need testing)  
- **Account**: ❌ Sandbox environment only
- **Web Interface**: ❌ Basic HTML placeholder (CURL APIs functional)
- **Data Delay**: ⚠️ 15-minute delayed data on Stock Starter plan
- **Implementation Status**: ❌ **CRITICAL** - All Polygon endpoints documented but 0% implemented in `src/data/mod.rs`

## Web Interface

Access the web interface at: http://localhost:8080

**Current Status**: Basic HTML response indicating server is running. No functional trading interface implemented yet.

**Planned Features**:
- Technical indicator dashboard (RSI, MACD, SMA, EMA charts)
- Rules configuration interface with indicator-based conditions
- Position monitoring with real-time P&L
- Market scanner with top movers and sentiment analysis
- Trading performance analytics with decision audit trail

## Development Information

### Architecture
- **Language**: Rust (for performance requirements)
- **Database**: SQLite with migrations
- **Runtime**: Tokio async with Docker containerization
- **APIs**: REST with planned WebSocket streaming

### Performance Targets
- **Decision Latency**: <5ms from data to trade execution
- **Strategy**: Momentum/breakout trading (seconds to minutes holding)
- **Risk Management**: Configurable stop-loss and position limits

### Data Sources
- **Market Data**: Polygon.io Stock Starter (unlimited API calls, 5yr history, all technical indicators)
- **Technical Indicators**: SMA, EMA, RSI, MACD via Polygon REST APIs
- **Real-time Updates**: WebSocket streaming for price/volume updates (15-min delayed)
- **Broker Integration**: LightSpeed Connect WebSocket API
- **Database**: Local SQLite for trade history and decision logging

## Account Requirements & Upgrades

### Current Capabilities & Limitations
1. **Polygon.io Stock Starter** - ✅ **COMPREHENSIVE**:
   - ✅ All technical indicators available (RSI, MACD, EMA, SMA)
   - ✅ 5 years historical data with unlimited API calls
   - ✅ Market snapshots, minute aggregates, top movers
   - ✅ News with sentiment analysis, financial fundamentals
   - ✅ WebSocket streaming available for real-time updates
   - ⚠️ **15-minute data delay** (real-time requires higher tier)

2. **LightSpeed Sandbox**:
   - Test environment only
   - Paper trading simulation  
   - Production account needed for live trading

### Next Development Steps
1. **CRITICAL - Polygon API Implementation in Application**: 
   - Implement technical indicator endpoints in `src/data/mod.rs` (SMA, EMA, RSI, MACD)
   - Implement market data endpoints in `src/data/mod.rs` (snapshots, aggregates, movers)
   - Build memory cache system for <5ms decision performance
   - Implement WebSocket streaming for real-time price updates
2. **BLOCKED - Rules Engine**: Build decision logic using implemented indicator APIs (blocked until #1 complete)
3. **TESTING - Broker Module**: Complete API testing (SELL orders, cancellations)
4. **OPTIONAL - Real-time Upgrade**: Polygon Advanced for live (non-delayed) data
5. **FUTURE - Production**: LightSpeed production account for live trading

## File Organization

```
trading-system/
├── README.md              # This file - user documentation
├── CLAUDE.md              # Development guidelines and workflow
├── STATUS.md              # Implementation progress tracking
├── ENDPOINTS.md           # Complete API endpoint reference and status
├── src/                   # Rust source code
├── docs/                  # Project specifications (PDF)
├── data/                  # SQLite database storage
├── logs/                  # Application log files
└── docker-compose.yml     # Container orchestration
```

## Support & Documentation

- **API Endpoints**: See `ENDPOINTS.md` - Complete endpoint reference and testing status
- **Development Workflow**: See `CLAUDE.md`
- **Implementation Status**: See `STATUS.md`  
- **Project Architecture**: See `README_orig.md`
- **Requirements Specification**: See `docs/module_overview.pdf`

## Troubleshooting

### Container Issues
```bash
# View application logs
docker compose logs -f trading-system

# Restart services
docker compose down && docker compose up --build
```

### API Connection Issues
- **Polygon.io**: Verify API key and account status
- **LightSpeed**: Confirm sandbox credentials and connection URL
- **Database**: Check `./data/` directory permissions

### Performance Monitoring
- **Memory Usage**: Configurable via MAX_MEMORY_MB
- **Latency Tracking**: Built-in performance measurement (metrics module pending)
- **Database Growth**: Monitor `./data/trading.db` file size

---
**Next Steps**: Implement documented Polygon technical indicator APIs in `src/data/mod.rs` to unblock rules engine development.