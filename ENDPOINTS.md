# API Endpoints Documentation

**Project Version:** v8.28.25.1  
**Last Updated:** 2025-09-02  
**Related Files:** `CLAUDE.md`, `STATUS.md`, `README.md`

This document contains a comprehensive inventory of all external API endpoints used by the trading system, their current implementation status, and related documentation links.

---

## Polygon.io API Endpoints

**Plan:** Stock Starter  
**Base URL:** `https://api.polygon.io`  
**Authentication:** API Key via query parameter `?apikey={key}`  
**Rate Limits:** Unlimited API calls  
**Data Delay:** 15 minutes (real-time available on higher tiers)

**⚠️ Testing Notes:**
- **Market Hours Dependency:** Some endpoints (market movers, real-time data) return empty results when markets are closed
- **Subscription Limitations:** Some endpoints may require higher tier plans for full functionality  
- **Testing Recommendation:** Test market-dependent endpoints during US market hours for accurate results:
  - **Pre-Market:** 4:00 AM - 9:30 AM ET
  - **Regular Hours:** 9:30 AM - 4:00 PM ET  
  - **After-Hours:** 4:00 PM - 8:00 PM ET
  - **Note:** Extended hours data may have different availability/pricing on Stock Starter plan

### Technical Indicators

#### Simple Moving Average (SMA)
- **Endpoint:** `GET /v1/indicators/sma/{stockTicker}`
- **Status:** ✅ **TESTED & WORKING**
- **Implementation:** ✅ **IMPLEMENTED & WORKING**
- **Parameters:**
  - `timestamp`: Date or millisecond timestamp
  - `timespan`: Size of aggregate time window (day, week, month)
  - `adjusted`: Whether aggregates are split-adjusted (default: true)
  - `window`: Size of SMA calculation window (default: 50)
  - `series_type`: Price type used for calculation (e.g., 'close')
  - `order`: Result ordering by timestamp
  - `limit`: Number of results (default: 10, max: 5000)
- **Documentation:** https://polygon.io/docs/rest/stocks/technical-indicators/simple-moving-average

#### Exponential Moving Average (EMA)
- **Endpoint:** `GET /v1/indicators/ema/{stockTicker}`
- **Status:** ✅ **TESTED & WORKING**
- **Implementation:** ✅ **IMPLEMENTED & WORKING**
- **Parameters:** Same as SMA with EMA-specific window calculations
- **Documentation:** https://polygon.io/docs/rest/stocks/technical-indicators/exponential-moving-average

#### Relative Strength Index (RSI)
- **Endpoint:** `GET /v1/indicators/rsi/{stockTicker}`
- **Status:** ✅ **TESTED & WORKING**
- **Implementation:** ✅ **IMPLEMENTED & WORKING**
- **Parameters:**
  - `window`: Size of RSI calculation window (default: 14)
  - Other parameters same as SMA
- **Example Response:** `{"results":{"values":[{"timestamp":1756094400000,"value":60.698}]}}`
- **Documentation:** https://polygon.io/docs/rest/stocks/technical-indicators/relative-strength-index

#### Moving Average Convergence Divergence (MACD)
- **Endpoint:** `GET /v1/indicators/macd/{stockTicker}`
- **Status:** ✅ **TESTED & WORKING**
- **Implementation:** ✅ **IMPLEMENTED & WORKING**
- **Parameters:**
  - `short_window`: Short window size for MACD calculation (default: 12)
  - `long_window`: Long window size for MACD calculation (default: 26)
  - `signal_window`: Window size for MACD signal line (default: 9)
  - Other parameters same as SMA
- **Example Response:** `{"results":{"values":[{"timestamp":1756094400000,"value":4.89,"signal":5.06,"histogram":-0.17}]}}`
- **Documentation:** https://polygon.io/docs/rest/stocks/technical-indicators/moving-average-convergence-divergence

### Market Data & Snapshots

