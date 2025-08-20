// src/web/mod.rs
use anyhow::Result;
use axum::{
    extract::State,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::engine::TradingEngine;

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

pub async fn start_server(bind_address: String, engine: Arc<TradingEngine>) -> Result<()> {
    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/api/status", get(get_status))
        .route("/api/order", post(place_order))
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
        <p><strong>Place Order:</strong> <code>POST /api/order</code></p>
        <p><strong>Check Status:</strong> <code>GET /api/status</code></p>
        <p><strong>Example curl:</strong></p>
        <code>curl -X POST http://localhost:8080/api/order -H "Content-Type: application/json" -d '{"symbol":"AAPL","side":"BUY","order_type":"LIMIT","quantity":1,"price":150.0}'</code>
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