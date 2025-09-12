// src/web/mod.rs
use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    response::{Html, Json},
    routing::{delete, get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashMap;
use crate::engine::TradingEngine;
use crate::data::{SymbolDataResponse, MarketMoverData, MarketSnapshotTicker, DailyMarketSummaryTicker};
use crate::types::{Position, RiskParameters};
// use crate::broker::paper::{PaperSession, PaperTrade};  // TODO: Enable when paper broker implemented

mod rules;

#[derive(Deserialize)]
struct SymbolRequest {
    symbol: String,
}

#[derive(Serialize)]
struct SymbolResponse {
    success: bool,
    data: Option<SymbolDataResponse>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct OrderRequest {
    symbol: String,
    side: String, // BUY, SELL, SELL_SHORT
    order_type: String, // MARKET, LIMIT
    quantity: u64,
    price: Option<f64>,
}

#[derive(Serialize)]
struct OrderResponse {
    success: bool,
    order_id: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct StatusResponse {
    broker_connected: bool,
    session_active: bool,
    message: String,
}

#[derive(Deserialize)]
struct IndicatorQuery {
    window: Option<u32>,
    timespan: Option<String>,
    timestamp: Option<String>,
    short_window: Option<u32>,
    long_window: Option<u32>,
    signal_window: Option<u32>,
}

#[derive(Serialize)]
struct IndicatorResponse {
    success: bool,
    value: Option<f64>,
    data: Option<serde_json::Value>,
    error: Option<String>,
}

#[derive(Serialize)]
struct MarketMoversResponse {
    success: bool,
    data: Option<Vec<MarketMoverData>>,
    error: Option<String>,
}

#[derive(Serialize)]
struct MarketStatusResponse {
    success: bool,
    status: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct SystemModeResponse {
    success: bool,
    mode: Option<String>,
    description: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct SystemVersionResponse {
    success: bool,
    version: Option<String>,
    branch: Option<String>,
    commit: Option<String>,
    data_delay_minutes: Option<u32>,
    polygon_mode: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct NewsResponse {
    success: bool,
    data: Option<serde_json::Value>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct NewsQuery {
    limit: Option<u32>,
}

#[derive(Serialize)]
struct CacheStatsResponse {
    success: bool,
    entries: Option<usize>,
    message: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct FullMarketSnapshotResponse {
    success: bool,
    count: Option<usize>,
    data: Option<Vec<MarketSnapshotTicker>>,
    error: Option<String>,
}

#[derive(Serialize)]
struct DailyMarketSummaryResponse {
    success: bool,
    count: Option<usize>,
    date: Option<String>,
    data: Option<Vec<DailyMarketSummaryTicker>>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct DateQuery {
    date: Option<String>,
}

#[derive(Deserialize)]
struct SubscribeRequest {
    symbols: Vec<String>,
}

#[derive(Serialize)]
struct WebSocketStatusResponse {
    success: bool,
    websocket_enabled: bool,
    status: Option<String>,
    url: Option<String>,
    subscriptions_count: Option<usize>,
    error: Option<String>,
}

#[derive(Serialize)]
struct WebSocketSubscribeResponse {
    success: bool,
    status: Option<String>,
    symbols: Option<Vec<String>>,
    error: Option<String>,
}

#[derive(Serialize)]
struct WebSocketSubscriptionsResponse {
    success: bool,
    subscriptions: Option<Vec<String>>,
    error: Option<String>,
}

#[derive(Serialize)]
struct RiskConfigResponse {
    success: bool,
    data: Option<RiskParameters>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct UpdateRiskRequest {
    max_positions: u32,
    max_portfolio_exposure_percent: f64,
    max_single_position_percent: f64,
    default_stop_loss_percent: f64,
    max_loss_per_trade_dollars: Option<f64>,
    max_daily_loss_dollars: Option<f64>,
    require_volume_confirmation: bool,
    min_volume_ratio: f64,
}

#[derive(Serialize)]
struct UpdateRiskResponse {
    success: bool,
    message: Option<String>,
    error: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct WatchlistSymbol {
    id: i64,
    symbol: String,
    description: Option<String>,
    active: bool,
    added_at: i64,
    updated_at: i64,
}

#[derive(Serialize)]
struct WatchlistResponse {
    success: bool,
    data: Option<Vec<WatchlistSymbol>>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct AddSymbolRequest {
    symbol: String,
    description: Option<String>,
}

#[derive(Serialize)]
struct AddSymbolResponse {
    success: bool,
    message: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct DeleteSymbolResponse {
    success: bool,
    message: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct ResetDatabaseResponse {
    success: bool,
    message: Option<String>,
    error: Option<String>,
}

// Paper Trading API Data Structures
// #[derive(Serialize)]
// struct PaperSessionResponse {
//     success: bool,
//     data: Option<PaperSession>,
//     error: Option<String>,
// }

// #[derive(Serialize)]
// struct PaperSessionsResponse {
//     success: bool,
//     data: Option<Vec<PaperSession>>,
//     error: Option<String>,
// }

// #[derive(Deserialize)]
// struct CreatePaperSessionRequest {
//     name: String,
//     description: Option<String>,
//     initial_cash: Option<f64>,
// }

// #[derive(Serialize)]
// struct CreatePaperSessionResponse {
//     success: bool,
//     session_id: Option<String>,
//     message: Option<String>,
//     error: Option<String>,
// }

// #[derive(Serialize)]
// struct PaperTradesResponse {
//     success: bool,
//     data: Option<Vec<PaperTrade>>,
//     total_count: Option<usize>,
//     error: Option<String>,
// }

#[derive(Deserialize)]
struct ExecuteTradeRequest {
    symbol: String,
    side: String,  // BUY or SELL
    quantity: u32,
    rule_id: Option<String>,
    rule_name: Option<String>,
}

#[derive(Serialize)]
struct ExecuteTradeResponse {
    success: bool,
    trade_id: Option<String>,
    execution_price: Option<f64>,
    commission: Option<f64>,
    message: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct ClosePositionResponse {
    success: bool,
    pnl: Option<f64>,
    exit_price: Option<f64>,
    message: Option<String>,
    error: Option<String>,
}


pub async fn start_server(bind_address: String, engine: Arc<TradingEngine>) -> Result<()> {
    let app = Router::new()
        .route("/", get(root))
        .route("/rules", get(rules_page))
        .route("/health", get(health))
        .route("/api/status", get(get_status))
        .route("/api/symbol", post(lookup_symbol))
        .route("/api/order", post(place_order))
        // Technical Indicators
        .route("/api/indicators/sma/:symbol", get(get_sma))
        .route("/api/indicators/ema/:symbol", get(get_ema))
        .route("/api/indicators/rsi/:symbol", get(get_rsi))
        .route("/api/indicators/macd/:symbol", get(get_macd))
        // Market Data
        .route("/api/snapshot/:symbol", get(get_snapshot))
        .route("/api/market/movers/gainers", get(get_market_gainers))
        .route("/api/market/movers/losers", get(get_market_losers))
        .route("/api/market/status", get(get_market_status))
        .route("/api/system/mode", get(get_system_mode))
        .route("/api/system/version", get(get_system_version))
        .route("/api/market/previous/:symbol", get(get_previous_day))
        .route("/api/market/minute/:symbol", get(get_minute_aggregates))
        .route("/api/market/snapshot/full", get(get_full_market_snapshot))
        .route("/api/market/summary", get(get_daily_market_summary))
        // Reference Data
        .route("/api/reference/tickers", get(get_tickers))
        .route("/api/reference/exchanges", get(get_exchanges))
        .route("/api/reference/splits/:symbol", get(get_splits))
        // News & Fundamentals
        .route("/api/news", get(get_news))
        .route("/api/financials/:symbol", get(get_financials))
        // Cache Management
        .route("/api/cache/stats", get(get_cache_stats))
        .route("/api/cache/clean", post(clean_cache))
        .route("/api/reset-database", post(reset_database))
        // WebSocket Management
        .route("/api/websocket/status", get(get_websocket_status))
        .route("/api/websocket/subscribe", post(websocket_subscribe))
        .route("/api/websocket/unsubscribe", post(websocket_unsubscribe))
        .route("/api/websocket/subscriptions", get(get_websocket_subscriptions))
        
        // Portfolio & Position endpoints
        .route("/api/positions", get(get_positions))
        .route("/api/portfolio", get(get_portfolio))
        // Risk Configuration endpoints
        .route("/api/risk", get(get_risk_config))
        .route("/api/risk", post(update_risk_config))
        // Symbol Watchlist endpoints
        .route("/api/symbols", get(get_watchlist_symbols))
        .route("/api/symbols", post(add_watchlist_symbol))
        .route("/api/symbols/:symbol", delete(delete_watchlist_symbol))
        .route("/api/symbols/:symbol/activate", post(activate_watchlist_symbol))
        .route("/api/symbols/:symbol/deactivate", post(deactivate_watchlist_symbol))
        // Rules Engine endpoints
        .route("/api/rules", get(rules::get_rules))
        .route("/api/rules", post(rules::create_rule))
        .route("/api/rules/:rule_id", delete(rules::delete_rule))
        .route("/api/rules/engine/start", post(rules::start_rules_engine_handler))
        .route("/api/rules/engine/stop", post(rules::stop_rules_engine_handler))
        .route("/api/rules/engine/status", get(rules::get_rules_engine_status))
        
        // Paper Trading endpoints (TODO: Implement handler functions)
        // .route("/api/paper/sessions", get(get_paper_sessions))
        // .route("/api/paper/sessions", post(create_paper_session))
        // .route("/api/paper/sessions/:session_id", get(get_paper_session))
        // .route("/api/paper/sessions/:session_id/trades", get(get_paper_trades))
        // .route("/api/paper/trade", post(execute_paper_trade))
        // .route("/api/paper/positions/:symbol/close", post(close_paper_position))
        .with_state(engine);

    let listener = tokio::net::TcpListener::bind(&bind_address).await?;
    
    tracing::info!("Web server listening on {}", bind_address);
    
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn root() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html>
<html>
<head>
    <title>📊 Paper Trading Dashboard</title>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style>
        * { box-sizing: border-box; margin: 0; padding: 0; }
        body { 
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; 
            background: #f5f7fa; 
            color: #2c3e50;
            line-height: 1.6;
        }
        .header { 
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); 
            color: white; 
            padding: 1rem 2rem; 
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
        }
        .header h1 { font-size: 1.8rem; margin-bottom: 0.5rem; }
        .header .subtitle { opacity: 0.9; font-size: 0.9rem; }
        .container { 
            max-width: 1400px; 
            margin: 0 auto; 
            padding: 2rem; 
            display: grid; 
            grid-template-columns: 1fr 1fr;
            gap: 2rem;
        }
        .card { 
            background: white; 
            border-radius: 12px; 
            padding: 1.5rem; 
            box-shadow: 0 2px 10px rgba(0,0,0,0.08);
            border: 1px solid #e1e8ed;
        }
        .card h3 { 
            color: #2c3e50; 
            margin-bottom: 1rem; 
            padding-bottom: 0.5rem;
            border-bottom: 2px solid #ecf0f1;
            font-size: 1.1rem;
        }
        .portfolio-summary { grid-column: 1 / -1; }
        .portfolio-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 1rem;
            margin-bottom: 1rem;
        }
        .metric-card {
            background: linear-gradient(135deg, #74b9ff 0%, #0984e3 100%);
            color: white;
            padding: 1rem;
            border-radius: 8px;
            text-align: center;
        }
        .metric-card.cash { background: linear-gradient(135deg, #00b894 0%, #00a085 100%); }
        .metric-card.exposure { background: linear-gradient(135deg, #fdcb6e 0%, #e17055 100%); }
        .metric-card.positions { background: linear-gradient(135deg, #a29bfe 0%, #6c5ce7 100%); }
        .metric-value { font-size: 1.5rem; font-weight: bold; }
        .metric-label { font-size: 0.8rem; opacity: 0.9; }
        .status-indicator {
            display: inline-block;
            width: 10px;
            height: 10px;
            border-radius: 50%;
            margin-right: 8px;
        }
        .status-online { background: #00b894; }
        .status-offline { background: #e17055; }
        .form-grid {
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: 1rem;
            margin-bottom: 1rem;
        }
        .form-group {
            margin-bottom: 1rem;
        }
        .form-group.full-width { grid-column: 1 / -1; }
        label {
            display: block;
            margin-bottom: 0.5rem;
            font-weight: 600;
            color: #2c3e50;
            font-size: 0.9rem;
        }
        input, select {
            width: 100%;
            padding: 0.75rem;
            border: 2px solid #ecf0f1;
            border-radius: 8px;
            font-size: 0.9rem;
            transition: all 0.3s ease;
        }
        input:focus, select:focus {
            outline: none;
            border-color: #74b9ff;
            box-shadow: 0 0 0 3px rgba(116, 185, 255, 0.1);
        }
        button {
            background: linear-gradient(135deg, #74b9ff 0%, #0984e3 100%);
            color: white;
            padding: 0.75rem 1.5rem;
            border: none;
            border-radius: 8px;
            font-size: 0.9rem;
            font-weight: 600;
            cursor: pointer;
            transition: all 0.3s ease;
            box-shadow: 0 2px 5px rgba(116, 185, 255, 0.3);
        }
        button:hover {
            transform: translateY(-2px);
            box-shadow: 0 4px 12px rgba(116, 185, 255, 0.4);
        }
        button.secondary {
            background: linear-gradient(135deg, #a29bfe 0%, #6c5ce7 100%);
        }
        button.success {
            background: linear-gradient(135deg, #00b894 0%, #00a085 100%);
        }
        .result {
            margin-top: 1rem;
            padding: 1rem;
            border-radius: 8px;
            font-weight: 500;
        }
        .success {
            background: #d1f2eb;
            color: #00a085;
            border-left: 4px solid #00b894;
        }
        .error {
            background: #fadbd8;
            color: #e74c3c;
            border-left: 4px solid #e17055;
        }
        .positions-table {
            width: 100%;
            border-collapse: collapse;
            margin-top: 1rem;
        }
        .positions-table th,
        .positions-table td {
            padding: 0.75rem;
            text-align: left;
            border-bottom: 1px solid #ecf0f1;
        }
        .positions-table th {
            background: #f8f9fa;
            font-weight: 600;
            color: #2c3e50;
        }
        .market-data {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
            gap: 0.5rem;
            margin-bottom: 1rem;
        }
        .market-item {
            background: #f8f9fa;
            padding: 0.75rem;
            border-radius: 6px;
            text-align: center;
            font-size: 0.85rem;
        }
        .market-item .symbol { font-weight: bold; color: #2c3e50; }
        .market-item .price { color: #74b9ff; font-weight: 600; }
        .watchlist {
            max-height: 300px;
            overflow-y: auto;
        }
        .watchlist-item {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 0.5rem;
            margin-bottom: 0.5rem;
            background: #f8f9fa;
            border-radius: 6px;
        }
        .tab-container {
            margin-bottom: 1rem;
        }
        .tabs {
            display: flex;
            border-bottom: 1px solid #ecf0f1;
            margin-bottom: 1rem;
        }
        .tab {
            padding: 0.75rem 1rem;
            cursor: pointer;
            border-bottom: 2px solid transparent;
            transition: all 0.3s ease;
            color: #74787e;
        }
        .tab.active {
            color: #74b9ff;
            border-bottom-color: #74b9ff;
        }
        .tab-content {
            display: none;
        }
        .tab-content.active {
            display: block;
        }
        @media (max-width: 768px) {
            .container { 
                grid-template-columns: 1fr; 
                padding: 1rem;
            }
            .portfolio-grid {
                grid-template-columns: 1fr 1fr;
            }
            .form-grid {
                grid-template-columns: 1fr;
            }
        }
        .loading {
            text-align: center;
            padding: 2rem;
            color: #74787e;
        }
        .refresh-btn {
            float: right;
            font-size: 0.8rem;
            padding: 0.5rem 1rem;
        }
        .status-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
            gap: 1rem;
            margin: 1rem 0;
        }
        .status-item {
            background: #f8f9fa;
            padding: 0.75rem;
            border-radius: 6px;
            border-left: 3px solid #74b9ff;
        }
        .status-item .success {
            color: #00b894;
            font-weight: bold;
        }
        .status-item .error {
            color: #e17055;
            font-weight: bold;
        }
        .form-section {
            background: #f8f9fa;
            padding: 1rem;
            border-radius: 8px;
            border: 1px solid #e1e8ed;
        }
        .form-section h4 {
            margin-bottom: 1rem;
            color: #2c3e50;
        }
        .rule-item {
            background: #f8f9fa;
            border: 1px solid #e1e8ed;
            border-radius: 8px;
            padding: 1rem;
            margin-bottom: 0.5rem;
        }
        .rule-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 0.5rem;
        }
        .rule-status.active {
            color: #00b894;
        }
        .rule-status.inactive {
            color: #74787e;
        }
        .rule-details {
            font-size: 0.9rem;
            color: #74787e;
            margin-bottom: 0.5rem;
        }
        .delete-btn {
            background: #e17055;
            color: white;
            border: none;
            padding: 0.25rem 0.5rem;
            border-radius: 4px;
            font-size: 0.8rem;
            cursor: pointer;
        }
        .delete-btn:hover {
            background: #d63031;
        }
        .no-rules {
            text-align: center;
            padding: 2rem;
            color: #74787e;
            font-style: italic;
        }
    </style>
</head>
<body>
    <div class="header">
        <div style="display: flex; justify-content: space-between; align-items: flex-start;">
            <div>
                <h1>📊 Paper Trading Dashboard</h1>
                <div class="subtitle">
                    <span class="status-indicator status-online" id="statusDot"></span>
                    <span id="statusText">System Online • Paper Trading Mode</span>
                    <div style="margin-top: 0.5rem;">
                        <a href="/" style="color: white; text-decoration: none; margin-right: 1rem; padding: 0.25rem 0.75rem; background: rgba(255,255,255,0.1); border-radius: 4px;">📊 Dashboard</a>
                        <a href="/rules" style="color: white; text-decoration: none; padding: 0.25rem 0.75rem; background: rgba(255,255,255,0.1); border-radius: 4px;">🤖 Rules Engine</a>
                    </div>
                </div>
            </div>
            <div style="text-align: right; font-size: 0.75rem; opacity: 0.8; line-height: 1.2;">
                <div style="margin-bottom: 0.5rem;">
                    <button id="resetDbBtn" style="
                        background: rgba(255,255,255,0.1); 
                        border: 1px solid rgba(255,255,255,0.2); 
                        color: #ff6b6b; 
                        padding: 0.25rem 0.5rem; 
                        font-size: 0.7rem; 
                        border-radius: 4px; 
                        cursor: pointer;
                        opacity: 0.7;
                        transition: opacity 0.2s;
                    " 
                    onmouseover="this.style.opacity='1'" 
                    onmouseout="this.style.opacity='0.7'"
                    onclick="resetDatabase()" 
                    title="Reset paper trading database to defaults">
                        🔄 Reset DB
                    </button>
                </div>
                <div id="systemVersion">v9.2.25.1-945055e</div>
                <div id="systemClock" style="font-family: monospace; font-weight: bold;">--:--:-- EST</div>
                <div id="dataDelay" style="font-size: 0.65rem; opacity: 0.7;">15min delayed</div>
            </div>
        </div>
    </div>

    <div class="container">
        <!-- Portfolio Summary -->
        <div class="card portfolio-summary">
            <h3>💼 Portfolio Overview
                <button class="refresh-btn secondary" onclick="refreshPortfolio()">Refresh</button>
            </h3>
            <div class="portfolio-grid">
                <div class="metric-card">
                    <div class="metric-value" id="totalValue">$50,000</div>
                    <div class="metric-label">Total Value</div>
                </div>
                <div class="metric-card cash">
                    <div class="metric-value" id="availableCash">$50,000</div>
                    <div class="metric-label">Available Cash</div>
                </div>
                <div class="metric-card exposure">
                    <div class="metric-value" id="totalExposure">$0</div>
                    <div class="metric-label">Total Exposure</div>
                </div>
                <div class="metric-card positions">
                    <div class="metric-value" id="positionCount">0</div>
                    <div class="metric-label">Open Positions</div>
                </div>
            </div>
            
            <!-- Market Status -->
            <div class="market-data" id="marketData">
                <div class="market-item">
                    <div class="symbol">Market</div>
                    <div class="price" id="marketStatus">CLOSED</div>
                </div>
                <div class="market-item">
                    <div class="symbol">WebSocket</div>
                    <div class="price" id="wsStatus">Connected</div>
                </div>
                <div class="market-item">
                    <div class="symbol">Subscriptions</div>
                    <div class="price" id="wsSubscriptions">5</div>
                </div>
            </div>
        </div>

        <!-- Trading Interface -->
        <div class="card">
            <h3>🎯 Paper Trading</h3>
            <form onsubmit="placeOrder(event)">
                <div class="form-grid">
                    <div class="form-group">
                        <label>Symbol:</label>
                        <input type="text" id="symbol" value="NVDA" required placeholder="e.g., NVDA, TSLA">
                    </div>
                    <div class="form-group">
                        <label>Side:</label>
                        <select id="side" required>
                            <option value="BUY">BUY</option>
                            <option value="SELL">SELL</option>
                        </select>
                    </div>
                    <div class="form-group">
                        <label>Order Type:</label>
                        <select id="orderType" required onchange="togglePrice()">
                            <option value="MARKET">MARKET</option>
                            <option value="LIMIT">LIMIT</option>
                        </select>
                    </div>
                    <div class="form-group">
                        <label>Quantity:</label>
                        <input type="number" id="quantity" value="10" min="1" required>
                    </div>
                    <div class="form-group" id="priceGroup" style="display: none;">
                        <label>Limit Price:</label>
                        <input type="number" id="price" value="150.00" step="0.01" min="0">
                    </div>
                </div>
                <button type="submit" class="success">Place Paper Trade</button>
                <button type="button" class="secondary" onclick="getQuote()">Get Quote</button>
            </form>
            <div id="result"></div>
        </div>

        <!-- Positions & Watchlist -->
        <div class="card">
            <div class="tab-container">
                <div class="tabs">
                    <div class="tab active" onclick="switchTab('positions')">📈 Positions</div>
                    <div class="tab" onclick="switchTab('watchlist')">👁️ Watchlist</div>
                </div>
                
                <div id="positions" class="tab-content active">
                    <div id="positionsLoading" class="loading">Loading positions...</div>
                    <div id="positionsContent" style="display: none;">
                        <table class="positions-table">
                            <thead>
                                <tr>
                                    <th>Symbol</th>
                                    <th>Position Size</th>
                                    <th>Total Cost</th>
                                    <th>Open P&L</th>
                                    <th>Realized P&L</th>
                                </tr>
                            </thead>
                            <tbody id="positionsBody">
                            </tbody>
                        </table>
                        <div id="noPositions" style="text-align: center; padding: 2rem; color: #74787e;">
                            No open positions
                        </div>
                    </div>
                </div>
                
                <div id="watchlist" class="tab-content">
                    <div class="watchlist" id="watchlistContent">
                        <div class="loading">Loading watchlist...</div>
                    </div>
                </div>
            </div>
        </div>

        <!-- Market Data & Indicators -->
        <div class="card">
            <h3>📊 Market Indicators</h3>
            <div class="form-group">
                <label>Symbol for Analysis:</label>
                <input type="text" id="indicatorSymbol" value="NVDA" placeholder="Enter symbol">
                <button type="button" onclick="loadIndicators()" style="margin-top: 0.5rem;">Load Indicators</button>
            </div>
            <div id="indicatorResults">
                <div class="loading">Click "Load Indicators" to view technical analysis</div>
            </div>
        </div>

        <!-- Rules Engine Quick Access -->
        <div class="card">
            <h3>🤖 Rules Engine</h3>
            <div style="text-align: center; padding: 2rem;">
                <div style="margin-bottom: 1rem;">
                    <strong>Automated Trading Rules</strong>
                    <div style="color: #74787e; font-size: 0.9rem;">Create and manage your trading automation</div>
                </div>
                <a href="/rules" style="display: inline-block; background: #74b9ff; color: white; padding: 0.75rem 2rem; border-radius: 6px; text-decoration: none; transition: background 0.3s;">
                    🚀 Manage Trading Rules
                </a>
            </div>
        </div>
    </div>

    <script>
        let currentTab = 'positions';
        let refreshInterval;
        let dataDelayMinutes = 15; // Cache the delay value

        // Initialize dashboard
        window.onload = function() {
            refreshPortfolio();
            loadPositions();
            loadWatchlist();
            loadMarketStatus();
            loadSystemVersion();
            initializeClock();
            
            // Auto-refresh every 30 seconds
            refreshInterval = setInterval(() => {
                refreshPortfolio();
                loadPositions();
                loadMarketStatus();
            }, 30000);
        };

        function togglePrice() {
            const orderType = document.getElementById('orderType').value;
            const priceGroup = document.getElementById('priceGroup');
            priceGroup.style.display = orderType === 'LIMIT' ? 'block' : 'none';
        }

        function switchTab(tabName) {
            document.querySelectorAll('.tab').forEach(t => t.classList.remove('active'));
            document.querySelectorAll('.tab-content').forEach(t => t.classList.remove('active'));
            
            event.target.classList.add('active');
            document.getElementById(tabName).classList.add('active');
            currentTab = tabName;
        }

        async function refreshPortfolio() {
            try {
                const response = await fetch('/api/portfolio');
                const data = await response.json();
                
                if (data.success && data.portfolio) {
                    const p = data.portfolio;
                    document.getElementById('totalValue').textContent = formatCurrency(p.total_value);
                    document.getElementById('availableCash').textContent = formatCurrency(p.available_cash);
                    document.getElementById('totalExposure').textContent = formatCurrency(p.total_exposure);
                    document.getElementById('positionCount').textContent = p.position_count;
                }
            } catch (error) {
                console.error('Failed to refresh portfolio:', error);
            }
        }

        async function loadPositions() {
            const loading = document.getElementById('positionsLoading');
            const content = document.getElementById('positionsContent');
            const body = document.getElementById('positionsBody');
            const noPositions = document.getElementById('noPositions');
            
            try {
                const response = await fetch('/api/positions');
                const data = await response.json();
                
                loading.style.display = 'none';
                content.style.display = 'block';
                
                if (data.success && data.positions && Object.keys(data.positions).length > 0) {
                    body.innerHTML = '';
                    noPositions.style.display = 'none';
                    
                    let totalOpenPnL = 0;
                    let totalRealizedPnL = 0;

                    // Group positions by symbol for cumulative P&L calculation
                    const symbolGroups = {};
                    Object.entries(data.positions).forEach(([positionId, position]) => {
                        const symbol = position.symbol;
                        if (!symbolGroups[symbol]) {
                            symbolGroups[symbol] = {
                                openPositions: [],
                                closedPositions: [],
                                totalRealizedPnL: 0,
                                totalQuantity: 0,
                                totalCost: 0
                            };
                        }
                        
                        // Add realized P&L from this position to symbol total
                        symbolGroups[symbol].totalRealizedPnL += (position.realized_pnl || 0);
                        
                        // Categorize position
                        const isClosed = position.status === 'CLOSED' || position.quantity === 0;
                        if (isClosed) {
                            symbolGroups[symbol].closedPositions.push(position);
                        } else {
                            symbolGroups[symbol].openPositions.push(position);
                            symbolGroups[symbol].totalQuantity += position.quantity;
                            symbolGroups[symbol].totalCost += (position.total_cost || 0);
                        }
                    });

                    // Display one row per symbol with aggregated data
                    Object.entries(symbolGroups).forEach(([symbol, group]) => {
                        const row = document.createElement('tr');
                        
                        // Calculate Open P&L from open positions only
                        let openPnL = 0;
                        let avgCostBasis = 0;
                        let quantityDisplay = '';
                        
                        if (group.openPositions.length > 0) {
                            // Calculate weighted average cost basis for display
                            let totalShares = 0;
                            let totalValue = 0;
                            let currentPrice = 0;
                            
                            group.openPositions.forEach(pos => {
                                totalShares += pos.quantity;
                                totalValue += pos.quantity * pos.avg_cost_basis;
                                currentPrice = pos.current_price; // Use latest price
                            });
                            
                            if (totalShares > 0) {
                                avgCostBasis = totalValue / totalShares;
                                openPnL = Math.round((currentPrice - avgCostBasis) * totalShares * 100) / 100;
                                quantityDisplay = `${totalShares} @ $${avgCostBasis.toFixed(5)}`;
                            }
                        } else {
                            quantityDisplay = 'CLOSED';
                        }
                        
                        // Use cumulative realized P&L for the symbol
                        const realizedPnL = group.totalRealizedPnL;
                        
                        totalOpenPnL += openPnL;
                        totalRealizedPnL += realizedPnL;
                        
                        const openPnLClass = openPnL >= 0 ? 'color: #00b894' : 'color: #e17055';
                        const realizedPnLClass = realizedPnL >= 0 ? 'color: #00b894' : 'color: #e17055';
                        
                        // Style based on whether symbol has open positions
                        const isClosed = group.openPositions.length === 0;
                        const rowStyle = isClosed ? 'opacity: 0.7; font-style: italic;' : '';
                        
                        row.style.cssText = rowStyle;
                        row.innerHTML = `
                            <td>${symbol}${isClosed ? ' (Closed Today)' : ''}</td>
                            <td>${quantityDisplay}</td>
                            <td>${formatCurrency(group.totalCost || 0)}</td>
                            <td style="${openPnLClass}">${formatCurrency(openPnL)}</td>
                            <td style="${realizedPnLClass}">${formatCurrency(realizedPnL)}</td>
                        `;
                        body.appendChild(row);
                    });

                    // Add totals row like in reference image
                    const totalsRow = document.createElement('tr');
                    totalsRow.style.borderTop = '2px solid #74787e';
                    totalsRow.style.fontWeight = 'bold';
                    
                    const totalOpenClass = totalOpenPnL >= 0 ? 'color: #00b894' : 'color: #e17055';
                    const totalRealizedClass = totalRealizedPnL >= 0 ? 'color: #00b894' : 'color: #e17055';
                    
                    totalsRow.innerHTML = `
                        <td><strong>All</strong></td>
                        <td><strong>0</strong></td>
                        <td></td>
                        <td style="${totalOpenClass}"><strong>${formatCurrency(totalOpenPnL)}</strong></td>
                        <td style="${totalRealizedClass}"><strong>${formatCurrency(totalRealizedPnL)}</strong></td>
                    `;
                    body.appendChild(totalsRow);
                } else {
                    body.innerHTML = '';
                    noPositions.style.display = 'block';
                }
            } catch (error) {
                loading.textContent = 'Error loading positions';
                console.error('Failed to load positions:', error);
            }
        }

        async function loadWatchlist() {
            const content = document.getElementById('watchlistContent');
            
            try {
                const response = await fetch('/api/symbols');
                const data = await response.json();
                
                if (data.success && data.data && data.data.length > 0) {
                    content.innerHTML = '';
                    data.data.forEach(symbol => {
                        const item = document.createElement('div');
                        item.className = 'watchlist-item';
                        item.innerHTML = `
                            <div>
                                <strong>${symbol.symbol}</strong>
                                ${symbol.description ? `<br><small>${symbol.description}</small>` : ''}
                            </div>
                            <div>
                                <button onclick="loadSymbolData('${symbol.symbol}')" style="font-size: 0.7rem; padding: 0.3rem 0.6rem;">Quote</button>
                            </div>
                        `;
                        content.appendChild(item);
                    });
                } else {
                    content.innerHTML = '<div style="text-align: center; color: #74787e; padding: 2rem;">No symbols in watchlist</div>';
                }
            } catch (error) {
                content.innerHTML = '<div style="text-align: center; color: #e17055; padding: 2rem;">Error loading watchlist</div>';
                console.error('Failed to load watchlist:', error);
            }
        }

        async function loadMarketStatus() {
            try {
                const [marketResp, wsResp, modeResp] = await Promise.all([
                    fetch('/api/market/status'),
                    fetch('/api/websocket/status'),
                    fetch('/api/system/mode')
                ]);
                
                const marketData = await marketResp.json();
                const wsData = await wsResp.json();
                const modeData = await modeResp.json();
                
                if (marketData.success) {
                    document.getElementById('marketStatus').textContent = marketData.status || 'UNKNOWN';
                }
                
                if (wsData.success) {
                    document.getElementById('wsStatus').textContent = wsData.websocket_enabled ? 'Connected' : 'Disconnected';
                    document.getElementById('wsSubscriptions').textContent = wsData.subscriptions_count || 0;
                }
                
                // Update system status line with trading mode
                if (modeData.success) {
                    const statusText = document.getElementById('statusText');
                    const statusDot = document.getElementById('statusDot');
                    const mode = modeData.mode || 'UNKNOWN';
                    const marketStatus = marketData.success ? marketData.status : 'UNKNOWN';
                    
                    statusText.textContent = `System Online • ${mode} Mode • Market: ${marketStatus}`;
                    
                    // Update status indicator color based on mode
                    statusDot.className = mode === 'PAPER' ? 'status-indicator status-online' : 'status-indicator status-online';
                }
            } catch (error) {
                console.error('Failed to load market status:', error);
            }
        }

        async function placeOrder(event) {
            event.preventDefault();
            
            const symbol = document.getElementById('symbol').value.toUpperCase();
            const side = document.getElementById('side').value;
            const orderType = document.getElementById('orderType').value;
            const quantity = parseInt(document.getElementById('quantity').value);
            const price = orderType === 'LIMIT' ? parseFloat(document.getElementById('price').value) : null;

            const orderData = {
                symbol: symbol,
                side: side,
                order_type: orderType,
                quantity: quantity,
                price: price
            };

            showResult('📤 Placing paper trade...', 'success');

            try {
                const response = await fetch('/api/order', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(orderData)
                });

                const result = await response.json();
                
                if (result.success) {
                    showResult(`✅ Paper trade executed! Order ID: ${result.order_id}`, 'success');
                    setTimeout(() => {
                        refreshPortfolio();
                        loadPositions();
                    }, 1000);
                } else {
                    showResult(`❌ Trade failed: ${result.error}`, 'error');
                }
            } catch (error) {
                showResult(`❌ Request failed: ${error.message}`, 'error');
            }
        }

        async function getQuote() {
            const symbol = document.getElementById('symbol').value.toUpperCase();
            if (!symbol) return;

            showResult(`📊 Getting quote for ${symbol}...`, 'success');

            try {
                const response = await fetch(`/api/snapshot/${symbol}`);
                const result = await response.json();
                
                if (result.success && result.data) {
                    const data = result.data;
                    showResult(`📊 ${symbol}: $${data.close} (${data.change_percent >= 0 ? '+' : ''}${data.change_percent.toFixed(2)}%)`, 'success');
                } else {
                    showResult(`❌ Failed to get quote: ${result.error}`, 'error');
                }
            } catch (error) {
                showResult(`❌ Quote request failed: ${error.message}`, 'error');
            }
        }

        async function loadIndicators() {
            const symbol = document.getElementById('indicatorSymbol').value.toUpperCase();
            if (!symbol) return;

            const results = document.getElementById('indicatorResults');
            results.innerHTML = '<div class="loading">Loading indicators...</div>';

            try {
                const [rsiResp, smaResp, emaResp] = await Promise.all([
                    fetch(`/api/indicators/rsi/${symbol}`),
                    fetch(`/api/indicators/sma/${symbol}?window=20`),
                    fetch(`/api/indicators/ema/${symbol}?window=20`)
                ]);

                const [rsiData, smaData, emaData] = await Promise.all([
                    rsiResp.json(), smaResp.json(), emaResp.json()
                ]);

                let html = `<h4>${symbol} Technical Indicators</h4>`;
                
                if (rsiData.success && rsiData.value) {
                    html += `<p><strong>RSI (14):</strong> ${rsiData.value.toFixed(2)}</p>`;
                }
                if (smaData.success && smaData.value) {
                    html += `<p><strong>SMA (20):</strong> $${smaData.value.toFixed(2)}</p>`;
                }
                if (emaData.success && emaData.value) {
                    html += `<p><strong>EMA (20):</strong> $${emaData.value.toFixed(2)}</p>`;
                }

                results.innerHTML = html;
            } catch (error) {
                results.innerHTML = `<div style="color: #e17055;">Error loading indicators: ${error.message}</div>`;
            }
        }

        function showResult(message, type) {
            const resultDiv = document.getElementById('result');
            resultDiv.className = `result ${type}`;
            resultDiv.innerHTML = message;
        }

        async function loadSystemVersion() {
            try {
                const response = await fetch('/api/system/version');
                const data = await response.json();
                
                if (data.success) {
                    document.getElementById('systemVersion').textContent = `v${data.version}`;
                    
                    dataDelayMinutes = data.data_delay_minutes || 0; // Cache globally
                    const delayElement = document.getElementById('dataDelay');
                    
                    if (dataDelayMinutes > 0) {
                        delayElement.textContent = `${dataDelayMinutes}min delayed`;
                        delayElement.style.display = 'block';
                    } else {
                        delayElement.textContent = 'real-time';
                        delayElement.style.display = 'block';
                    }
                }
            } catch (error) {
                console.error('Failed to load system version:', error);
            }
        }

        function initializeClock() {
            updateClock(); // Set immediately
            setInterval(updateClock, 1000); // Update every second
        }

        function updateClock() {
            try {
                // Get current UTC time and convert to Eastern Time
                const now = new Date();
                const utc = new Date(now.getTime() + (now.getTimezoneOffset() * 60000));
                
                // Calculate Eastern Time (EST/EDT)
                const easternOffset = isDST(now) ? -4 : -5; // EDT = -4, EST = -5
                const eastern = new Date(utc.getTime() + (easternOffset * 3600000));
                
                // Apply data delay using cached value
                const displayTime = new Date(eastern.getTime() - (dataDelayMinutes * 60000));
                
                // Format time
                const timeString = displayTime.toLocaleTimeString('en-US', {
                    hour12: false,
                    hour: '2-digit',
                    minute: '2-digit',
                    second: '2-digit'
                });
                
                const timezone = isDST(now) ? 'EDT' : 'EST';
                document.getElementById('systemClock').textContent = `${timeString} ${timezone}`;
                
            } catch (error) {
                // Fallback to local time if API fails
                const now = new Date();
                document.getElementById('systemClock').textContent = 
                    now.toLocaleTimeString('en-US', { hour12: false }) + ' LOCAL';
            }
        }

        function isDST(date) {
            // Simple DST check for US Eastern Time
            const year = date.getFullYear();
            
            // DST starts second Sunday in March
            const dstStart = new Date(year, 2, 14 - new Date(year, 2, 1).getDay());
            
            // DST ends first Sunday in November  
            const dstEnd = new Date(year, 10, 7 - new Date(year, 10, 1).getDay());
            
            return date >= dstStart && date < dstEnd;
        }

        function formatCurrency(value) {
            return new Intl.NumberFormat('en-US', {
                style: 'currency',
                currency: 'USD'
            }).format(value);
        }

        // Database reset function
        async function resetDatabase() {
            const confirmed = confirm(
                "⚠️ WARNING: This will permanently delete ALL paper trading data!\n\n" +
                "This includes:\n" +
                "• All open and closed positions\n" +
                "• All trade history and transactions\n" +
                "• Portfolio balances and P&L history\n" +
                "• Trading rules configuration\n" +
                "• Session data and statistics\n\n" +
                "This action CANNOT be undone.\n\n" +
                "Are you absolutely sure you want to reset the database?"
            );
            
            if (!confirmed) {
                return;
            }
            
            try {
                const response = await fetch('/api/reset-database', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json'
                    }
                });
                
                const result = await response.json();
                
                if (result.success) {
                    alert("✅ Database has been reset successfully!\n\nRefreshing dashboard...");
                    // Refresh the page to show clean data
                    window.location.reload();
                } else {
                    alert("❌ Database reset failed: " + (result.error || "Unknown error"));
                }
            } catch (error) {
                alert("❌ Error communicating with server: " + error.message);
            }
        }

        // Initialize dashboard
        window.onload = function() {
            refreshPortfolio();
            loadPositions();
            loadWatchlist();
            loadMarketStatus();
            loadSystemVersion();
            
            refreshInterval = setInterval(() => {
                refreshPortfolio();
                loadPositions();
            }, 30000); // Refresh every 30 seconds
        };
    </script>
</body>
</html>"#)
}

// Rules Engine Management Page
async fn rules_page() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html>
<html>
<head>
    <title>🤖 Rules Engine Management</title>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style>
        * { box-sizing: border-box; margin: 0; padding: 0; }
        body { 
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; 
            background: #f5f7fa; 
            color: #2c3e50;
            line-height: 1.6;
        }
        .header { 
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); 
            color: white; 
            padding: 1rem 2rem; 
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
        }
        .header h1 { 
            font-size: 1.8rem; 
            margin-bottom: 0.5rem; 
        }
        .header .subtitle { 
            opacity: 0.9; 
            font-size: 0.9rem; 
            display: flex;
            justify-content: space-between;
            align-items: center;
        }
        .nav-links {
            display: flex;
            gap: 1rem;
        }
        .nav-links a {
            color: white;
            text-decoration: none;
            padding: 0.5rem 1rem;
            border-radius: 6px;
            background: rgba(255,255,255,0.1);
            transition: background 0.3s;
        }
        .nav-links a:hover {
            background: rgba(255,255,255,0.2);
        }
        .container { 
            max-width: 1200px; 
            margin: 2rem auto; 
            padding: 0 2rem;
            display: grid;
            gap: 2rem;
        }
        .card { 
            background: white; 
            border-radius: 12px; 
            padding: 1.5rem; 
            box-shadow: 0 2px 10px rgba(0,0,0,0.08);
            border: 1px solid #e1e8ed;
        }
        .card h3 { 
            color: #2c3e50; 
            margin-bottom: 1rem; 
            padding-bottom: 0.5rem;
            border-bottom: 2px solid #ecf0f1;
            font-size: 1.1rem;
            display: flex;
            justify-content: space-between;
            align-items: center;
        }
        .status-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 1rem;
            margin: 1rem 0;
        }
        .status-item {
            background: #f8f9fa;
            padding: 1rem;
            border-radius: 8px;
            border-left: 4px solid #74b9ff;
            text-align: center;
        }
        .status-item .value {
            font-size: 1.5rem;
            font-weight: bold;
            margin-bottom: 0.5rem;
        }
        .status-item .success { color: #00b894; }
        .status-item .error { color: #e17055; }
        .form-section {
            background: #f8f9fa;
            padding: 1.5rem;
            border-radius: 8px;
            border: 1px solid #e1e8ed;
            margin-bottom: 2rem;
        }
        .form-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
            gap: 1rem;
            margin-bottom: 1rem;
        }
        .form-group {
            display: flex;
            flex-direction: column;
        }
        .form-group label {
            font-weight: 600;
            margin-bottom: 0.5rem;
            color: #2c3e50;
        }
        .form-group input,
        .form-group select {
            padding: 0.75rem;
            border: 2px solid #e1e8ed;
            border-radius: 6px;
            font-size: 1rem;
            transition: border-color 0.3s;
        }
        .form-group input:focus,
        .form-group select:focus {
            outline: none;
            border-color: #74b9ff;
        }
        button {
            background: #74b9ff;
            color: white;
            border: none;
            padding: 0.75rem 1.5rem;
            border-radius: 6px;
            font-size: 1rem;
            cursor: pointer;
            transition: background 0.3s;
            margin-right: 0.5rem;
        }
        button:hover {
            background: #0984e3;
        }
        button.success {
            background: #00b894;
        }
        button.success:hover {
            background: #00a085;
        }
        button.secondary {
            background: #74787e;
        }
        button.secondary:hover {
            background: #636e72;
        }
        button.danger {
            background: #e17055;
        }
        button.danger:hover {
            background: #d63031;
        }
        .rule-item {
            background: white;
            border: 1px solid #e1e8ed;
            border-radius: 8px;
            padding: 1.5rem;
            margin-bottom: 1rem;
            transition: box-shadow 0.3s;
        }
        .rule-item:hover {
            box-shadow: 0 4px 15px rgba(0,0,0,0.1);
        }
        .rule-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 1rem;
        }
        .rule-name {
            font-size: 1.1rem;
            font-weight: bold;
            color: #2c3e50;
        }
        .rule-status {
            padding: 0.25rem 0.75rem;
            border-radius: 20px;
            font-size: 0.8rem;
            font-weight: bold;
        }
        .rule-status.active {
            background: #d1f2eb;
            color: #00b894;
        }
        .rule-status.inactive {
            background: #f8f9fa;
            color: #74787e;
        }
        .rule-details {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
            gap: 1rem;
            margin-bottom: 1rem;
            color: #74787e;
        }
        .rule-actions {
            display: flex;
            gap: 0.5rem;
            justify-content: flex-end;
        }
        .rule-actions button {
            padding: 0.5rem 1rem;
            font-size: 0.9rem;
        }
        .loading {
            text-align: center;
            padding: 2rem;
            color: #74787e;
        }
        .no-rules {
            text-align: center;
            padding: 3rem;
            color: #74787e;
            font-style: italic;
        }
        .alert {
            padding: 1rem;
            border-radius: 6px;
            margin-bottom: 1rem;
        }
        .alert.success {
            background: #d1f2eb;
            color: #00b894;
            border-left: 4px solid #00b894;
        }
        .alert.error {
            background: #ffeaa7;
            color: #e17055;
            border-left: 4px solid #e17055;
        }
        @media (max-width: 768px) {
            .container { padding: 0 1rem; }
            .form-grid { grid-template-columns: 1fr; }
            .nav-links { flex-direction: column; gap: 0.5rem; }
        }
    </style>
</head>
<body>
    <div class="header">
        <div class="header-content">
            <h1>🤖 Rules Engine Management</h1>
            <div class="subtitle">
                <span>Create and manage your automated trading rules</span>
                <div class="nav-links">
                    <a href="/">📊 Dashboard</a>
                    <a href="/rules">🤖 Rules</a>
                </div>
            </div>
        </div>
    </div>

    <div class="container">
        <!-- Rules Engine Status -->
        <div class="card">
            <h3>⚡ Engine Status
                <button class="secondary" onclick="refreshStatus()">Refresh</button>
            </h3>
            <div id="engineStatus">
                <div class="loading">Loading engine status...</div>
            </div>
        </div>

        <!-- Create New Rule -->
        <div class="card">
            <h3>➕ Create New Trading Rule</h3>
            <div id="alerts"></div>
            <form class="form-section" onsubmit="createRule(event)">
                <div class="form-grid">
                    <div class="form-group">
                        <label for="ruleName">Rule Name*</label>
                        <input type="text" id="ruleName" required placeholder="e.g., NVDA Momentum Breakout">
                    </div>
                    <div class="form-group">
                        <label for="ruleStrategy">Strategy*</label>
                        <select id="ruleStrategy" required>
                            <option value="">Select Strategy</option>
                            <option value="momentum_breakout">Momentum Breakout</option>
                            <option value="mean_reversion">Mean Reversion</option>
                            <option value="volume_spike">Volume Spike</option>
                            <option value="rsi_oversold">RSI Oversold</option>
                            <option value="moving_average_cross">Moving Average Crossover</option>
                        </select>
                    </div>
                    <div class="form-group">
                        <label for="positionSize">Position Size (%)*</label>
                        <input type="number" id="positionSize" value="5.0" min="0.1" max="25.0" step="0.1" required>
                    </div>
                    <div class="form-group">
                        <label for="stopLoss">Stop Loss (%)</label>
                        <input type="number" id="stopLoss" value="3.0" min="0.1" max="20.0" step="0.1">
                    </div>
                    <div class="form-group">
                        <label for="takeProfit">Take Profit (%)</label>
                        <input type="number" id="takeProfit" value="10.0" min="0.1" max="50.0" step="0.1">
                    </div>
                    <div class="form-group">
                        <label for="ruleActive">Rule Active</label>
                        <select id="ruleActive">
                            <option value="true">Active</option>
                            <option value="false">Inactive</option>
                        </select>
                    </div>
                </div>
                <button type="submit" class="success">Create Trading Rule</button>
                <button type="button" class="secondary" onclick="clearForm()">Clear Form</button>
            </form>
        </div>

        <!-- Active Rules List -->
        <div class="card">
            <h3>📋 Active Trading Rules
                <button class="secondary" onclick="loadRules()">Refresh</button>
            </h3>
            <div id="rulesList">
                <div class="loading">Loading rules...</div>
            </div>
        </div>
    </div>

    <script>
        let refreshInterval;

        // Initialize page
        window.onload = function() {
            refreshStatus();
            loadRules();
            
            // Set up auto-refresh
            refreshInterval = setInterval(() => {
                refreshStatus();
                loadRules();
            }, 30000); // Refresh every 30 seconds
        };

        // Load engine status
        async function refreshStatus() {
            try {
                const response = await fetch('/api/rules/engine/status');
                const data = await response.json();
                
                const statusDiv = document.getElementById('engineStatus');
                
                if (data.success) {
                    statusDiv.innerHTML = `
                        <div class="status-grid">
                            <div class="status-item">
                                <div class="value ${data.running ? 'success' : 'error'}">
                                    ${data.running ? '🟢 RUNNING' : '🔴 STOPPED'}
                                </div>
                                <div>Engine Status</div>
                            </div>
                            <div class="status-item">
                                <div class="value">${data.total_rules || 0}</div>
                                <div>Total Rules</div>
                            </div>
                            <div class="status-item">
                                <div class="value success">${data.active_rules || 0}</div>
                                <div>Active Rules</div>
                            </div>
                            <div class="status-item">
                                <div class="value">$100,000</div>
                                <div>Available Capital</div>
                            </div>
                        </div>
                    `;
                } else {
                    statusDiv.innerHTML = '<div class="alert error">Failed to load engine status</div>';
                }
            } catch (error) {
                console.error('Error loading status:', error);
                document.getElementById('engineStatus').innerHTML = 
                    '<div class="alert error">Error connecting to rules engine</div>';
            }
        }

        // Load rules list
        async function loadRules() {
            try {
                const response = await fetch('/api/rules');
                const data = await response.json();
                
                const rulesList = document.getElementById('rulesList');
                
                if (data.success && data.rules && data.rules.length > 0) {
                    rulesList.innerHTML = data.rules.map(rule => `
                        <div class="rule-item">
                            <div class="rule-header">
                                <div class="rule-name">${rule.name}</div>
                                <div class="rule-status ${rule.active ? 'active' : 'inactive'}">
                                    ${rule.active ? 'ACTIVE' : 'INACTIVE'}
                                </div>
                            </div>
                            <div class="rule-details">
                                <div><strong>Strategy:</strong> ${rule.strategy}</div>
                                <div><strong>Position:</strong> ${rule.position_size_percent}%</div>
                                <div><strong>Stop Loss:</strong> ${rule.stop_loss_percent || 'N/A'}%</div>
                                <div><strong>Take Profit:</strong> ${rule.take_profit_percent || 'N/A'}%</div>
                            </div>
                            <div class="rule-actions">
                                <button onclick="toggleRule('${rule.id}', ${!rule.active})" class="secondary">
                                    ${rule.active ? 'Deactivate' : 'Activate'}
                                </button>
                                <button onclick="deleteRule('${rule.id}')" class="danger">Delete</button>
                            </div>
                        </div>
                    `).join('');
                } else {
                    rulesList.innerHTML = `
                        <div class="no-rules">
                            <h4>No trading rules created yet</h4>
                            <p>Create your first automated trading rule using the form above.</p>
                        </div>
                    `;
                }
            } catch (error) {
                console.error('Error loading rules:', error);
                document.getElementById('rulesList').innerHTML = 
                    '<div class="alert error">Error loading rules list</div>';
            }
        }

        // Create new rule
        async function createRule(event) {
            event.preventDefault();
            
            const formData = {
                name: document.getElementById('ruleName').value,
                strategy: document.getElementById('ruleStrategy').value,
                position_size_percent: parseFloat(document.getElementById('positionSize').value),
                stop_loss_percent: parseFloat(document.getElementById('stopLoss').value) || null,
                take_profit_percent: parseFloat(document.getElementById('takeProfit').value) || null,
                active: document.getElementById('ruleActive').value === 'true',
                description: `${document.getElementById('ruleStrategy').value} rule created via web interface`
            };
            
            try {
                const response = await fetch('/api/rules', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json'
                    },
                    body: JSON.stringify(formData)
                });
                
                const result = await response.json();
                
                if (result.success) {
                    showAlert('✅ Rule created successfully!', 'success');
                    clearForm();
                    loadRules();
                    refreshStatus();
                } else {
                    showAlert(`❌ Failed to create rule: ${result.error || 'Unknown error'}`, 'error');
                }
            } catch (error) {
                console.error('Error creating rule:', error);
                showAlert('❌ Network error creating rule', 'error');
            }
        }

        // Delete rule
        async function deleteRule(ruleId) {
            if (!confirm('Are you sure you want to delete this rule?')) return;
            
            try {
                const response = await fetch(`/api/rules/${ruleId}`, {
                    method: 'DELETE'
                });
                
                const result = await response.json();
                
                if (result.success) {
                    showAlert('✅ Rule deleted successfully!', 'success');
                    loadRules();
                    refreshStatus();
                } else {
                    showAlert(`❌ Failed to delete rule: ${result.error || 'Unknown error'}`, 'error');
                }
            } catch (error) {
                console.error('Error deleting rule:', error);
                showAlert('❌ Network error deleting rule', 'error');
            }
        }

        // Toggle rule active status
        async function toggleRule(ruleId, newStatus) {
            try {
                const response = await fetch(`/api/rules/${ruleId}`, {
                    method: 'PATCH',
                    headers: {
                        'Content-Type': 'application/json'
                    },
                    body: JSON.stringify({
                        active: newStatus
                    })
                });
                
                const result = await response.json();
                
                if (result.success) {
                    showAlert(`✅ Rule ${newStatus ? 'activated' : 'deactivated'} successfully!`, 'success');
                    loadRules();
                    refreshStatus();
                } else {
                    showAlert(`❌ Failed to toggle rule: ${result.error || 'Unknown error'}`, 'error');
                }
            } catch (error) {
                console.error('Error toggling rule:', error);
                showAlert('❌ Network error toggling rule', 'error');
            }
        }

        // Clear form
        function clearForm() {
            document.getElementById('ruleName').value = '';
            document.getElementById('ruleStrategy').value = '';
            document.getElementById('positionSize').value = '5.0';
            document.getElementById('stopLoss').value = '3.0';
            document.getElementById('takeProfit').value = '10.0';
            document.getElementById('ruleActive').value = 'true';
        }

        // Show alert
        function showAlert(message, type) {
            const alertsDiv = document.getElementById('alerts');
            const alert = document.createElement('div');
            alert.className = `alert ${type}`;
            alert.textContent = message;
            
            alertsDiv.innerHTML = '';
            alertsDiv.appendChild(alert);
            
            // Auto-hide after 5 seconds
            setTimeout(() => {
                alert.remove();
            }, 5000);
        }
    </script>
</body>
</html>"#)
}

// System mode endpoint
async fn get_system_mode(
    State(engine): State<Arc<TradingEngine>>,
) -> Json<SystemModeResponse> {
    tracing::info!("🔧 System mode request");

    let trading_mode = engine.get_trading_mode().await;
    let (mode_name, description) = match trading_mode {
        crate::config::TradingMode::Paper => (
            "PAPER".to_string(),
            "Paper Trading Mode - Virtual portfolio with real market data".to_string()
        ),
        crate::config::TradingMode::Live => (
            "LIVE".to_string(),
            "Live Trading Mode - Real money trading".to_string()
        ),
        crate::config::TradingMode::Simulation => (
            "SIMULATION".to_string(), 
            "Simulation Mode - Backtesting and strategy validation".to_string()
        ),
    };

    Json(SystemModeResponse {
        success: true,
        mode: Some(mode_name),
        description: Some(description),
        error: None,
    })
}

// System version endpoint
async fn get_system_version(
    State(engine): State<Arc<TradingEngine>>,
) -> Json<SystemVersionResponse> {
    tracing::info!("🔧 System version request");

    // Get git info - in production this would be baked into the binary at build time
    // For now we'll use static values that represent the branch name from CLAUDE.md
    let branch = "9.2.25.1".to_string();
    let commit = "945055e".to_string();
    let version = format!("{}-{}", branch, commit);
    
    // Check if using delayed data from config
    let config = engine.get_config();
    let data_delay_minutes = if config.polygon_use_delayed_data {
        Some(15) // Stock Starter plan has 15-minute delay
    } else {
        Some(0)  // Real-time data
    };
    
    let polygon_mode = if config.polygon_use_delayed_data {
        "delayed".to_string()
    } else {
        "real-time".to_string()
    };

    Json(SystemVersionResponse {
        success: true,
        version: Some(version),
        branch: Some(branch),
        commit: Some(commit),
        data_delay_minutes,
        polygon_mode: Some(polygon_mode),
        error: None,
    })
}

async fn health() -> &'static str {
    "OK"
}

async fn get_status(State(engine): State<Arc<TradingEngine>>) -> Json<StatusResponse> {
    let connected = engine.is_broker_connected().await;
    let message = if connected {
        "✅ Broker connected and ready for trading".to_string()
    } else {
        "❌ Broker not connected".to_string()
    };

    Json(StatusResponse {
        broker_connected: connected,
        session_active: connected,
        message,
    })
}

async fn lookup_symbol(
    State(engine): State<Arc<TradingEngine>>,
    Json(request): Json<SymbolRequest>,
) -> Json<SymbolResponse> {
    tracing::info!("📊 Received symbol lookup request: {}", request.symbol);

    match engine.lookup_symbol(&request.symbol).await {
        Ok(data) => {
            tracing::info!("✅ Symbol lookup successful: {} - ${:.2}", request.symbol, data.close);
            Json(SymbolResponse {
                success: true,
                data: Some(data),
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("❌ Symbol lookup failed: {}", e);
            Json(SymbolResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })
        }
    }
}

async fn place_order(
    State(engine): State<Arc<TradingEngine>>,
    Json(request): Json<OrderRequest>,
) -> Json<OrderResponse> {
    tracing::info!(
        "📊 Received order request: {} {} {} @ {:?}",
        request.side,
        request.quantity,
        request.symbol,
        request.price
    );

    match engine.place_order(
        &request.symbol,
        &request.side,
        &request.order_type,
        request.quantity,
        request.price,
    ).await {
        Ok(order_id) => {
            tracing::info!("✅ Order placed successfully: {}", order_id);
            Json(OrderResponse {
                success: true,
                order_id: Some(order_id),
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("❌ Order placement failed: {}", e);
            Json(OrderResponse {
                success: false,
                order_id: None,
                error: Some(e.to_string()),
            })
        }
    }
}

// Technical Indicator Handlers
async fn get_sma(
    State(engine): State<Arc<TradingEngine>>,
    Path(symbol): Path<String>,
    Query(params): Query<IndicatorQuery>,
) -> Json<IndicatorResponse> {
    let window = params.window.unwrap_or(50);
    let timespan = params.timespan.as_deref().unwrap_or("day");
    // Use a recent date that should have data (30 days ago)
    let default_timestamp = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(30))
        .unwrap_or_else(chrono::Utc::now)
        .format("%Y-%m-%d")
        .to_string();
    let timestamp = params.timestamp.as_deref().unwrap_or(&default_timestamp);
    
    tracing::info!("📊 SMA request: {} window={} timespan={} timestamp={}", symbol, window, timespan, timestamp);

    match engine.get_data_module().get_sma_cached(&symbol, window, timespan, timestamp).await {
        Ok(response) => {
            tracing::info!("📊 SMA response status: {} results: {:?}", response.status, response.results.is_some());
            let value = response.results
                .as_ref()
                .and_then(|r| r.values.as_ref())
                .and_then(|v| v.first())
                .and_then(|iv| iv.value);
            
            // Return the full response for debugging
            let data = serde_json::to_value(&response).ok();
            
            Json(IndicatorResponse {
                success: true,
                value,
                data,
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("❌ SMA calculation failed: {}", e);
            Json(IndicatorResponse {
                success: false,
                value: None,
                data: None,
                error: Some(e.to_string()),
            })
        }
    }
}

async fn get_ema(
    State(engine): State<Arc<TradingEngine>>,
    Path(symbol): Path<String>,
    Query(params): Query<IndicatorQuery>,
) -> Json<IndicatorResponse> {
    let window = params.window.unwrap_or(50);
    let timespan = params.timespan.as_deref().unwrap_or("day");
    // Use a recent date that should have data (30 days ago)
    let default_timestamp = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(30))
        .unwrap_or_else(chrono::Utc::now)
        .format("%Y-%m-%d")
        .to_string();
    let timestamp = params.timestamp.as_deref().unwrap_or(&default_timestamp);
    
    tracing::info!("📊 EMA request: {} window={} timespan={} timestamp={}", symbol, window, timespan, timestamp);

    match engine.get_data_module().get_ema_cached(&symbol, window, timespan, timestamp).await {
        Ok(response) => {
            tracing::info!("📊 EMA response status: {} results: {:?}", response.status, response.results.is_some());
            let value = response.results
                .as_ref()
                .and_then(|r| r.values.as_ref())
                .and_then(|v| v.first())
                .and_then(|iv| iv.value);
            
            // Return the full response for debugging
            let data = serde_json::to_value(&response).ok();
            
            Json(IndicatorResponse {
                success: true,
                value,
                data,
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("❌ EMA calculation failed: {}", e);
            Json(IndicatorResponse {
                success: false,
                value: None,
                data: None,
                error: Some(e.to_string()),
            })
        }
    }
}

async fn get_rsi(
    State(engine): State<Arc<TradingEngine>>,
    Path(symbol): Path<String>,
    Query(params): Query<IndicatorQuery>,
) -> Json<IndicatorResponse> {
    let window = params.window.unwrap_or(14);
    let timespan = params.timespan.as_deref().unwrap_or("day");
    // Use a recent date that should have data (30 days ago)
    let default_timestamp = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(30))
        .unwrap_or_else(chrono::Utc::now)
        .format("%Y-%m-%d")
        .to_string();
    let timestamp = params.timestamp.as_deref().unwrap_or(&default_timestamp);
    
    tracing::info!("📊 RSI request: {} window={} timespan={} timestamp={}", symbol, window, timespan, timestamp);

    match engine.get_data_module().get_rsi_cached(&symbol, window, timespan, timestamp).await {
        Ok(response) => {
            tracing::info!("📊 RSI response status: {} results: {:?}", response.status, response.results.is_some());
            let value = response.results
                .as_ref()
                .and_then(|r| r.values.as_ref())
                .and_then(|v| v.first())
                .and_then(|iv| iv.value);
            
            // Return the full response for debugging
            let data = serde_json::to_value(&response).ok();
            
            Json(IndicatorResponse {
                success: true,
                value,
                data,
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("❌ RSI calculation failed: {}", e);
            Json(IndicatorResponse {
                success: false,
                value: None,
                data: None,
                error: Some(e.to_string()),
            })
        }
    }
}

async fn get_macd(
    State(engine): State<Arc<TradingEngine>>,
    Path(symbol): Path<String>,
    Query(params): Query<IndicatorQuery>,
) -> Json<IndicatorResponse> {
    let short_window = params.short_window.unwrap_or(12);
    let long_window = params.long_window.unwrap_or(26);
    let signal_window = params.signal_window.unwrap_or(9);
    let timespan = params.timespan.as_deref().unwrap_or("day");
    // Use a recent date that should have data (30 days ago)
    let default_timestamp = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(30))
        .unwrap_or_else(chrono::Utc::now)
        .format("%Y-%m-%d")
        .to_string();
    let timestamp = params.timestamp.as_deref().unwrap_or(&default_timestamp);
    
    tracing::info!("📊 MACD request: {} short={} long={} signal={} timespan={} timestamp={}", 
                  symbol, short_window, long_window, signal_window, timespan, timestamp);

    // MACD returns different structure so we need special handling
    match engine.get_data_module().get_macd(&symbol, short_window, long_window, signal_window, timespan, timestamp).await {
        Ok(response) => {
            tracing::info!("📊 MACD response status: {} results: {:?}", response.status, response.results.is_some());
            let value = response.results
                .as_ref()
                .and_then(|r| r.values.as_ref())
                .and_then(|v| v.first())
                .and_then(|iv| iv.value);
            
            // Return the full response for debugging
            let data = serde_json::to_value(&response).ok();
            
            Json(IndicatorResponse {
                success: true,
                value,
                data,
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("❌ MACD calculation failed: {}", e);
            Json(IndicatorResponse {
                success: false,
                value: None,
                data: None,
                error: Some(e.to_string()),
            })
        }
    }
}

// Market Data Handlers
async fn get_snapshot(
    State(engine): State<Arc<TradingEngine>>,
    Path(symbol): Path<String>,
) -> Json<SymbolResponse> {
    tracing::info!("📊 Snapshot request: {}", symbol);

    // Use current market data with snapshot API for real-time pricing
    match engine.get_current_symbol_data(&symbol).await {
        Ok(data) => Json(SymbolResponse {
            success: true,
            data: Some(data),
            error: None,
        }),
        Err(e) => {
            tracing::error!("❌ Snapshot failed: {}", e);
            Json(SymbolResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })
        }
    }
}
async fn get_market_gainers(
    State(engine): State<Arc<TradingEngine>>,
) -> Json<MarketMoversResponse> {
    tracing::info!("📊 Market gainers request");

    match engine.get_data_module().get_market_movers("gainers").await {
        Ok(movers) => Json(MarketMoversResponse {
            success: true,
            data: Some(movers),
            error: None,
        }),
        Err(e) => {
            tracing::error!("❌ Market gainers failed: {}", e);
            Json(MarketMoversResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })
        }
    }
}

async fn get_market_losers(
    State(engine): State<Arc<TradingEngine>>,
) -> Json<MarketMoversResponse> {
    tracing::info!("📊 Market losers request");

    match engine.get_data_module().get_market_movers("losers").await {
        Ok(movers) => Json(MarketMoversResponse {
            success: true,
            data: Some(movers),
            error: None,
        }),
        Err(e) => {
            tracing::error!("❌ Market losers failed: {}", e);
            Json(MarketMoversResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })
        }
    }
}

async fn get_market_status(
    State(engine): State<Arc<TradingEngine>>,
) -> Json<MarketStatusResponse> {
    tracing::info!("📊 Market status request");

    match engine.get_data_module().get_market_status().await {
        Ok(response) => {
            let status = match response.market.as_deref() {
                Some("open") => "OPEN",
                Some("extended-hours") => {
                    // Check if it's pre-market or after-hours
                    if response.early_hours == Some(true) {
                        "PRE-MARKET"
                    } else if response.after_hours == Some(true) {
                        "AFTER-HOURS" 
                    } else {
                        "EXTENDED-HOURS"
                    }
                },
                Some("closed") => "CLOSED",
                _ => "UNKNOWN"
            };
            Json(MarketStatusResponse {
                success: true,
                status: Some(status.to_string()),
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("❌ Market status failed: {}", e);
            Json(MarketStatusResponse {
                success: false,
                status: None,
                error: Some(e.to_string()),
            })
        }
    }
}

async fn get_previous_day(
    State(engine): State<Arc<TradingEngine>>,
    Path(symbol): Path<String>,
) -> Json<SymbolResponse> {
    tracing::info!("📊 Previous day request: {}", symbol);

    match engine.get_data_module().get_previous_day_cached(&symbol).await {
        Ok(bar) => {
            let symbol_data = SymbolDataResponse {
                symbol: symbol.clone(),
                date: "previous_day".to_string(),
                open: bar.open.unwrap_or(0.0),
                high: bar.high.unwrap_or(0.0),
                low: bar.low.unwrap_or(0.0),
                close: bar.close.unwrap_or(0.0),
                volume: bar.volume.unwrap_or(0.0) as u64,
                vwap: bar.vwap.unwrap_or(0.0),
                transactions: 0,
                change: 0.0,
                change_percent: 0.0,
            };
            Json(SymbolResponse {
                success: true,
                data: Some(symbol_data),
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("❌ Previous day failed: {}", e);
            Json(SymbolResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })
        }
    }
}

async fn get_minute_aggregates(
    State(engine): State<Arc<TradingEngine>>,
    Path(symbol): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<SymbolResponse> {
    let from = params.get("from").cloned().unwrap_or_else(|| "2024-01-02".to_string());
    let to = params.get("to").cloned().unwrap_or_else(|| "2024-01-02".to_string());
    
    tracing::info!("📊 Minute aggregates request: {} from={} to={}", symbol, from, to);

    match engine.get_data_module().get_minute_aggregates(&symbol, &from, &to).await {
        Ok(bars) => {
            // Return summary of minute aggregates
            let symbol_data = if let Some(first_bar) = bars.first() {
                SymbolDataResponse {
                    symbol: symbol.clone(),
                    date: format!("{} to {}", from, to),
                    open: first_bar.open.unwrap_or(0.0),
                    high: bars.iter().map(|b| b.high.unwrap_or(0.0)).fold(0.0, f64::max),
                    low: bars.iter().map(|b| b.low.unwrap_or(f64::MAX)).fold(f64::MAX, f64::min),
                    close: bars.last().map(|b| b.close.unwrap_or(0.0)).unwrap_or(0.0),
                    volume: bars.iter().map(|b| b.volume.unwrap_or(0.0)).sum::<f64>() as u64,
                    vwap: 0.0,
                    transactions: bars.len() as u32,
                    change: 0.0,
                    change_percent: 0.0,
                }
            } else {
                SymbolDataResponse {
                    symbol: symbol.clone(),
                    date: format!("{} to {}", from, to),
                    open: 0.0,
                    high: 0.0,
                    low: 0.0,
                    close: 0.0,
                    volume: 0,
                    vwap: 0.0,
                    transactions: 0,
                    change: 0.0,
                    change_percent: 0.0,
                }
            };
            Json(SymbolResponse {
                success: true,
                data: Some(symbol_data),
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("❌ Minute aggregates failed: {}", e);
            Json(SymbolResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })
        }
    }
}

// Cache Handlers
async fn get_cache_stats(
    State(engine): State<Arc<TradingEngine>>,
) -> Json<CacheStatsResponse> {
    tracing::info!("📊 Cache stats request");

    match engine.get_data_module().get_cache_stats().await {
        Ok(stats) => Json(CacheStatsResponse {
            success: true,
            entries: Some(stats.indicator_entries + stats.market_data_entries),
            message: Some(format!("Cache contains {} entries", stats.indicator_entries + stats.market_data_entries)),
            error: None,
        }),
        Err(e) => {
            tracing::error!("❌ Cache stats failed: {}", e);
            Json(CacheStatsResponse {
                success: false,
                entries: None,
                message: None,
                error: Some(e.to_string()),
            })
        }
    }
}

async fn clean_cache(
    State(engine): State<Arc<TradingEngine>>,
) -> Json<CacheStatsResponse> {
    tracing::info!("📊 Cache cleanup request");

    match engine.get_data_module().clean_cache().await {
        Ok(_) => Json(CacheStatsResponse {
            success: true,
            entries: Some(0),
            message: Some("Cache cleared successfully".to_string()),
            error: None,
        }),
        Err(e) => {
            tracing::error!("❌ Cache cleanup failed: {}", e);
            Json(CacheStatsResponse {
                success: false,
                entries: None,
                message: None,
                error: Some(e.to_string()),
            })
        }
    }
}

// Reference Data Handlers
async fn get_tickers(
    State(engine): State<Arc<TradingEngine>>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let limit = params.get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    
    tracing::info!("📊 All tickers request: limit={}", limit);

    match engine.get_data_module().get_all_tickers(limit).await {
        Ok(tickers) => {
            let data = serde_json::to_value(tickers).unwrap_or(serde_json::Value::Null);
            Json(serde_json::json!({
                "success": true,
                "data": data,
                "error": null
            }))
        }
        Err(e) => {
            tracing::error!("❌ Tickers request failed: {}", e);
            Json(serde_json::json!({
                "success": false,
                "data": null,
                "error": e.to_string()
            }))
        }
    }
}

async fn get_exchanges(
    State(engine): State<Arc<TradingEngine>>,
) -> Json<serde_json::Value> {
    tracing::info!("📊 Stock exchanges request");

    match engine.get_data_module().get_exchanges().await {
        Ok(exchanges) => {
            let data = serde_json::to_value(exchanges).unwrap_or(serde_json::Value::Null);
            Json(serde_json::json!({
                "success": true,
                "data": data,
                "error": null
            }))
        }
        Err(e) => {
            tracing::error!("❌ Exchanges request failed: {}", e);
            Json(serde_json::json!({
                "success": false,
                "data": null,
                "error": e.to_string()
            }))
        }
    }
}

async fn get_splits(
    State(engine): State<Arc<TradingEngine>>,
    Path(symbol): Path<String>,
) -> Json<serde_json::Value> {
    tracing::info!("📊 Stock splits request: {}", symbol);

    match engine.get_data_module().get_stock_splits(&symbol).await {
        Ok(splits) => {
            let data = serde_json::to_value(splits).unwrap_or(serde_json::Value::Null);
            Json(serde_json::json!({
                "success": true,
                "data": data,
                "error": null
            }))
        }
        Err(e) => {
            tracing::error!("❌ Stock splits request failed: {}", e);
            Json(serde_json::json!({
                "success": false,
                "data": null,
                "error": e.to_string()
            }))
        }
    }
}

// News & Fundamentals Handlers
async fn get_news(
    State(engine): State<Arc<TradingEngine>>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<NewsResponse> {
    let ticker = params.get("ticker").cloned();
    let limit = params.get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    
    tracing::info!("📊 News request: ticker={:?} limit={}", ticker, limit);

    match engine.get_data_module().get_news(ticker.as_deref(), limit).await {
        Ok(articles) => {
            let data = serde_json::to_value(articles).unwrap_or(serde_json::Value::Null);
            Json(NewsResponse {
                success: true,
                data: Some(data),
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("❌ News request failed: {}", e);
            Json(NewsResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })
        }
    }
}

async fn get_financials(
    State(engine): State<Arc<TradingEngine>>,
    Path(symbol): Path<String>,
) -> Json<serde_json::Value> {
    tracing::info!("📊 Financials request: {}", symbol);

    match engine.get_data_module().get_financials(&symbol).await {
        Ok(financials) => {
            let data = serde_json::to_value(financials).unwrap_or(serde_json::Value::Null);
            Json(serde_json::json!({
                "success": true,
                "data": data,
                "error": null
            }))
        }
        Err(e) => {
            tracing::error!("❌ Financials request failed: {}", e);
            Json(serde_json::json!({
                "success": false,
                "data": null,
                "error": e.to_string()
            }))
        }
    }
}

// Full Market Snapshot Handler
async fn get_full_market_snapshot(
    State(engine): State<Arc<TradingEngine>>,
) -> Json<FullMarketSnapshotResponse> {
    tracing::info!("📊 Full market snapshot request");

    match engine.get_data_module().get_full_market_snapshot_cached().await {
        Ok(snapshot_data) => {
            let count = snapshot_data.len();
            tracing::info!("📊 Full market snapshot returned {} tickers", count);
            Json(FullMarketSnapshotResponse {
                success: true,
                count: Some(count),
                data: Some(snapshot_data),
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("❌ Full market snapshot failed: {}", e);
            Json(FullMarketSnapshotResponse {
                success: false,
                count: None,
                data: None,
                error: Some(e.to_string()),
            })
        }
    }
}

// Daily Market Summary Handler
async fn get_daily_market_summary(
    State(engine): State<Arc<TradingEngine>>,
    Query(query): Query<DateQuery>,
) -> Json<DailyMarketSummaryResponse> {
    // Default to yesterday if no date provided
    let default_date = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(1))
        .unwrap_or_else(chrono::Utc::now)
        .format("%Y-%m-%d")
        .to_string();
    
    let date = query.date.as_deref().unwrap_or(&default_date);
    
    tracing::info!("📊 Daily market summary request for: {}", date);

    match engine.get_data_module().get_daily_market_summary_cached(date).await {
        Ok(summary_data) => {
            let count = summary_data.len();
            tracing::info!("📊 Daily market summary for {} returned {} tickers", date, count);
            Json(DailyMarketSummaryResponse {
                success: true,
                count: Some(count),
                date: Some(date.to_string()),
                data: Some(summary_data),
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("❌ Daily market summary failed: {}", e);
            Json(DailyMarketSummaryResponse {
                success: false,
                count: None,
                date: Some(date.to_string()),
                data: None,
                error: Some(e.to_string()),
            })
        }
    }
}

// ===============================
// WebSocket API Handlers
// ===============================

// Get WebSocket connection status
async fn get_websocket_status(
    State(engine): State<Arc<TradingEngine>>,
) -> Json<WebSocketStatusResponse> {
    tracing::info!("📡 WebSocket status request");

    let data_module = engine.get_data_module();
    
    match data_module.websocket_status().await {
        Some(status) => {
            let subscriptions = data_module.websocket_subscriptions().await.unwrap_or_default();
            let subscriptions_count = subscriptions.len();
            
            // Determine WebSocket URL based on configuration
            let ws_url = if engine.get_config().polygon_use_delayed_data {
                "wss://delayed.polygon.io/stocks".to_string()
            } else {
                "wss://socket.polygon.io/stocks".to_string()
            };
            
            Json(WebSocketStatusResponse {
                success: true,
                websocket_enabled: true,
                status: Some(format!("{:?}", status)),
                url: Some(ws_url),
                subscriptions_count: Some(subscriptions_count),
                error: None,
            })
        }
        None => {
            Json(WebSocketStatusResponse {
                success: true,
                websocket_enabled: false,
                status: Some("disabled".to_string()),
                url: None,
                subscriptions_count: Some(0),
                error: None,
            })
        }
    }
}

// Subscribe to symbols via WebSocket
async fn websocket_subscribe(
    State(engine): State<Arc<TradingEngine>>,
    Json(payload): Json<SubscribeRequest>,
) -> Json<WebSocketSubscribeResponse> {
    tracing::info!("📡 WebSocket subscribe request for {} symbols: {:?}", payload.symbols.len(), payload.symbols);

    let data_module = engine.get_data_module();
    
    match data_module.websocket_subscribe(payload.symbols.clone()).await {
        Ok(_) => {
            Json(WebSocketSubscribeResponse {
                success: true,
                status: Some("subscribed".to_string()),
                symbols: Some(payload.symbols),
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("❌ WebSocket subscription failed: {}", e);
            Json(WebSocketSubscribeResponse {
                success: false,
                status: Some("failed".to_string()),
                symbols: Some(payload.symbols),
                error: Some(e.to_string()),
            })
        }
    }
}

// Unsubscribe from symbols via WebSocket
async fn websocket_unsubscribe(
    State(engine): State<Arc<TradingEngine>>,
    Json(payload): Json<SubscribeRequest>,
) -> Json<WebSocketSubscribeResponse> {
    tracing::info!("📡 WebSocket unsubscribe request for {} symbols: {:?}", payload.symbols.len(), payload.symbols);

    let data_module = engine.get_data_module();
    
    match data_module.websocket_unsubscribe(payload.symbols.clone()).await {
        Ok(_) => {
            Json(WebSocketSubscribeResponse {
                success: true,
                status: Some("unsubscribed".to_string()),
                symbols: Some(payload.symbols),
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("❌ WebSocket unsubscription failed: {}", e);
            Json(WebSocketSubscribeResponse {
                success: false,
                status: Some("failed".to_string()),
                symbols: Some(payload.symbols),
                error: Some(e.to_string()),
            })
        }
    }
}

// Get current WebSocket subscriptions
async fn get_websocket_subscriptions(
    State(engine): State<Arc<TradingEngine>>,
) -> Json<WebSocketSubscriptionsResponse> {
    tracing::info!("📡 WebSocket subscriptions list request");

    let data_module = engine.get_data_module();
    
    match data_module.websocket_subscriptions().await {
        Ok(subscriptions) => {
            let subscriptions_vec: Vec<String> = subscriptions.into_iter().collect();
            Json(WebSocketSubscriptionsResponse {
                success: true,
                subscriptions: Some(subscriptions_vec),
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("❌ Failed to get WebSocket subscriptions: {}", e);
            Json(WebSocketSubscriptionsResponse {
                success: false,
                subscriptions: None,
                error: Some(e.to_string()),
            })
        }
    }
}

// =====================================
// Portfolio & Position Management
// =====================================

#[derive(Debug, Serialize)]
struct PositionsResponse {
    success: bool,
    positions: Option<HashMap<String, Position>>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct PortfolioResponse {
    success: bool,
    portfolio: Option<PortfolioSummary>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct PortfolioSummary {
    total_value: f64,
    available_cash: f64,
    total_exposure: f64,
    position_count: usize,
    positions: HashMap<String, Position>,
    last_updated: String,
}

async fn get_positions(State(engine): State<Arc<TradingEngine>>) -> Json<PositionsResponse> {
    tracing::info!("📊 Positions request");
    
    let broker_module = engine.get_broker_module().await;
    
    match broker_module.get_positions().await {
        Ok(positions) => {
            tracing::info!("✅ Retrieved {} positions from LightSpeed", positions.len());
            Json(PositionsResponse {
                success: true,
                positions: Some(positions),
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("❌ Failed to get positions: {}", e);
            Json(PositionsResponse {
                success: false,
                positions: None,
                error: Some(e.to_string()),
            })
        }
    }
}

async fn get_portfolio(State(engine): State<Arc<TradingEngine>>) -> Json<PortfolioResponse> {
    tracing::info!("📈 Portfolio request");
    
    let broker_module = engine.get_broker_module().await;
    
    // Get real portfolio data from paper broker with actual position values
    match broker_module.get_portfolio_snapshot().await {
        Ok(portfolio_state) => {
            // Use real portfolio data from paper broker
            let portfolio_summary = PortfolioSummary {
                total_value: portfolio_state.total_value,
                available_cash: portfolio_state.available_cash,
                total_exposure: portfolio_state.total_exposure,
                position_count: portfolio_state.current_position_count as usize,
                positions: portfolio_state.positions,
                last_updated: portfolio_state.last_updated.to_rfc3339(),
            };
            
            tracing::info!("✅ Portfolio from paper broker: ${:.2} total, ${:.2} cash, {} positions", 
                portfolio_summary.total_value, portfolio_summary.available_cash, portfolio_summary.position_count);
            
            Json(PortfolioResponse {
                success: true,
                portfolio: Some(portfolio_summary),
                error: None,
            })
        },
        Err(_) => {
            // Fallback to broker positions if rules engine unavailable
            match broker_module.get_positions().await {
                Ok(positions) => {
                    // Calculate portfolio summary from broker positions
                    let mut position_value = 0.0;
                    let mut total_exposure = 0.0;
                    
                    for position in positions.values() {
                        let value = position.current_price * position.quantity as f64;
                        position_value += value;
                        total_exposure += value;
                    }
                    
                    // Default fallback values when portfolio state unavailable
                    let available_cash = 100000.0 - total_exposure; // Use $100k as documented in paper_sessions
                    
                    let portfolio_summary = PortfolioSummary {
                        total_value: available_cash + position_value,
                        available_cash,
                        total_exposure,
                        position_count: positions.len(),
                        positions: positions.clone(),
                        last_updated: chrono::Utc::now().to_rfc3339(),
                    };
            
            tracing::info!("✅ Portfolio calculated from broker: ${:.2} total, {} positions", 
                portfolio_summary.total_value, portfolio_summary.position_count);
                
            Json(PortfolioResponse {
                success: true,
                portfolio: Some(portfolio_summary),
                error: None,
            })
                }
                Err(e) => {
                    tracing::error!("❌ Failed to get broker positions: {}", e);
                    Json(PortfolioResponse {
                        success: false,
                        portfolio: None,
                        error: Some(e.to_string()),
                    })
                }
            }
        }
    }
}

// =====================================
// Database Reset Management
// =====================================

async fn reset_database(
    State(engine): State<Arc<TradingEngine>>,
) -> Json<ResetDatabaseResponse> {
    tracing::warn!("⚠️ Database reset request received");
    
    // Get paper trading config values from engine
    let config = engine.get_config();
    let initial_cash = config.paper_trading_config.initial_cash;
    let session_id = &config.paper_trading_config.session_id;
    
    match engine.get_database().await {
        Ok(db) => {
            tracing::info!("🔄 Starting database reset process...");
            
            // Begin transaction for safe reset
            let mut tx = match db.begin().await {
                Ok(transaction) => transaction,
                Err(e) => {
                    tracing::error!("❌ Failed to begin transaction: {}", e);
                    return Json(ResetDatabaseResponse {
                        success: false,
                        message: None,
                        error: Some(format!("Transaction failed: {}", e)),
                    });
                }
            };
            
            // Reset all paper trading tables using config values
            let delete_trades = "DELETE FROM paper_trades";
            let delete_positions = "DELETE FROM positions";
            let delete_rules = "DELETE FROM trading_rules_config";
            let delete_snapshots = "DELETE FROM paper_portfolio_snapshots";
            
            let update_session = format!(
                "UPDATE paper_sessions SET 
                    current_cash = {},
                    total_pnl = 0.0,
                    realized_pnl = 0.0,
                    unrealized_pnl = 0.0,
                    open_positions = 0,
                    total_trades = 0,
                    winning_trades = 0,
                    losing_trades = 0,
                    commission_paid = 0.0,
                    max_drawdown = 0.0,
                    updated_at = strftime('%s', 'now'),
                    status = 'ACTIVE'
                    WHERE id = '{}'",
                initial_cash, session_id
            );
            
            let insert_session = format!(
                "INSERT OR IGNORE INTO paper_sessions (
                    id, name, description, initial_cash, current_cash
                ) VALUES (
                    '{}', 'Default Paper Trading Session', 
                    'Default session created for immediate paper trading testing',
                    {}, {}
                )",
                session_id, initial_cash, initial_cash
            );
            
            let reset_queries = [
                delete_trades,
                delete_positions,
                delete_rules,
                delete_snapshots,
                update_session.as_str(),
                insert_session.as_str(),
            ];
            
            for query in &reset_queries {
                if let Err(e) = sqlx::query(query).execute(&mut *tx).await {
                    tracing::error!("❌ Database reset query failed: {}", e);
                    if let Err(rollback_err) = tx.rollback().await {
                        tracing::error!("❌ Failed to rollback transaction: {}", rollback_err);
                    }
                    return Json(ResetDatabaseResponse {
                        success: false,
                        message: None,
                        error: Some(format!("Reset query failed: {}", e)),
                    });
                }
            }
            
            // Commit the transaction
            match tx.commit().await {
                Ok(_) => {
                    tracing::info!("✅ Database reset completed successfully");
                    
                    // Refresh the paper broker's position cache
                    tracing::info!("🔄 Refreshing paper broker position cache...");
                    if let Err(e) = engine.refresh_paper_broker_cache().await {
                        tracing::warn!("⚠️ Failed to refresh paper broker cache: {}", e);
                        // Don't fail the reset for this - it's not critical
                    } else {
                        tracing::info!("✅ Paper broker cache refreshed");
                    }
                    
                    Json(ResetDatabaseResponse {
                        success: true,
                        message: Some("Database has been reset to default state with $100,000 starting cash".to_string()),
                        error: None,
                    })
                }
                Err(e) => {
                    tracing::error!("❌ Failed to commit database reset: {}", e);
                    Json(ResetDatabaseResponse {
                        success: false,
                        message: None,
                        error: Some(format!("Commit failed: {}", e)),
                    })
                }
            }
        }
        Err(e) => {
            tracing::error!("❌ Failed to get database connection for reset: {}", e);
            Json(ResetDatabaseResponse {
                success: false,
                message: None,
                error: Some(format!("Database connection failed: {}", e)),
            })
        }
    }
}

// =====================================
// Risk Configuration Management
// =====================================

// Get current risk configuration
async fn get_risk_config(
    State(engine): State<Arc<TradingEngine>>,
) -> Json<RiskConfigResponse> {
    tracing::info!("🛡️ Risk configuration request");

    match engine.get_database().await {
        Ok(db) => {
            match RiskParameters::load_from_database(&db).await {
                Ok(risk_params) => {
                    tracing::info!("✅ Risk configuration loaded: max_positions={}, max_exposure={}%", 
                        risk_params.max_positions, risk_params.max_portfolio_exposure_percent);
                    Json(RiskConfigResponse {
                        success: true,
                        data: Some(risk_params),
                        error: None,
                    })
                }
                Err(e) => {
                    tracing::error!("❌ Failed to load risk configuration: {}", e);
                    Json(RiskConfigResponse {
                        success: false,
                        data: None,
                        error: Some(e.to_string()),
                    })
                }
            }
        }
        Err(e) => {
            tracing::error!("❌ Failed to get database connection: {}", e);
            Json(RiskConfigResponse {
                success: false,
                data: None,
                error: Some("Database connection failed".to_string()),
            })
        }
    }
}

// Update risk configuration
async fn update_risk_config(
    State(engine): State<Arc<TradingEngine>>,
    Json(request): Json<UpdateRiskRequest>,
) -> Json<UpdateRiskResponse> {
    tracing::info!("🛡️ Risk configuration update request: max_positions={}, max_exposure={}%", 
        request.max_positions, request.max_portfolio_exposure_percent);

    // Validate input parameters
    if request.max_positions == 0 {
        return Json(UpdateRiskResponse {
            success: false,
            message: None,
            error: Some("max_positions must be greater than 0".to_string()),
        });
    }

    if request.max_portfolio_exposure_percent <= 0.0 || request.max_portfolio_exposure_percent > 100.0 {
        return Json(UpdateRiskResponse {
            success: false,
            message: None,
            error: Some("max_portfolio_exposure_percent must be between 0 and 100".to_string()),
        });
    }

    if request.max_single_position_percent <= 0.0 || request.max_single_position_percent > 100.0 {
        return Json(UpdateRiskResponse {
            success: false,
            message: None,
            error: Some("max_single_position_percent must be between 0 and 100".to_string()),
        });
    }

    if request.default_stop_loss_percent <= 0.0 || request.default_stop_loss_percent > 50.0 {
        return Json(UpdateRiskResponse {
            success: false,
            message: None,
            error: Some("default_stop_loss_percent must be between 0 and 50".to_string()),
        });
    }

    // Create new risk parameters
    let new_risk_params = RiskParameters {
        max_positions: request.max_positions,
        max_portfolio_exposure_percent: request.max_portfolio_exposure_percent,
        max_single_position_percent: request.max_single_position_percent,
        default_stop_loss_percent: request.default_stop_loss_percent,
        max_loss_per_trade_dollars: request.max_loss_per_trade_dollars,
        max_daily_loss_dollars: request.max_daily_loss_dollars,
        require_volume_confirmation: request.require_volume_confirmation,
        min_volume_ratio: request.min_volume_ratio,
    };

    match engine.get_database().await {
        Ok(db) => {
            match new_risk_params.save_to_database(&db).await {
                Ok(()) => {
                    tracing::info!("✅ Risk configuration updated successfully");
                    Json(UpdateRiskResponse {
                        success: true,
                        message: Some("Risk configuration updated successfully".to_string()),
                        error: None,
                    })
                }
                Err(e) => {
                    tracing::error!("❌ Failed to save risk configuration: {}", e);
                    Json(UpdateRiskResponse {
                        success: false,
                        message: None,
                        error: Some(e.to_string()),
                    })
                }
            }
        }
        Err(e) => {
            tracing::error!("❌ Failed to get database connection: {}", e);
            Json(UpdateRiskResponse {
                success: false,
                message: None,
                error: Some("Database connection failed".to_string()),
            })
        }
    }
}

// =====================================
// Symbol Watchlist Management
// =====================================

// Get all watchlist symbols
async fn get_watchlist_symbols(
    State(engine): State<Arc<TradingEngine>>,
) -> Json<WatchlistResponse> {
    tracing::info!("📋 Watchlist symbols request");

    match engine.get_database().await {
        Ok(db) => {
            match sqlx::query_as::<_, (i64, String, Option<String>, i64, i64, i64)>(
                "SELECT id, symbol, description, active, added_at, updated_at FROM watchlist_symbols ORDER BY symbol"
            )
            .fetch_all(db.as_ref())
            .await
            {
                Ok(rows) => {
                    let symbols: Vec<WatchlistSymbol> = rows
                        .into_iter()
                        .map(|(id, symbol, description, active, added_at, updated_at)| {
                            WatchlistSymbol {
                                id,
                                symbol,
                                description,
                                active: active != 0,
                                added_at,
                                updated_at,
                            }
                        })
                        .collect();
                    
                    tracing::info!("✅ Retrieved {} watchlist symbols", symbols.len());
                    Json(WatchlistResponse {
                        success: true,
                        data: Some(symbols),
                        error: None,
                    })
                }
                Err(e) => {
                    tracing::error!("❌ Failed to load watchlist symbols: {}", e);
                    Json(WatchlistResponse {
                        success: false,
                        data: None,
                        error: Some(e.to_string()),
                    })
                }
            }
        }
        Err(e) => {
            tracing::error!("❌ Failed to get database connection: {}", e);
            Json(WatchlistResponse {
                success: false,
                data: None,
                error: Some("Database connection failed".to_string()),
            })
        }
    }
}

// Add new symbol to watchlist
async fn add_watchlist_symbol(
    State(engine): State<Arc<TradingEngine>>,
    Json(request): Json<AddSymbolRequest>,
) -> Json<AddSymbolResponse> {
    tracing::info!("📋 Add symbol to watchlist: {}", request.symbol);

    // Validate symbol format (basic validation)
    let symbol = request.symbol.trim().to_uppercase();
    if symbol.is_empty() || symbol.len() > 10 {
        return Json(AddSymbolResponse {
            success: false,
            message: None,
            error: Some("Symbol must be 1-10 characters long".to_string()),
        });
    }

    match engine.get_database().await {
        Ok(db) => {
            match sqlx::query(
                "INSERT INTO watchlist_symbols (symbol, description, active) VALUES (?, ?, 1)"
            )
            .bind(&symbol)
            .bind(&request.description)
            .execute(db.as_ref())
            .await
            {
                Ok(_) => {
                    tracing::info!("✅ Added symbol {} to watchlist", symbol);
                    Json(AddSymbolResponse {
                        success: true,
                        message: Some(format!("Symbol {} added to watchlist", symbol)),
                        error: None,
                    })
                }
                Err(e) => {
                    if e.to_string().contains("UNIQUE constraint failed") {
                        tracing::warn!("⚠️ Symbol {} already exists in watchlist", symbol);
                        Json(AddSymbolResponse {
                            success: false,
                            message: None,
                            error: Some(format!("Symbol {} already exists in watchlist", symbol)),
                        })
                    } else {
                        tracing::error!("❌ Failed to add symbol {}: {}", symbol, e);
                        Json(AddSymbolResponse {
                            success: false,
                            message: None,
                            error: Some(e.to_string()),
                        })
                    }
                }
            }
        }
        Err(e) => {
            tracing::error!("❌ Failed to get database connection: {}", e);
            Json(AddSymbolResponse {
                success: false,
                message: None,
                error: Some("Database connection failed".to_string()),
            })
        }
    }
}

// Delete symbol from watchlist
async fn delete_watchlist_symbol(
    State(engine): State<Arc<TradingEngine>>,
    Path(symbol): Path<String>,
) -> Json<DeleteSymbolResponse> {
    let symbol = symbol.trim().to_uppercase();
    tracing::info!("📋 Delete symbol from watchlist: {}", symbol);

    match engine.get_database().await {
        Ok(db) => {
            match sqlx::query("DELETE FROM watchlist_symbols WHERE symbol = ?")
                .bind(&symbol)
                .execute(db.as_ref())
                .await
            {
                Ok(result) => {
                    if result.rows_affected() > 0 {
                        tracing::info!("✅ Removed symbol {} from watchlist", symbol);
                        Json(DeleteSymbolResponse {
                            success: true,
                            message: Some(format!("Symbol {} removed from watchlist", symbol)),
                            error: None,
                        })
                    } else {
                        tracing::warn!("⚠️ Symbol {} not found in watchlist", symbol);
                        Json(DeleteSymbolResponse {
                            success: false,
                            message: None,
                            error: Some(format!("Symbol {} not found in watchlist", symbol)),
                        })
                    }
                }
                Err(e) => {
                    tracing::error!("❌ Failed to delete symbol {}: {}", symbol, e);
                    Json(DeleteSymbolResponse {
                        success: false,
                        message: None,
                        error: Some(e.to_string()),
                    })
                }
            }
        }
        Err(e) => {
            tracing::error!("❌ Failed to get database connection: {}", e);
            Json(DeleteSymbolResponse {
                success: false,
                message: None,
                error: Some("Database connection failed".to_string()),
            })
        }
    }
}

// Activate symbol in watchlist
async fn activate_watchlist_symbol(
    State(engine): State<Arc<TradingEngine>>,
    Path(symbol): Path<String>,
) -> Json<AddSymbolResponse> {
    let symbol = symbol.trim().to_uppercase();
    tracing::info!("📋 Activate symbol in watchlist: {}", symbol);

    match engine.get_database().await {
        Ok(db) => {
            match sqlx::query("UPDATE watchlist_symbols SET active = 1 WHERE symbol = ?")
                .bind(&symbol)
                .execute(db.as_ref())
                .await
            {
                Ok(result) => {
                    if result.rows_affected() > 0 {
                        tracing::info!("✅ Activated symbol {} in watchlist", symbol);
                        Json(AddSymbolResponse {
                            success: true,
                            message: Some(format!("Symbol {} activated in watchlist", symbol)),
                            error: None,
                        })
                    } else {
                        tracing::warn!("⚠️ Symbol {} not found in watchlist", symbol);
                        Json(AddSymbolResponse {
                            success: false,
                            message: None,
                            error: Some(format!("Symbol {} not found in watchlist", symbol)),
                        })
                    }
                }
                Err(e) => {
                    tracing::error!("❌ Failed to activate symbol {}: {}", symbol, e);
                    Json(AddSymbolResponse {
                        success: false,
                        message: None,
                        error: Some(e.to_string()),
                    })
                }
            }
        }
        Err(e) => {
            tracing::error!("❌ Failed to get database connection: {}", e);
            Json(AddSymbolResponse {
                success: false,
                message: None,
                error: Some("Database connection failed".to_string()),
            })
        }
    }
}

// Deactivate symbol in watchlist
async fn deactivate_watchlist_symbol(
    State(engine): State<Arc<TradingEngine>>,
    Path(symbol): Path<String>,
) -> Json<AddSymbolResponse> {
    let symbol = symbol.trim().to_uppercase();
    tracing::info!("📋 Deactivate symbol in watchlist: {}", symbol);

    match engine.get_database().await {
        Ok(db) => {
            match sqlx::query("UPDATE watchlist_symbols SET active = 0 WHERE symbol = ?")
                .bind(&symbol)
                .execute(db.as_ref())
                .await
            {
                Ok(result) => {
                    if result.rows_affected() > 0 {
                        tracing::info!("✅ Deactivated symbol {} in watchlist", symbol);
                        Json(AddSymbolResponse {
                            success: true,
                            message: Some(format!("Symbol {} deactivated in watchlist", symbol)),
                            error: None,
                        })
                    } else {
                        tracing::warn!("⚠️ Symbol {} not found in watchlist", symbol);
                        Json(AddSymbolResponse {
                            success: false,
                            message: None,
                            error: Some(format!("Symbol {} not found in watchlist", symbol)),
                        })
                    }
                }
                Err(e) => {
                    tracing::error!("❌ Failed to deactivate symbol {}: {}", symbol, e);
                    Json(AddSymbolResponse {
                        success: false,
                        message: None,
                        error: Some(e.to_string()),
                    })
                }
            }
        }
        Err(e) => {
            tracing::error!("❌ Failed to get database connection: {}", e);
            Json(AddSymbolResponse {
                success: false,
                message: None,
                error: Some("Database connection failed".to_string()),
            })
        }
    }
}