#### Single Ticker Snapshot
- **Endpoint:** `GET /v2/snapshot/locale/us/markets/stocks/tickers/{ticker}`
- **Status:** ✅ **IMPLEMENTED & WORKING**
- **Implementation:** ✅ **IMPLEMENTED & WORKING** (via `/api/snapshot/{symbol}` endpoint)
- **Data Delay:** 15-minute delay on Stock Starter plan (real-time requires higher tier)
- **Response:** OHLCV, minute data, previous day data with current pricing
- **Example:** `{"ticker":{"ticker":"AAPL","todaysChange":-0.69,"day":{"o":226.48,"h":229.3,"l":226.23,"c":227.16,"v":3.0982024e+07}}}`

#### Full Market Snapshot
- **Endpoint:** `GET /v2/snapshot/locale/us/markets/stocks`
- **Status:** ✅ **TESTED & WORKING**
- **Implementation:** ✅ **IMPLEMENTED & WORKING** (via `/api/market/snapshot/full` endpoint)
- **Response:** All US stock snapshots in single call
- **Web API Route:** `GET /api/market/snapshot/full`
- **Cache:** In-memory cache with 5-minute TTL for <5ms performance
- **Market Hours Note:** Returns empty/404 when markets are closed

#### Top Market Movers
- **Endpoint:** `GET /v2/snapshot/locale/us/markets/stocks/gainers`
- **Endpoint:** `GET /v2/snapshot/locale/us/markets/stocks/losers`
- **Status:** ✅ **TESTED & WORKING**
- **Implementation:** ✅ **IMPLEMENTED & WORKING** (via `/api/market/movers/gainers` and `/api/market/movers/losers` endpoints)
- **Cache:** In-memory cache with 5-minute TTL for <5ms performance
- **Market Hours Note:** Returns empty results when markets are closed
- **Example Response:** Top 20 gainers with full ticker data including change percentages

#### Minute Aggregates
- **Endpoint:** `GET /v2/aggs/ticker/{ticker}/range/1/minute/{from}/{to}`
- **Status:** ✅ **TESTED & WORKING**
- **Implementation:** ✅ **IMPLEMENTED & WORKING** (via `/api/market/minute/{symbol}?from={date}&to={date}` endpoint)
- **Parameters:** Date range, adjustment settings
- **Response:** OHLCV data per minute with transaction counts
- **Example:** `{"results":[{"v":3107,"vw":227.5691,"o":227.9,"c":227.45,"h":227.9,"l":227.39,"t":1756108800000}]}`

#### Previous Day Bar
- **Endpoint:** `GET /v2/aggs/ticker/{ticker}/prev`
- **Status:** ✅ **TESTED & WORKING**
- **Implementation:** ✅ **IMPLEMENTED & WORKING** (via `/api/market/previous/{symbol}` endpoint)
- **Cache:** In-memory cache with 5-minute TTL for <5ms performance
- **Response:** Complete previous trading day OHLCV data

#### Daily Market Summary  
- **Endpoint:** `GET /v2/aggs/grouped/locale/us/market/stocks/{date}`
- **Status:** ✅ **TESTED & WORKING**
- **Implementation:** ✅ **IMPLEMENTED & WORKING** (via `/api/market/summary?date={date}` endpoint)
- **Response:** All tickers' daily aggregates for specified date (tested with 10,532+ tickers)
- **Web API Route:** `GET /api/market/summary?date=YYYY-MM-DD`
- **Cache:** In-memory cache with 5-minute TTL for <5ms performance
- **Default:** Uses yesterday's date if no date parameter provided
- **Example Response:** Returns daily OHLCV data for all active US stocks for the specified trading day

### Reference & Corporate Data

#### All Tickers
- **Endpoint:** `GET /v3/reference/tickers?market=stocks`
- **Status:** ✅ **TESTED & WORKING**
- **Implementation:** ✅ **IMPLEMENTED & WORKING** (via `/api/reference/tickers?limit={limit}` endpoint)
- **Response:** Complete US stock ticker list with metadata (name, type, exchange)
- **Pagination:** Supports cursor-based pagination

#### Ticker Details
- **Endpoint:** `GET /v3/reference/tickers/{ticker}`
- **Status:** ✅ **AVAILABLE** (structure confirmed)
- **Implementation:** Not implemented

