// src/data/websocket.rs
// Branch: 8.28.25.1

use anyhow::{Context, Result};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info, warn};

use crate::config::{Config, PolygonWebSocketConfig};

#[derive(Debug)]
pub struct PolygonWebSocket {
    ws_url: String,
    api_key: String,
    use_delayed_data: bool,
    subscriptions: Arc<RwLock<HashSet<String>>>,
    message_buffer: Arc<RwLock<VecDeque<WebSocketMessage>>>,
    connection_status: Arc<RwLock<ConnectionStatus>>,
    config: PolygonWebSocketConfig,
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
        #[serde(rename = "as")]
        ask_size: f64,       // Ask size (renamed to avoid keyword)
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

#[derive(Debug, Deserialize)]
struct AuthMessage {
    action: String,
    params: String,
}

#[derive(Debug, Deserialize)]
struct SubscribeMessage {
    action: String,
    params: String,
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

    /// Unsubscribe from symbols (non-blocking)
    pub async fn unsubscribe(&self, symbols: Vec<String>) -> Result<()> {
        if let Some(ref tx) = self.command_tx {
            tx.send(WebSocketCommand::Unsubscribe(symbols))
                .context("Failed to send unsubscribe command")?;
        }
        Ok(())
    }

    /// Get connection status
    pub async fn get_status(&self) -> ConnectionStatus {
        self.connection_status.read().await.clone()
    }

    /// Get current subscriptions
    pub async fn get_subscriptions(&self) -> HashSet<String> {
        self.subscriptions.read().await.clone()
    }

    /// Get recent messages from buffer
    pub async fn get_recent_messages(&self, limit: usize) -> Vec<WebSocketMessage> {
        let buffer = self.message_buffer.read().await;
        buffer.iter().rev().take(limit).cloned().collect()
    }

    /// Force reconnection
    pub async fn reconnect(&self) -> Result<()> {
        if let Some(ref tx) = self.command_tx {
            tx.send(WebSocketCommand::Reconnect)
                .context("Failed to send reconnect command")?;
        }
        Ok(())
    }

    /// Shutdown WebSocket connection
    pub async fn shutdown(&self) -> Result<()> {
        if let Some(ref tx) = self.command_tx {
            tx.send(WebSocketCommand::Shutdown)
                .context("Failed to send shutdown command")?;
        }
        Ok(())
    }
}

struct WebSocketTask {
    ws_url: String,
    api_key: String,
    config: PolygonWebSocketConfig,
    command_rx: mpsc::UnboundedReceiver<WebSocketCommand>,
    connection_status: Arc<RwLock<ConnectionStatus>>,
    subscriptions: Arc<RwLock<HashSet<String>>>,
    message_buffer: Arc<RwLock<VecDeque<WebSocketMessage>>>,
}

impl WebSocketTask {
    fn new(
        ws_url: String,
        api_key: String,
        config: PolygonWebSocketConfig,
        command_rx: mpsc::UnboundedReceiver<WebSocketCommand>,
        connection_status: Arc<RwLock<ConnectionStatus>>,
        subscriptions: Arc<RwLock<HashSet<String>>>,
        message_buffer: Arc<RwLock<VecDeque<WebSocketMessage>>>,
    ) -> Self {
        Self {
            ws_url,
            api_key,
            config,
            command_rx,
            connection_status,
            subscriptions,
            message_buffer,
        }
    }

    async fn run(mut self) {
        info!("📡 WebSocket task starting connection to {}", self.ws_url);
        
        loop {
            // Set connecting status
            *self.connection_status.write().await = ConnectionStatus::Connecting;
            
            match self.connect_and_run().await {
                Ok(_) => {
                    info!("📡 WebSocket connection ended normally");
                },
                Err(e) => {
                    error!("📡 WebSocket connection error: {}", e);
                    *self.connection_status.write().await = ConnectionStatus::Error(e.to_string());
                }
            }
            
            // Wait before reconnecting
            tokio::time::sleep(Duration::from_secs(self.config.reconnect_interval_secs)).await;
            info!("📡 Attempting WebSocket reconnection...");
        }
    }

