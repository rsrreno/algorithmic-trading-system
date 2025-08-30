# Algorithmic Trading System

**Project Version:** v8.30.25.1  
**Last Updated:** 2025-08-30 - **POLYGON API INTEGRATION COMPLETE**: Technical indicators fully implemented with real-time data  
**Related Files:** `CLAUDE.md`, `STATUS.md`

## Overview
High-frequency algorithmic trading system built in Rust for momentum and breakout trading strategies. Features <5ms decision latency and integrations with Polygon.io data feeds and LightSpeed brokerage.

**Current Status**: Technical indicators operational with real Polygon data, ready for rules engine development

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

### Current Capabilities
- **Order Management**: BUY orders working (SELL orders in testing)
- **Market Data**: Basic daily aggregates implemented
- **Technical Indicators**: Available via Polygon API (implementation in progress)
- **Account**: Sandbox environment for development
- **Web Interface**: REST API functional, web UI in development

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

## Account Status

**Data Provider**: Polygon.io Stock Starter plan - comprehensive market data with technical indicators, 5-year history, unlimited API calls (15-minute delay)

**Broker**: LightSpeed sandbox account - development and testing environment

### Next Development Steps
1. **Polygon API Integration**: Implement technical indicators and market data endpoints
2. **Rules Engine**: Build algorithmic decision logic
3. **Broker Testing**: Complete order type testing and position management
4. **Web Interface**: Build functional trading dashboard

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
**Development Focus**: Implementing Polygon.io data APIs and rules engine for algorithmic trading decisions.