#### Stock Exchanges
- **Endpoint:** `GET /v3/reference/exchanges?market=stocks`
- **Status:** ✅ **TESTED & WORKING**
- **Implementation:** ✅ **IMPLEMENTED & WORKING** (via `/api/reference/exchanges` endpoint)
- **Response:** All US stock exchanges with MIC codes and details

#### Market Status
- **Endpoint:** `GET /v1/marketstatus/now`
- **Status:** ✅ **TESTED & WORKING**
- **Implementation:** ✅ **IMPLEMENTED & WORKING** (via `/api/market/status` endpoint)
- **Cache:** In-memory cache with 5-minute TTL for <5ms performance
- **Response:** Current market open/closed status for all asset classes

#### Stock Splits
- **Endpoint:** `GET /v3/reference/splits?ticker={ticker}`
- **Status:** ✅ **TESTED & WORKING**
- **Implementation:** ✅ **IMPLEMENTED & WORKING** (via `/api/reference/splits/{symbol}` endpoint)
- **Example Response:** `{"results":[{"execution_date":"2020-08-31","split_from":1,"split_to":4,"ticker":"AAPL"}]}`

#### Market Holidays
- **Endpoint:** `GET /v1/marketstatus/upcoming`
- **Status:** ✅ **AVAILABLE**
- **Implementation:** Not implemented

#### Condition Codes
- **Endpoint:** `GET /v3/reference/conditions`
- **Status:** ✅ **AVAILABLE**
- **Implementation:** Not implemented

### Fundamentals & News

#### Financial Statements
- **Endpoint:** `GET /vX/reference/financials?ticker={ticker}`
- **Status:** ✅ **TESTED & WORKING**
- **Implementation:** ✅ **IMPLEMENTED & WORKING** (via `/api/financials/{symbol}` endpoint)
- **Response:** Complete financial statements (income, balance sheet, cash flow)
- **Example:** Full AAPL financials with TTM data

#### News with Sentiment Analysis
- **Endpoint:** `GET /v2/reference/news?ticker={ticker}`
- **Status:** ✅ **TESTED & WORKING**
- **Implementation:** ✅ **IMPLEMENTED & WORKING** (via `/api/news?ticker={ticker}&limit={limit}` endpoint)
- **Response:** Recent news articles with AI-generated sentiment analysis per ticker
- **Features:** Publisher info, article URLs, sentiment reasoning

#### Short Interest
- **Endpoint:** `GET /v3/reference/financials/short-interest`
- **Status:** ✅ **AVAILABLE** (not tested)
- **Implementation:** Not implemented

#### Short Volume
- **Endpoint:** `GET /v3/reference/financials/short-volume`
- **Status:** ✅ **AVAILABLE** (not tested)
- **Implementation:** Not implemented

### Corporate Actions

#### IPOs
- **Endpoint:** `GET /v3/reference/ipos`
- **Status:** ✅ **AVAILABLE**
- **Implementation:** Not implemented

#### Dividends
- **Endpoint:** `GET /v3/reference/dividends`
- **Status:** ✅ **AVAILABLE**
- **Implementation:** Not implemented

#### Ticker Events
- **Endpoint:** `GET /v3/reference/ticker-events`
- **Status:** ✅ **AVAILABLE**
- **Implementation:** Not implemented

### WebSocket Streaming

#### Real-time Aggregates (Per-Minute)
- **Endpoint:** `wss://socket.polygon.io/stocks`
- **Status:** ✅ **AVAILABLE**
- **Implementation:** Not implemented
- **Subscription:** `{"action":"subscribe","params":"AM.{ticker}"}`
- **Documentation:** https://polygon.io/docs/websocket/stocks/aggregates-per-minute

#### Real-time Aggregates (Per-Second)
- **Endpoint:** `wss://socket.polygon.io/stocks`
- **Status:** ✅ **AVAILABLE**
- **Implementation:** Not implemented
- **Subscription:** `{"action":"subscribe","params":"A.{ticker}"}`
- **Documentation:** https://polygon.io/docs/websocket/stocks/aggregates-per-second

