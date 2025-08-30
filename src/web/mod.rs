// src/web/mod.rs
use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashMap;
use crate::engine::TradingEngine;
use crate::data::{SymbolDataResponse, MarketMoverData, DailyBar, MinuteBar};

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


pub async fn start_server(bind_address: String, engine: Arc<TradingEngine>) -> Result<()> {
    let app = Router::new()
        .route("/", get(root))
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
        .route("/api/market/movers/gainers", get(get_market_gainers))
        .route("/api/market/movers/losers", get(get_market_losers))
        .route("/api/market/status", get(get_market_status))
        .route("/api/market/previous/:symbol", get(get_previous_day))
        .route("/api/market/minute/:symbol", get(get_minute_aggregates))
        // News & Cache
        // .route("/api/news", get(get_news)) // TODO: implement news endpoint
        .route("/api/cache/stats", get(get_cache_stats))
        .route("/api/cache/clean", post(clean_cache))
        .with_state(engine);

    let listener = tokio::net::TcpListener::bind(&bind_address).await?;
    
    tracing::info!("Web server listening on {}", bind_address);
    
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn root() -> &'static str {
    r#"<!DOCTYPE html>
<html>
<head>
    <title>Trading System</title>
    <style>
        body { font-family: Arial, sans-serif; max-width: 600px; margin: 50px auto; padding: 20px; }
        .status { background: #e8f5e8; padding: 15px; margin: 20px 0; border-radius: 8px; border-left: 4px solid #4caf50; }
        .form { background: #f9f9f9; padding: 20px; border-radius: 8px; margin: 20px 0; }
        .form-group { margin: 15px 0; }
        label { display: block; margin-bottom: 5px; font-weight: bold; }
        input, select { width: 100%; padding: 8px; border: 1px solid #ddd; border-radius: 4px; box-sizing: border-box; }
        button { background: #007cba; color: white; padding: 10px 20px; border: none; border-radius: 4px; cursor: pointer; margin: 10px 5px 0 0; }
        button:hover { background: #005a8b; }
        .result { margin: 20px 0; padding: 15px; border-radius: 8px; }
        .success { background: #d4edda; border: 1px solid #c3e6cb; color: #155724; }
        .error { background: #f8d7da; border: 1px solid #f5c6cb; color: #721c24; }
        .api-info { background: #e3f2fd; padding: 15px; border-radius: 8px; margin: 20px 0; }
        code { background: #f4f4f4; padding: 2px 4px; border-radius: 3px; }
    </style>
</head>
<body>
    <h1>🚀 Trading System</h1>
    
    <div class="status">
        <h3>✅ System Status: Online</h3>
        <p>LightSpeed broker connection established and authenticated.</p>
        <button onclick="checkStatus()">Check Status</button>
    </div>

    <div class="form">
        <h3>📊 Place Test Order</h3>
        <form onsubmit="placeOrder(event)">
            <div class="form-group">
                <label>Symbol:</label>
                <input type="text" id="symbol" value="AAPL" required>
            </div>
            <div class="form-group">
                <label>Side:</label>
                <select id="side" required>
                    <option value="BUY">BUY</option>
                    <option value="SELL">SELL</option>
                    <option value="SELL_SHORT">SELL_SHORT</option>
                </select>
            </div>
            <div class="form-group">
                <label>Order Type:</label>
                <select id="orderType" required onchange="togglePrice()">
                    <option value="LIMIT">LIMIT</option>
                    <option value="MARKET">MARKET</option>
                </select>
            </div>
            <div class="form-group">
                <label>Quantity:</label>
                <input type="number" id="quantity" value="1" min="1" required>
            </div>
            <div class="form-group" id="priceGroup">
                <label>Price:</label>
                <input type="number" id="price" value="150.00" step="0.01" min="0">
            </div>
            <button type="submit">Place Order</button>
        </form>
    </div>

    <div id="result"></div>

    <div class="api-info">
        <h3>🔧 API Usage</h3>
        <p><strong>Symbol Lookup:</strong> <code>POST /api/symbol</code></p>
        <p><strong>Place Order:</strong> <code>POST /api/order</code></p>
        <p><strong>Check Status:</strong> <code>GET /api/status</code></p>
        <p><strong>Example curl commands:</strong></p>
        <p><code>curl -X POST http://localhost:8080/api/symbol -H "Content-Type: application/json" -d '{"symbol":"AMZN"}'</code></p>
        <p><code>curl -X POST http://localhost:8080/api/order -H "Content-Type: application/json" -d '{"symbol":"AAPL","side":"BUY","order_type":"LIMIT","quantity":1,"price":150.0}'</code></p>
    </div>

    <script>
        function togglePrice() {
            const orderType = document.getElementById('orderType').value;
            const priceGroup = document.getElementById('priceGroup');
            priceGroup.style.display = orderType === 'MARKET' ? 'none' : 'block';
        }

        async function checkStatus() {
            try {
                const response = await fetch('/api/status');
                const data = await response.json();
                showResult(data.message, 'success');
            } catch (error) {
                showResult('Failed to check status: ' + error.message, 'error');
            }
        }

        async function placeOrder(event) {
            event.preventDefault();
            
            const symbol = document.getElementById('symbol').value;
            const side = document.getElementById('side').value;
            const orderType = document.getElementById('orderType').value;
            const quantity = parseInt(document.getElementById('quantity').value);
            const price = orderType === 'MARKET' ? null : parseFloat(document.getElementById('price').value);

            const orderData = {
                symbol: symbol,
                side: side,
                order_type: orderType,
                quantity: quantity,
                price: price
            };

            try {
                const response = await fetch('/api/order', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify(orderData)
                });

                const result = await response.json();
                
                if (result.success) {
                    showResult(`✅ Order placed successfully! Order ID: ${result.order_id}`, 'success');
                } else {
                    showResult(`❌ Order failed: ${result.error}`, 'error');
                }
            } catch (error) {
                showResult(`❌ Request failed: ${error.message}`, 'error');
            }
        }

        function showResult(message, type) {
            const resultDiv = document.getElementById('result');
            resultDiv.className = `result ${type}`;
            resultDiv.innerHTML = message;
        }
    </script>
</body>
</html>"#
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
            let status = if response.market == Some("open".to_string()) {
                "OPEN".to_string()
            } else {
                "CLOSED".to_string()
            };
            Json(MarketStatusResponse {
                success: true,
                status: Some(status),
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