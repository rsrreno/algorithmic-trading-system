# Algorithmic Trading System

**Project Version:** v8.28.25.1  
**Last Updated:** 2025-09-02 - **POLYGON WEBSOCKET INTEGRATION COMPLETE**: Real-time streaming implemented with full REST API management  
**Related Files:** `CLAUDE.md`, `STATUS.md`

## Overview
High-frequency algorithmic trading system built in Rust for momentum and breakout trading strategies. Features <5ms decision latency and integrations with Polygon.io data feeds and LightSpeed brokerage.

**Current Status**: Polygon.io API integration complete including WebSocket streaming - market data, technical indicators, news, reference data, and real-time streaming all implemented and working. Rules engine implementation ready to begin.

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
POLYGON_USE_DELAYED_DATA=true

# WebSocket Configuration (Optional - Real-time Streaming)
POLYGON_ENABLE_WEBSOCKET=true
POLYGON_WEBSOCKET_RECONNECT_INTERVAL=5
POLYGON_WEBSOCKET_HEARTBEAT_INTERVAL=30
POLYGON_WEBSOCKET_MAX_SUBSCRIPTIONS=100
POLYGON_WEBSOCKET_BUFFER_SIZE=1000
POLYGON_DEFAULT_SUBSCRIPTIONS=GOOGL,AMZN,TSLA,MSFT,CHWY
POLYGON_AUTO_SUBSCRIBE_MOVERS=true

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
# Use LightSpeed certification test symbols for consistent behavior
curl -X POST http://localhost:8080/api/symbol \
  -H "Content-Type: application/json" \
  -d '{"symbol":"GOOGL"}'

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

#### Technical Indicators (Polygon.io Integration)
```bash
# Simple Moving Average (SMA) - 50-day window
curl -s "http://localhost:8080/api/indicators/sma/AAPL?window=50&timespan=day"

# Exponential Moving Average (EMA) - 50-day window  
curl -s "http://localhost:8080/api/indicators/ema/AAPL?window=50&timespan=day"

# Relative Strength Index (RSI) - 14-day window
curl -s "http://localhost:8080/api/indicators/rsi/AAPL?window=14&timespan=day"

# Moving Average Convergence Divergence (MACD)
curl -s "http://localhost:8080/api/indicators/macd/AAPL?short_window=12&long_window=26&signal_window=9"

# Example response:
{
  "success": true,
  "value": 205.36659999999992,
  "data": {
    "request_id": "828cae7a1e7c3cd5b281572ab978c134",
    "results": {
      "underlying": {
        "aggregates": null,
        "url": "https://api.polygon.io/v2/aggs/ticker/AAPL/range/1/day/..."
      }
    }
  }
}
```

#### Market Data & News (Polygon.io Integration)
```bash
# Single ticker snapshot
curl -s "http://localhost:8080/api/snapshot/AAPL"

# News with sentiment analysis
curl -s "http://localhost:8080/api/news?ticker=AAPL&limit=5"

# Market movers (gainers and losers)
curl -s "http://localhost:8080/api/market/movers/gainers?limit=10"
curl -s "http://localhost:8080/api/market/movers/losers?limit=10"

# Full market snapshot (all US stocks)
curl -s "http://localhost:8080/api/market/snapshot/full"

# Daily market summary for specific date
curl -s "http://localhost:8080/api/market/summary?date=2024-08-29"

# Previous day bar data
curl -s "http://localhost:8080/api/market/previous/AAPL"

# Minute aggregates for date range
curl -s "http://localhost:8080/api/market/minute/AAPL?from=2024-08-29&to=2024-08-29"

# Market status
curl -s "http://localhost:8080/api/market/status"

# Example news response:
{
  "success": true,
  "data": [
    {
      "title": "Apple Stock Analysis Update",
      "author": "Market Analyst",
      "published_utc": "2025-08-30T12:00:00Z",
      "insights": [
        {
          "ticker": "AAPL",
          "sentiment": "positive",
          "sentiment_reasoning": "Strong quarterly results with revenue growth"
        }
      ]
    }
  ]
}
```

#### Reference Data & Fundamentals (Polygon.io Integration)
```bash
# All US stock tickers (limit 10 for demo)
curl -s "http://localhost:8080/api/reference/tickers?limit=10"

# Stock exchanges with MIC codes
curl -s "http://localhost:8080/api/reference/exchanges"

# Stock splits history for a specific ticker
curl -s "http://localhost:8080/api/reference/splits/AAPL"

# Financial statements (balance sheet, income, cash flow)
curl -s "http://localhost:8080/api/financials/AAPL"

# Example financials response:
{
  "success": true,
  "data": [
    {
      "company_name": "Apple Inc.",
      "end_date": "2025-06-28",
      "timeframe": "quarterly",
      "financials": {
        "balance_sheet": {
          "assets": {"value": 331495000000.0, "unit": "USD"},
          "equity": {"value": 65830000000.0, "unit": "USD"}
        },
        "income_statement": {
          "revenues": {"value": 94036000000.0, "unit": "USD"},
          "net_income_loss": {"value": 23434000000.0, "unit": "USD"}
        }
      }
    }
  ]
}
```

