# Complete Polygon WebSocket Implementation Plan

## Architecture Overview

**Hybrid Design**: WebSocket streams run concurrently with REST API calls, both feeding the same high-performance memory cache for <5ms decision pipeline.

### Key URLs & Configuration
- **Delayed**: `wss://delayed.polygon.io/stocks` (Stock Starter plan, 15-min delay)
- **Real-Time**: `wss://socket.polygon.io/stocks` (Advanced/Pro plans, <20ms latency)
- **Control**: `POLYGON_USE_DELAYED_DATA` environment variable switches URLs
- **REST API**: Continues using `https://api.polygon.io` with status-based delay handling

## 1. Environment Configuration Updates

### .env.example Additions
```bash
# Polygon.io API Configuration
POLYGON_API_KEY=your_polygon_api_key_here
POLYGON_BASE_URL=https://api.polygon.io
# Set to true for Stock Starter plan (15-min delayed), false for real-time plans
POLYGON_USE_DELAYED_DATA=true

# WebSocket Configuration
POLYGON_ENABLE_WEBSOCKET=true
POLYGON_WEBSOCKET_RECONNECT_INTERVAL=5
POLYGON_WEBSOCKET_HEARTBEAT_INTERVAL=30
POLYGON_WEBSOCKET_MAX_SUBSCRIPTIONS=100
POLYGON_WEBSOCKET_BUFFER_SIZE=1000

# WebSocket Subscription Defaults
POLYGON_DEFAULT_SUBSCRIPTIONS=AAPL,TSLA,MSFT,GOOGL,AMZN
POLYGON_AUTO_SUBSCRIBE_MOVERS=true
```

### Config Module Extensions (src/config/mod.rs)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // ... existing fields ...
    pub polygon_websocket_config: Option<PolygonWebSocketConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolygonWebSocketConfig {
    pub enabled: bool,
    pub reconnect_interval_secs: u64,
    pub heartbeat_interval_secs: u64,
    pub max_subscriptions: usize,
    pub buffer_size: usize,
    pub default_subscriptions: Vec<String>,
    pub auto_subscribe_movers: bool,
}

impl Config {
    pub fn load() -> Result<Self> {
        // ... existing code ...
        
        polygon_websocket_config: if polygon_api_key.is_some() {
            Some(PolygonWebSocketConfig {
                enabled: env::var("POLYGON_ENABLE_WEBSOCKET")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse().unwrap_or(true),
                reconnect_interval_secs: env::var("POLYGON_WEBSOCKET_RECONNECT_INTERVAL")
                    .unwrap_or_else(|_| "5".to_string())
                    .parse().unwrap_or(5),
                heartbeat_interval_secs: env::var("POLYGON_WEBSOCKET_HEARTBEAT_INTERVAL")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse().unwrap_or(30),
                max_subscriptions: env::var("POLYGON_WEBSOCKET_MAX_SUBSCRIPTIONS")
                    .unwrap_or_else(|_| "100".to_string())
                    .parse().unwrap_or(100),
                buffer_size: env::var("POLYGON_WEBSOCKET_BUFFER_SIZE")
                    .unwrap_or_else(|_| "1000".to_string())
                    .parse().unwrap_or(1000),
                default_subscriptions: env::var("POLYGON_DEFAULT_SUBSCRIPTIONS")
                    .unwrap_or_else(|_| "AAPL,TSLA,MSFT".to_string())
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect(),
                auto_subscribe_movers: env::var("POLYGON_AUTO_SUBSCRIBE_MOVERS")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse().unwrap_or(true),
            })
        } else {
            None
        },
    }
}
```

## 2. WebSocket Module Architecture (src/data/websocket.rs)

```rust
// src/data/websocket.rs
// Branch: 8.28.25.1

use anyhow::{Context, Result};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info, warn};

use crate::config::Config;

pub struct PolygonWebSocket {
    ws_url: String,
    api_key: String,
    use_delayed_data: bool,
    connection: Option<WebSocketConnection>,
    subscriptions: Arc<RwLock<HashSet<String>>>,
    message_buffer: Arc<RwLock<VecDeque<WebSocketMessage>>>,
    connection_status: Arc<RwLock<ConnectionStatus>>,
    config: PolygonWebSocketConfig,
    // Channel for sending messages to WebSocket task
    command_tx: Option<mpsc::UnboundedSender<WebSocketCommand>>,
}

