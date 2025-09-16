# Full Lightspeed API Buildout Plan - Priority Based

**Project Version:** v9.14.25
**Last Updated:** 2025-09-16
**Purpose:** Complete implementation of Lightspeed Connect API for professional trading interface

## Prerequisites
- Existing WebSocket connection and basic order placement working
- Current position tracking and event system functional

## Priority 1: Complete Order Type Support (CRITICAL)
**Must be completed before UI work**

### 1.1. Advanced Order Fields Implementation
- TimeInForce (DAY, GTC, IOC, FOK)
- StopPrice for stop orders
- TrailingAmount and TrailingPercent for trailing stops
- MinQty, DisplaySize, PegOffsetValue, ExecInst

### 1.2. All Order Types Functional
- STOP, STOP_LIMIT orders
- TRAILING_STOP, TRAILING_STOP_LIMIT orders
- ICEBERG orders (with DisplaySize)
- PEGGED orders (PegOffsetValue/Type)
- Market/Limit ON_OPEN and ON_CLOSE orders

### 1.3. Order Creation Helper Functions
- Factory methods for each order type
- Validation logic for order parameters
- Default value handling

## Priority 2: Complete Order Lifecycle Management (CRITICAL)
**Depends on Priority 1 completion**

### 2.1. Order Modification System
- OrderCancelReplace message implementation
- Modify quantity, price, stop price, time in force
- Enhanced order state tracking

### 2.2. Order Status & Tracking
- OrderStatusRequest implementation
- Request all open orders functionality
- Order history retrieval
- Enhanced order states (PENDING_REPLACE, EXPIRED, SUSPENDED)

### 2.3. Mass Order Operations
- Cancel all orders functionality
- Cancel by symbol or side
- Mass order status requests

## Priority 3: Real-time Account Data Architecture (HIGH)
**Can be developed in parallel with Priority 2**

### 3.1. Account Data Structures
- Complete AccountSummary with buying power, margin data
- Real-time PositionSummary with P&L calculations
- Enhanced position tracking with market values

### 3.2. Non-blocking Data Refresh System
- Multi-second refresh rates (4Hz for account, 10Hz for orders)
- Concurrent tokio task management
- Memory-first caching with <1ms access times

### 3.3. Account Message Handling
- AccountDataResponse processing
- PositionReport handling
- BuyingPower update events
- Margin and collateral reporting

## Priority 4: Enhanced WebSocket Message Coverage (HIGH)
**Prerequisite for Priority 3, can start early**

### 4.1. New Message Type Handlers
- OrderCancelReject, OrderCancelReplace responses
- AccountDataResponse, PositionReport
- TradingSessionStatus, SecurityListResponse
- MassCancelReport, CollateralReport

### 4.2. Enhanced BrokerEvent System
- AccountUpdate, OrderModified, OrderExpired events
- PositionUpdate with market values
- BuyingPowerUpdate, MarginCall events
- TradingSessionStatus events

## Priority 5: Complete Backend API Suite (MEDIUM)
**Depends on Priorities 1-4 completion**

### 5.1. Order Management Endpoints
- Pending orders, completed orders, order history APIs
- Order modification and mass cancel endpoints
- Order session management for 4-ticker interface

### 5.2. Account Management Endpoints
- Account summary, buying power APIs
- Real-time account data streaming endpoints
- Position summary and streaming APIs

### 5.3. Supporting Endpoints
- Available exchange routes
- Security list and trading session status
- Order validation and pre-trade checks

## Priority 6: Polygon Market Data Integration (MEDIUM)
**Can be developed in parallel with Priority 5**

### 6.1. Real-time Price Data for P&L Calculations
- Integrate existing Polygon WebSocket connection for real-time quotes
- 100ms price updates for position P&L calculations
- Memory cache for last known prices per symbol
- Fallback to cached prices if WebSocket disconnects

### 6.2. Level 2 Market Data Integration (Future)
- Polygon Level 2 API integration (when available)
- Bid/ask ladder with depth display
- Replace placeholder components in 4-ticker sections
- Real-time order book updates

### 6.3. Time & Sales Integration (Future)
- Polygon Time & Sales API integration (when available)
- Real-time trade execution feed
- Replace placeholder components in center and right panels
- Trade volume and price trend analysis

### 6.4. Market Data Architecture
- Non-blocking Polygon data streams
- Separate WebSocket connection for market data
- Memory-first caching with <1ms quote access
- Rate limiting and connection management
- Error handling and reconnection logic

## Priority 7: Professional Trading Interface (MEDIUM)
**Depends on Priorities 1-5 completion**

### 7.1. Professional Screen Layout Implementation
**New Route:** `/broker-pro` - Professional trading interface

**Layout Structure (3-Panel Design):**
```
┌─────────────────┬──────────────────────────────────────┬─────────────────┐
│ LEFT PANEL      │ CENTER PANEL                         │ RIGHT PANEL     │
│                 │                                      │                 │
│ Combined        │ 4-Ticker Order Entry Grid            │ Extended        │
│ Positions       │ ┌─────────┬─────────┐               │ Time & Sales    │
│ (All + Today)   │ │ Ticker1 │ Ticker2 │               │                 │
│                 │ │ Level2* │ Level2* │               │ (Corresponds to │
│ Active/Pending  │ │ T&S*    │ T&S*    │               │ active ticker   │
│ Orders          │ │ Entry   │ Entry   │               │ in center)      │
│                 │ └─────────┴─────────┘               │                 │
│ Executed &      │ ┌─────────┬─────────┐               │                 │
│ Cancelled       │ │ Ticker3 │ Ticker4 │               │                 │
│ Orders          │ │ Level2* │ Level2* │               │                 │
│                 │ │ T&S*    │ T&S*    │               │                 │
│ Messages        │ │ Entry   │ Entry   │               │                 │
│                 │ └─────────┴─────────┘               │                 │
└─────────────────┴──────────────────────────────────────┴─────────────────┘
```
*Level 2 and Time & Sales are placeholders for future Polygon integration