#### Trades & Quotes
- **Endpoint:** `wss://socket.polygon.io/stocks`
- **Status:** ✅ **AVAILABLE**
- **Implementation:** Not implemented
- **Subscriptions:** `T.{ticker}` for trades, `Q.{ticker}` for quotes
- **Documentation:** https://polygon.io/docs/websocket/stocks/overview

---

## LightSpeed Connect API Endpoints

**Platform:** LightSpeed Connect  
**Protocol:** WebSocket (WSS) exclusively  
**Base URL:** `wss://onboarding.connecttrade.com:28052` (Sandbox)  
**Authentication:** API Key + Client ID + Account ID  
**Message Format:** JSON over WebSocket  
**Current Environment:** Sandbox (Certification Environment)

### Connection & Authentication

#### WebSocket Connection
- **Endpoint:** `wss://onboarding.connecttrade.com:28052` (Sandbox)
- **Production:** `wss://api.lightspeedconnect.com:443` (Contact support for access)
- **Status:** ✅ **IMPLEMENTED & WORKING**
- **Implementation:** `src/broker/lightspeed.rs`
- **Protocol:** WebSocket with JSON messaging
- **Authentication:** Login message with API credentials

#### Session Management
- **Message Type:** `LOGIN` (MsgType: "A")
- **Status:** ✅ **IMPLEMENTED & WORKING**
- **Implementation:** Automatic login on connection
- **Parameters:**
  - `ClientID`: Your assigned client identifier
  - `Account`: Account number for trading
  - `ApiKey`: Authentication key
- **Response:** Session establishment confirmation

### Order Management

#### Place Order
- **Message Type:** `NEW_ORDER_SINGLE` (MsgType: "D")
- **Status:** ✅ **IMPLEMENTED & WORKING** (BUY orders tested)
- **Implementation:** `place_order()` method in `LightspeedBroker`
- **Tested Order Types:**
  - ✅ BUY LIMIT orders
  - ❌ SELL orders (needs testing)
  - ❌ SELL_SHORT orders (needs testing)
- **Parameters:**
  - `Symbol`: Stock ticker
  - `Side`: BUY, SELL, SELL_SHORT
  - `OrderType`: LIMIT, MARKET, STOP
  - `OrderQty`: Quantity as string
  - `Price`: Limit price (for LIMIT orders)
  - `ClientOrderID`: Unique order identifier

#### Order Status Updates
- **Message Type:** `EXECUTION_REPORT` (MsgType: "8")
- **Status:** ✅ **IMPLEMENTED & WORKING**
- **Implementation:** Automatic message handling
- **Response Fields:**
  - `ExecType`: Order execution type
  - `OrdStatus`: Current order status
  - `OrderID`: Broker-assigned order ID
  - `LastPx`: Fill price
  - `CumQty`: Cumulative filled quantity

#### Cancel Order
- **Message Type:** `ORDER_CANCEL_REQUEST` (MsgType: "F")
- **Status:** ❌ **NOT TESTED**
- **Implementation:** `cancel_order()` method exists but untested
- **Parameters:**
  - `ClientOrderID`: Original order ID
  - `OrderID`: Broker order ID

### Position Management

#### Get Current Positions
- **Endpoint:** `GET /api/positions`
- **Status:** ✅ **IMPLEMENTED & WORKING**
- **Implementation:** Direct access to LightSpeed position data
- **Response:** Complete position details including quantities, prices, timestamps
- **Example Response:**
```json
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
      "closed_at": null,
      "status": "Open"
    }
  }
}
```