#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Authenticated,
    Error(String),
}

#[derive(Debug, Clone)]
pub enum WebSocketCommand {
    Subscribe(Vec<String>),
    Unsubscribe(Vec<String>),
    Reconnect,
    Shutdown,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "ev")]
pub enum WebSocketMessage {
    #[serde(rename = "AM")]
    AggregateMinute {
        sym: String,           // Symbol
        o: f64,               // Open
        h: f64,               // High  
        l: f64,               // Low
        c: f64,               // Close
        v: f64,               // Volume
        vw: f64,              // Volume weighted average price
        t: i64,               // Timestamp
        n: u32,               // Number of transactions
    },
    #[serde(rename = "A")]
    AggregateSecond {
        sym: String,
        o: f64, h: f64, l: f64, c: f64,
        v: f64, vw: f64, t: i64, n: u32,
    },
    #[serde(rename = "T")]
    Trade {
        sym: String,          // Symbol
        p: f64,              // Price
        s: f64,              // Size
        x: i32,              // Exchange ID
        t: i64,              // Timestamp
        c: Vec<i32>,         // Conditions
    },
    #[serde(rename = "Q")]
    Quote {
        sym: String,          // Symbol
        bp: f64,             // Bid price
        ap: f64,             // Ask price
        bs: f64,             // Bid size
        as_: f64,            // Ask size
        t: i64,              // Timestamp
    },
    #[serde(rename = "status")]
    Status {
        status: String,
        message: String,
    },
    #[serde(other)]
    Unknown,
}

impl PolygonWebSocket {
    pub fn new(config: &Config) -> Result<Self> {
        let ws_config = config.polygon_websocket_config
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("WebSocket configuration missing"))?;

        let ws_url = if config.polygon_use_delayed_data {
            "wss://delayed.polygon.io/stocks".to_string()
        } else {
            "wss://socket.polygon.io/stocks".to_string()
        };
        
        if config.polygon_use_delayed_data {
            info!("🔗 WebSocket will connect to delayed data feed (15-min delay)");
        } else {
            info!("🔗 WebSocket will connect to real-time data feed");
        }

        Ok(PolygonWebSocket {
            ws_url,
            api_key: config.polygon_api_key.clone().unwrap(),
            use_delayed_data: config.polygon_use_delayed_data,
            connection: None,
            subscriptions: Arc::new(RwLock::new(HashSet::new())),
            message_buffer: Arc::new(RwLock::new(VecDeque::new())),
            connection_status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            config: ws_config.clone(),
            command_tx: None,
        })
    }

    /// Start WebSocket connection and message processing in background task
    pub async fn start(&mut self) -> Result<()> {
        if !self.config.enabled {
            info!("📡 WebSocket disabled in configuration");
            return Ok(());
        }

        let (command_tx, command_rx) = mpsc::unbounded_channel();
        self.command_tx = Some(command_tx);

        // Spawn background task for WebSocket management
        let ws_task = WebSocketTask::new(
            self.ws_url.clone(),
            self.api_key.clone(),
            self.config.clone(),
            command_rx,
            self.connection_status.clone(),
            self.subscriptions.clone(),
            self.message_buffer.clone(),
        );

        tokio::spawn(async move {
            ws_task.run().await;
        });

        info!("📡 WebSocket background task started");
        Ok(())
    }

    /// Subscribe to symbols (non-blocking)
    pub async fn subscribe(&self, symbols: Vec<String>) -> Result<()> {
        if let Some(ref tx) = self.command_tx {
            tx.send(WebSocketCommand::Subscribe(symbols))
                .context("Failed to send subscribe command")?;
        }
        Ok(())
    }

    /// Get connection status
    pub async fn get_status(&self) -> ConnectionStatus {
        self.connection_status.read().await.clone()
    }

    /// Get recent messages from buffer
    pub async fn get_recent_messages(&self, limit: usize) -> Vec<WebSocketMessage> {
        let buffer = self.message_buffer.read().await;
        buffer.iter().rev().take(limit).cloned().collect()
    }
}
```

## 3. Integration with Existing Cache System (src/data/mod.rs)

```rust
// Add to DataModule struct
pub struct DataModule {
    // ... existing fields ...
    websocket: Option<PolygonWebSocket>,
    realtime_cache: Arc<RwLock<HashMap<String, RealtimeTickerData>>>,
}