#### WebSocket Real-time Streaming (Optional)
```bash
# Check WebSocket connection status
curl -s "http://localhost:8080/api/websocket/status"

# Get current WebSocket subscriptions
curl -s "http://localhost:8080/api/websocket/subscriptions"

# Subscribe to additional symbols
curl -s -X POST "http://localhost:8080/api/websocket/subscribe" \
  -H "Content-Type: application/json" \
  -d '{"symbols": ["AAPL", "NVDA", "META"]}'

# Unsubscribe from symbols
curl -s -X POST "http://localhost:8080/api/websocket/unsubscribe" \
  -H "Content-Type: application/json" \
  -d '{"symbols": ["META"]}'

# Example WebSocket status response:
{
  "success": true,
  "websocket_enabled": true,
  "status": "Authenticated",
  "url": "wss://delayed.polygon.io/stocks",
  "subscriptions_count": 7,
  "error": null
}
```

**WebSocket Features:**
- **Real-time streaming**: Delayed (15-min) or real-time data based on Polygon plan
- **Dynamic subscriptions**: Add/remove symbols via REST API without restart
- **Background processing**: Non-blocking operation, REST API continues working
- **Memory cache**: WebSocket data populates same cache as REST API (<1ms access)
- **URL switching**: Automatically uses correct delayed vs real-time endpoint

### 🧪 **LightSpeed Certification Test Symbols**

**⚠️ IMPORTANT:** The LightSpeed certification environment only accepts specific test symbols that exhibit predefined behaviors to replicate real-world trading scenarios. **Use only these symbols for testing.**

| Symbol | Behavior | Use Case |
|--------|----------|----------|
| **GOOGL** | Immediate fill | Test successful order execution |
| **AMZN** | Partial fill, balance remains open | Test partial fill handling |
| **CHWY** | Multiple partial fills until complete | Test incremental fill processing |
| **F** | Multiple partial fills until complete | Test incremental fill processing |
| **GE** | Multiple partial fills until complete | Test incremental fill processing |
| **MSFT** | Order rejection | Test error handling |
| **TSLA** | Order does not fill | Test unfilled order management |
| **ORCL** | Order cancel scenario | Test cancellation logic |
| **BAC** | Order replace (increase quantity) | Test order modification |

**Testing Strategy:**
- Use **GOOGL** for reliable immediate fills
- Use **MSFT** to test rejection handling
- Use **AMZN** to test partial fill logic
- Use **TSLA** to test timeout/unfilled scenarios

#### Order Management (LightSpeed Integration - Sandbox)
```bash
# Place buy order (TESTED AND WORKING)
# Use GOOGL (immediate fill) for reliable testing
curl -X POST http://localhost:8080/api/order \
  -H "Content-Type: application/json" \
  -d '{
    "symbol": "GOOGL",
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

#### Portfolio & Position Management (LightSpeed Integration)
```bash
# Get all current positions from LightSpeed
curl http://localhost:8080/api/positions

# Get portfolio summary with calculations
curl http://localhost:8080/api/portfolio

# Example positions response:
{
  "success": true,
  "positions": {
    "GOOGL": {
      "id": "8e2a1a9d-6851-4f5b-8f81-ba8e7dbbebf9",
      "symbol": "GOOGL",
      "side": "Long",
      "quantity": 20,
      "entry_price": 0.0,
      "current_price": 0.0,
      "opened_at": "2025-09-03T01:14:26.675610364Z",
      "status": "Open"
    }
  }
}

# Example portfolio response:
{
  "success": true,
  "portfolio": {
    "total_value": 50000.0,
    "available_cash": 50000.0,
    "total_exposure": 0.0,
    "position_count": 6,
    "positions": { ... },
    "last_updated": "2025-09-03T01:14:47.261367865+00:00"
  }
}
```

**Features:**
- **Real-time data**: Direct from LightSpeed certification environment
- **6 test positions**: Using LightSpeed test symbols (GOOGL, AMZN, TSLA, MSFT, CHWY, F, GE)
- **Live calculations**: Portfolio value, exposure, available cash
- **WebSocket integration**: Positions update automatically via WebSocket messages

### Current Capabilities
- **Technical Indicators**: Complete implementation (SMA, EMA, RSI, MACD) with real Polygon data
- **Market Data**: Complete implementation (snapshots, aggregates, movers, full market snapshot, daily summaries)
- **Reference Data**: Complete implementation (tickers, exchanges, splits, market status) 
- **News Integration**: Complete implementation with AI-powered sentiment analysis
- **Financial Data**: Complete implementation (balance sheets, income statements, cash flow)
- **Order Management**: BUY orders working (SELL orders in testing)  
- **Portfolio Management**: Real-time positions and portfolio calculations from LightSpeed
- **Position Tracking**: Live WebSocket-based position updates from certification environment
- **Performance**: In-memory caching system for <5ms decision latency
- **Web Interface**: Full REST API implemented, web UI in development

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
1. **Rules Engine**: Build algorithmic decision logic using implemented market data APIs
2. **WebSocket Streaming**: Add real-time data feeds for live market updates
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