### 7.2. Left Panel Components

**Combined Positions Section:**
- Merge "All open positions" and "Open positions today" into single view
- Columns: Symbol, Shares, Avg Cost, Current Price, P&L, % Change
- Real-time P&L updates (100ms refresh cycle, Polygon data when available)
- Color coding: Green (profit), Red (loss), Blue (flat)

**Active/Pending Orders Section:**
- Display orders with status: NEW, PARTIALLY_FILLED
- Columns: Symbol, Side, Type, Qty, Filled, Remaining, Price, Status, Time
- Real-time updates via WebSocket events
- Partial fill progress indicators

**Executed & Cancelled Orders Section:**
- Display completed orders: FILLED, CANCELED, REJECTED, EXPIRED
- Columns: Symbol, Side, Type, Qty, Price, Status, Time, Reason
- Filter by time period (Today, Week, Month)
- Order history scrollable list

**Messages Section:**
- Real-time broker events and notifications
- Types: Connection status, order confirmations, errors, warnings
- Timestamp and message categorization
- Auto-scroll with manual scroll override

### 7.3. Center Panel - 4-Ticker Order Entry Grid

**Individual Order Entry Sections (4 independent):**
- Each section operates on its own ticker symbol
- Ticker symbol input with validation
- Exchange routing dropdown (SMART, NASDAQ, NYSE, ARCA, BATS, EDGX, IEX)

**Order Entry Form Components:**
- Order Type dropdown: MARKET, LIMIT, STOP, STOP_LIMIT, TRAILING_STOP, ICEBERG, PEGGED
- Side selection: BUY, SELL, SELL_SHORT
- Quantity input with validation
- Price fields (dynamic based on order type):
  - Limit Price (LIMIT, STOP_LIMIT)
  - Stop Price (STOP, STOP_LIMIT, TRAILING_STOP)
  - Trailing Amount/Percent (TRAILING_STOP)
  - Display Size (ICEBERG)
- Time in Force: DAY, GTC, IOC, FOK
- Special Instructions: ALL_OR_NONE, FILL_OR_KILL, etc.

**Level 2 Market Data Placeholders:**
- Mock bid/ask ladder display
- "Level 2 data coming soon" messaging
- Reserved space for future Polygon integration

**Time & Sales Placeholders:**
- Mock recent trades table
- "Time & Sales coming soon" messaging
- Reserved space for future Polygon integration

### 7.4. Right Panel - Extended Time & Sales

**Extended Trade History:**
- Corresponds to currently selected/active ticker from center panel
- Columns: Time, Price, Size, Side (Buy/Sell indicator)
- Scrollable history with real-time updates
- Placeholder implementation until Polygon integration

### 7.5. Advanced Order Entry Forms
- Dynamic form fields based on order type selection
- Real-time order validation and confirmation
- Exchange routing selection with smart routing default
- Order preview and confirmation dialogs
- Keyboard shortcuts for power users

### 7.6. Real-time Data Integration
- Server-Sent Events for all data streams
- High-frequency UI updates (250ms account, 100ms orders)
- Non-blocking user interface with loading indicators
- WebSocket reconnection handling
- Error state management and user notifications

## Priority 8: Advanced Order Features (LOW)
**Optional enhancements after core functionality**

### 8.1. Complex Order Types
- Bracket orders (parent/child relationships)
- Conditional orders with trigger logic
- One-cancels-other (OCO) orders

### 8.2. Intelligent Routing
- Best execution algorithms
- Exchange selection logic
- Route performance tracking

## Performance Requirements
- Order placement: <50ms end-to-end
- Data access: <1ms from memory cache
- Account data refresh: 4Hz (250ms intervals)
- Order status refresh: 10Hz (100ms intervals)
- Memory-first architecture with no trading-time database reads

## Success Criteria
- All advanced order types placing successfully
- Real-time account data updating multiple times per second
- Professional trading interface operational
- Complete order lifecycle management functional
- Memory-first architecture maintaining <1ms data access

## Polygon Integration Timeline
- **Immediate (Priority 6.1):** Real-time price data for P&L calculations using existing WebSocket
- **Phase 2 (Priority 6.2-6.3):** Level 2 and Time & Sales when Polygon APIs become available
- **Architecture:** Separate WebSocket connection for market data, independent of Lightspeed
- **Performance:** 100ms price update cycle, memory-cached quotes with <1ms access

## Implementation Notes
- Focus on order execution functionality first (Priorities 1-5)
- Polygon price data integration for real-time P&L (Priority 6.1)
- Level 2 and Time & Sales placeholders until Polygon APIs available (Priority 6.2-6.3)
- Risk management implementation deferred (future)
- All data streams must be non-blocking and concurrent
- Professional interface without emojis, business-focused design

---
**Related Files:** `CLAUDE.md`, `STATUS.md`, `README.md`