#[derive(Debug, Clone)]
struct RealtimeTickerData {
    symbol: String,
    last_price: f64,
    last_volume: f64,
    bid: Option<f64>,
    ask: Option<f64>,
    last_trade_time: i64,
    cached_at: Instant,
}

impl DataModule {
    pub fn new(config: &Config) -> Result<Self> {
        // ... existing initialization ...
        
        let websocket = if config.is_polygon_enabled() && 
                          config.polygon_websocket_config.as_ref().map(|c| c.enabled).unwrap_or(false) {
            Some(PolygonWebSocket::new(config)?)
        } else {
            None
        };

        Ok(DataModule {
            // ... existing fields ...
            websocket,
            realtime_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Start WebSocket connections
    pub async fn start_websocket(&mut self) -> Result<()> {
        if let Some(ref mut ws) = self.websocket {
            ws.start().await?;
            
            // Subscribe to default symbols
            if let Some(config) = &ws.config {
                if !config.default_subscriptions.is_empty() {
                    ws.subscribe(config.default_subscriptions.clone()).await?;
                }
            }
        }
        Ok(())
    }

    /// Process WebSocket messages and update cache (called periodically)
    pub async fn process_websocket_messages(&self) -> Result<()> {
        if let Some(ref ws) = self.websocket {
            let messages = ws.get_recent_messages(100).await;
            
            for message in messages {
                self.update_cache_from_websocket(message).await?;
            }
        }
        Ok(())
    }

    async fn update_cache_from_websocket(&self, message: WebSocketMessage) -> Result<()> {
        match message {
            WebSocketMessage::AggregateMinute { sym, c, v, t, .. } => {
                let mut cache = self.realtime_cache.write().await;
                cache.insert(sym.clone(), RealtimeTickerData {
                    symbol: sym,
                    last_price: c,
                    last_volume: v,
                    bid: None,
                    ask: None,
                    last_trade_time: t,
                    cached_at: Instant::now(),
                });
            },
            WebSocketMessage::Quote { sym, bp, ap, t, .. } => {
                let mut cache = self.realtime_cache.write().await;
                if let Some(data) = cache.get_mut(&sym) {
                    data.bid = Some(bp);
                    data.ask = Some(ap);
                    data.cached_at = Instant::now();
                }
            },
            _ => {} // Handle other message types
        }
        Ok(())
    }
}
```

## 4. REST API Extensions (src/web/mod.rs)

### New WebSocket Management Endpoints

```rust
// GET /api/websocket/status - Check WebSocket connection status  
async fn websocket_status(data: web::Data<Arc<DataModule>>) -> Result<impl Responder, Error> {
    if let Some(ref ws) = data.websocket {
        let status = ws.get_status().await;
        Ok(web::Json(json!({
            "websocket_enabled": true,
            "status": format!("{:?}", status),
            "url": ws.ws_url
        })))
    } else {
        Ok(web::Json(json!({
            "websocket_enabled": false,
            "status": "disabled"
        })))
    }
}

// POST /api/websocket/subscribe - Subscribe to symbols
async fn websocket_subscribe(
    data: web::Data<Arc<DataModule>>,
    payload: web::Json<SubscribeRequest>
) -> Result<impl Responder, Error> {
    if let Some(ref ws) = data.websocket {
        ws.subscribe(payload.symbols.clone()).await
            .map_err(|e| error::ErrorInternalServerError(e))?;
        Ok(web::Json(json!({"status": "subscribed", "symbols": payload.symbols})))
    } else {
        Ok(web::Json(json!({"error": "WebSocket not enabled"})))
    }
}

#[derive(Deserialize)]
struct SubscribeRequest {
    symbols: Vec<String>,
}
```

## 5. Testing Strategy

### CURL Commands for WebSocket Testing

```bash
#!/bin/bash
# WebSocket Integration Tests

echo "=== WebSocket Status Test ==="
curl -s http://localhost:8080/api/websocket/status | jq '.'

echo "=== Subscribe to Symbols Test ==="
curl -s -X POST http://localhost:8080/api/websocket/subscribe \
  -H "Content-Type: application/json" \
  -d '{"symbols": ["AAPL", "TSLA", "MSFT"]}' | jq '.'

echo "=== Test Hybrid Data Access (REST + WebSocket) ==="
curl -s http://localhost:8080/api/snapshot/AAPL | jq '.'

echo "=== Test Cache Performance ==="
time curl -s http://localhost:8080/api/snapshot/AAPL > /dev/null
time curl -s http://localhost:8080/api/snapshot/AAPL > /dev/null
time curl -s http://localhost:8080/api/snapshot/AAPL > /dev/null
```

### test_api.sh Integration

```bash
# Add to existing test_api.sh
test_websocket_integration() {
    echo "Testing WebSocket integration..."
    
    # Test connection status
    response=$(curl -s "$BASE_URL/api/websocket/status")
    if echo "$response" | jq -e '.websocket_enabled' > /dev/null; then
        echo "✅ WebSocket status endpoint working"
    else
        echo "❌ WebSocket status endpoint failed"
        return 1
    fi
    
    # Test subscription
    response=$(curl -s -X POST "$BASE_URL/api/websocket/subscribe" \
        -H "Content-Type: application/json" \
        -d '{"symbols": ["AAPL", "TSLA"]}')
    if echo "$response" | jq -e '.status' > /dev/null; then
        echo "✅ WebSocket subscription working"
    else
        echo "❌ WebSocket subscription failed"
        return 1
    fi
    
    # Test that REST API still works with WebSocket running
    test_single_ticker_snapshot "AAPL"
    
    echo "✅ WebSocket integration tests passed"
    return 0
}
```

## 6. Docker Configuration

### Cargo.toml Dependencies
```toml
[dependencies]
# ... existing dependencies ...
tokio-tungstenite = "0.20"
futures-util = "0.3"
tungstenite = "0.20"
```

### Docker Commands for Development
```bash
# WebSocket development and testing
docker compose exec trading-system cargo build
docker compose exec trading-system cargo test websocket
docker compose exec trading-system cargo test --release  # Performance testing

# Monitor WebSocket connections
docker compose logs -f trading-system | grep -E "(WebSocket|🔗|📡)"
```

## 7. Performance Architecture

### <5ms Decision Pipeline
1. **WebSocket Message → Cache**: <1ms (background task, non-blocking)
2. **Cache Lookup**: <1ms (Arc<RwLock> memory access)  
3. **Rule Evaluation**: <1ms (using cached data)
4. **Decision Pipeline**: <2ms (order preparation)

### Concurrent Operations
- **WebSocket Task**: Runs independently, updates cache continuously
- **REST API**: Serves from cache instantly, never blocked by WebSocket
- **Cache Updates**: Atomic operations using Arc<RwLock>
- **Background Cleanup**: Removes stale cache entries

## 8. Error Handling & Reliability

```rust
// Automatic reconnection logic
impl WebSocketTask {
    async fn handle_connection_error(&mut self, error: &str) {
        error!("WebSocket connection error: {}", error);
        
        // Update status
        *self.connection_status.write().await = ConnectionStatus::Error(error.to_string());
        
        // Wait before reconnecting
        tokio::time::sleep(Duration::from_secs(self.config.reconnect_interval_secs)).await;
        
        // Attempt reconnection
        if let Err(e) = self.connect_and_authenticate().await {
            error!("Reconnection failed: {}", e);
        } else {
            // Re-subscribe to previous symbols
            self.resubscribe_all().await;
        }
    }
}
```

## 9. Plan Summary

**Architecture Benefits:**
- Non-blocking: WebSocket streams don't interrupt REST API calls
- Unified Cache: Both REST and WebSocket feed same high-performance cache  
- URL Switching: Correct delayed vs real-time URL handling
- Docker Ready: All development in containers
- Production Ready: Error handling, reconnection, monitoring
- Performance: <5ms decision pipeline maintained
- Configuration: Complete .env.example with WebSocket controls