    async fn connect_and_run(&mut self) -> Result<()> {
        // Connect to WebSocket
        let (ws_stream, _) = connect_async(&self.ws_url).await
            .context("Failed to connect to WebSocket")?;
        
        info!("📡 WebSocket connected to {}", self.ws_url);
        *self.connection_status.write().await = ConnectionStatus::Connected;

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        // Send authentication message
        let auth_msg = serde_json::json!({
            "action": "auth",
            "params": self.api_key
        });
        
        ws_sender.send(Message::Text(auth_msg.to_string())).await
            .context("Failed to send auth message")?;
        
        debug!("📡 Authentication message sent");
        *self.connection_status.write().await = ConnectionStatus::Authenticated;

        // Subscribe to default symbols
        let default_subs = self.config.default_subscriptions.clone();
        if !default_subs.is_empty() {
            self.send_subscription(&mut ws_sender, &default_subs, "subscribe").await?;
        }

        // Main message processing loop
        loop {
            tokio::select! {
                // Handle incoming WebSocket messages
                msg = ws_receiver.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            if let Err(e) = self.handle_message(&text).await {
                                warn!("📡 Error handling WebSocket message: {}", e);
                            }
                        }
                        Some(Ok(Message::Close(_))) => {
                            info!("📡 WebSocket connection closed by server");
                            break;
                        }
                        Some(Err(e)) => {
                            error!("📡 WebSocket error: {}", e);
                            break;
                        }
                        None => {
                            warn!("📡 WebSocket stream ended");
                            break;
                        }
                        _ => {} // Ignore other message types
                    }
                }
                
                // Handle commands from the main thread
                cmd = self.command_rx.recv() => {
                    match cmd {
                        Some(WebSocketCommand::Subscribe(symbols)) => {
                            if let Err(e) = self.send_subscription(&mut ws_sender, &symbols, "subscribe").await {
                                error!("📡 Failed to subscribe: {}", e);
                            }
                        }
                        Some(WebSocketCommand::Unsubscribe(symbols)) => {
                            if let Err(e) = self.send_subscription(&mut ws_sender, &symbols, "unsubscribe").await {
                                error!("📡 Failed to unsubscribe: {}", e);
                            }
                        }
                        Some(WebSocketCommand::Reconnect) => {
                            info!("📡 Manual reconnect requested");
                            break;
                        }
                        Some(WebSocketCommand::Shutdown) => {
                            info!("📡 WebSocket shutdown requested");
                            return Ok(());
                        }
                        None => {
                            warn!("📡 Command channel closed");
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn send_subscription(
        &mut self, 
        ws_sender: &mut futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>,
        symbols: &[String],
        action: &str
    ) -> Result<()> {
        // Format subscription parameters
        let mut params = Vec::new();
        for symbol in symbols {
            params.push(format!("AM.{}", symbol)); // Subscribe to minute aggregates
        }
        
        let sub_msg = serde_json::json!({
            "action": action,
            "params": params.join(",")
        });
        
        ws_sender.send(Message::Text(sub_msg.to_string())).await
            .context("Failed to send subscription message")?;
        
        // Update subscription tracking
        let mut subs = self.subscriptions.write().await;
        match action {
            "subscribe" => {
                for symbol in symbols {
                    subs.insert(symbol.clone());
                }
                info!("📡 Subscribed to {} symbols: {:?}", symbols.len(), symbols);
            }
            "unsubscribe" => {
                for symbol in symbols {
                    subs.remove(symbol);
                }
                info!("📡 Unsubscribed from {} symbols: {:?}", symbols.len(), symbols);
            }
            _ => {}
        }
        
        Ok(())
    }

    async fn handle_message(&mut self, text: &str) -> Result<()> {
        debug!("📡 Received WebSocket message: {}", text);
        
        // Try to parse as WebSocket message
        if let Ok(ws_msg) = serde_json::from_str::<WebSocketMessage>(text) {
            // Add to message buffer with size limit
            let mut buffer = self.message_buffer.write().await;
            buffer.push_back(ws_msg);
            
            // Keep buffer size within limits
            while buffer.len() > self.config.buffer_size {
                buffer.pop_front();
            }
        } else {
            // Handle status messages and other responses
            debug!("📡 Non-data message received: {}", text);
        }
        
        Ok(())
    }
}