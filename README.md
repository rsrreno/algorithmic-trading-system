# Algorithmic Trading System

**Project Version:** v8.23.25.1  
**Last Updated:** 2025-08-23 - Added remote repository push to claude commit workflow  
**Related Files:** `CLAUDE.md`, `STATUS.md`

## Overview
High-frequency algorithmic trading system built in Rust for momentum and breakout trading strategies. Features <5ms decision latency and integrations with Polygon.io data feeds and LightSpeed brokerage.

**Current Status**: Phase 3 Development - Rules Engine Implementation (see `STATUS.md` for detailed progress)

## Quick Start

### Prerequisites
- Docker v28+ with Compose v2+
- Polygon.io API account (Free tier supported, Stock Advanced recommended)
- LightSpeed trading account (Sandbox for development)

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
    "date": "2025-01-22",
    "open": 225.00,
    "high": 230.50,
    "low": 224.75,
    "close": 229.87,
    "volume": 45234567,
    "change": 4.87,
    "change_percent": 2.16
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

### Current API Limitations
- **Order Types**: Only BUY orders tested (SELL/SELL_SHORT need testing)
- **Data Feed**: Single REST endpoint verified (free tier only)
- **Account**: Sandbox environment only
- **Web Interface**: Basic HTML placeholder (CURL APIs functional)

## Web Interface

Access the web interface at: http://localhost:8080

**Current Status**: Basic HTML response indicating server is running. No functional trading interface implemented yet.

**Planned Features**:
- Rules configuration dashboard
- Position monitoring interface
- Trading performance analytics
- Real-time market data display

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
- **Market Data**: Polygon.io (currently free tier, upgrade to Stock Advanced planned)
- **Broker Integration**: LightSpeed Connect WebSocket API
- **Database**: Local SQLite for trade history and decision logging

## Account Requirements & Upgrades

### Current Limitations
1. **Polygon.io Free Tier**:
   - Limited to basic daily/minute aggregates
   - No real-time WebSocket streaming
   - No premium endpoints (top movers, news)

2. **LightSpeed Sandbox**:
   - Test environment only
   - Paper trading simulation
   - Production account needed for live trading

### Recommended Upgrades
1. **Polygon.io Stock Advanced**: Real-time data feeds and WebSocket streaming
2. **LightSpeed Production Account**: Live trading capabilities
3. **Development**: Complete broker API testing (SELL orders, cancellations)

## File Organization

```
trading-system/
├── README.md              # This file - user documentation
├── CLAUDE.md              # Development guidelines and workflow
├── STATUS.md              # Implementation progress tracking
├── src/                   # Rust source code
├── docs/                  # Project specifications (PDF)
├── data/                  # SQLite database storage
├── logs/                  # Application log files
└── docker-compose.yml     # Container orchestration
```

## Support & Documentation

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
**Next Steps**: Complete rules engine implementation and upgrade to production accounts for live trading capabilities.