#### Get Portfolio Summary  
- **Endpoint:** `GET /api/portfolio`
- **Status:** ✅ **IMPLEMENTED & WORKING**
- **Implementation:** Real-time portfolio calculations with LightSpeed data
- **Response:** Total value, available cash, exposure, position count
- **Features:** Live position data, calculated portfolio metrics
- **Example Response:**
```json
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

#### Position Updates (WebSocket)
- **Status:** ✅ **IMPLEMENTED & WORKING**
- **Implementation:** Automatic position tracking from LightSpeed execution reports
- **Data Flow:** WebSocket → Memory → REST API
- **Update Frequency:** Real-time via WebSocket message handling

### Market Data (via Third-Party Integration)

**Note:** LightSpeed Connect recommends integrating with third-party market data providers (Databento, Polygon) rather than providing direct market data feeds.

#### Supported Integration Partners
- **Polygon.io** ✅ **CURRENTLY INTEGRATED**
- **Databento** ❌ **NOT INTEGRATED**
- **Other providers** available on request

### Advanced Order Types

#### Bracket Orders
- **Status:** ✅ **AVAILABLE** (not implemented)
- **Implementation:** Requires multi-leg order structure
- **Features:** Parent order with profit target and stop-loss

#### One-Cancels-All (OCA) Orders
- **Status:** ✅ **AVAILABLE** (not implemented)
- **Implementation:** Group multiple orders with cancellation logic

#### Multi-leg Options
- **Status:** ✅ **AVAILABLE** (not implemented)
- **Implementation:** Complex option strategies
- **Parameters:** Multiple legs with different strikes/expirations

### Error Handling & Events

#### Error Messages
- **Status:** ✅ **IMPLEMENTED**
- **Implementation:** `handle_error()` method
- **Error Fields:**
  - `ErrorCode`: Numeric error code
  - `ErrorText`: Human-readable error description

#### Connection Events
- **Status:** ✅ **IMPLEMENTED**
- **Implementation:** `BrokerEvent` enum for connection status
- **Events:** Connected, Disconnected, Error, OrderFilled, etc.

### Development & Testing

#### Certification Environment
- **Status:** ✅ **CURRENTLY IN USE**
- **URL:** `wss://onboarding.connecttrade.com:28052`
- **Features:** Full simulation environment for testing
- **Supported:** Test orders, realistic execution scenarios, error testing

#### Test Symbols - Certification Mode
**⚠️ IMPORTANT:** The LightSpeed certification environment only accepts specific test symbols that exhibit predefined behaviors to replicate real-world trading scenarios. **Only use these symbols for testing.**

| Test Symbol | Behavior Description |
|-------------|---------------------|
| **AMZN** | Order **partially fills**; balance of order remains open |
| **CHWY** | Order receives **partial fills until completed** |
| **F** | Order receives **partial fills until completed** |
| **GE** | Order receives **partial fills until completed** |
| **GOOGL** | Order **fills immediately** |
| **MSFT** | Order is **rejected** |
| **TSLA** | Order **does not fill** (remains open) |
| **ORCL** | Order **Cancel** scenario testing |
| **BAC** | Order **Replace** scenario (increase quantity) |

**Testing Strategy:**
- Use **GOOGL** for immediate fill testing
- Use **AMZN** for partial fill handling  
- Use **CHWY, F, GE** for multiple partial fills
- Use **MSFT** for rejection handling
- Use **TSLA** for unfilled order management
- Use **ORCL** for cancel functionality
- Use **BAC** for order modification testing

**Documentation Reference:** https://lightspeed-connect.connecttrade.com/developers/documentation/getting-started-guide#using-the-api-console-for-order-testing

#### Production Environment
- **Status:** ❌ **NOT ACTIVATED**
- **Requirements:** Complete testing in certification environment
- **Process:** Contact LightSpeed support to activate live trading
- **URL:** `wss://api.lightspeedconnect.com:443` (estimated)

### Documentation & Support

#### Getting Started Guide
- **Status:** ✅ **REFERENCED**
- **Access:** Available through LightSpeed Connect portal
- **Content:** API setup, sample code, integration examples

#### Sample Code & WS_Adapter
- **Status:** ❌ **NOT IMPLEMENTED**
- **Available:** Pre-built WebSocket adapter for easier integration
- **Features:** Session management, reconnection logic, event handling

#### Developer Support
- **Contact:** LightSpeed Connect support team
- **Resources:** Technical documentation, troubleshooting, live environment activation

---

