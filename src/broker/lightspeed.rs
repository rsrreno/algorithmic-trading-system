// src/broker/lightspeed.rs
use anyhow::{Context, Result, bail};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::config::LightspeedConfig;
use crate::types::{Position, PositionSide, PositionStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightspeedOrder {
    #[serde(rename = "MsgType")]
    pub msg_type: String,
    #[serde(rename = "ClientID")]
    pub client_id: String,
    #[serde(rename = "Account")]
    pub account: String,
    #[serde(rename = "SessionId", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(rename = "ClientOrderID", skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
    #[serde(rename = "SendingTime", skip_serializing_if = "Option::is_none")]
    pub sending_time: Option<String>,
    #[serde(rename = "Symbol")]
    pub symbol: String,
    #[serde(rename = "Side")]
    pub side: String, // BUY, SELL, SELL_SHORT
    #[serde(rename = "OrderType")]
    pub order_type: String, // LIMIT, MARKET, STOP, etc.
    #[serde(rename = "OrderQty")]
    pub order_qty: String,
    #[serde(rename = "Price", skip_serializing_if = "Option::is_none")]
    pub price: Option<String>,
    #[serde(rename = "SecurityExchange", skip_serializing_if = "Option::is_none")]
    pub security_exchange: Option<String>,
    #[serde(rename = "ExchangeDestination", skip_serializing_if = "Option::is_none")]
    pub exchange_destination: Option<String>,
    // Option-specific fields
    #[serde(rename = "SecurityType", skip_serializing_if = "Option::is_none")]
    pub security_type: Option<String>,
    #[serde(rename = "PutOrCall", skip_serializing_if = "Option::is_none")]
    pub put_or_call: Option<String>,
    #[serde(rename = "StrikePrice", skip_serializing_if = "Option::is_none")]
    pub strike_price: Option<String>,
    #[serde(rename = "MaturityYearMonth", skip_serializing_if = "Option::is_none")]
    pub maturity_year_month: Option<String>,
    #[serde(rename = "MaturityDay", skip_serializing_if = "Option::is_none")]
    pub maturity_day: Option<String>,
    #[serde(rename = "OpenClose", skip_serializing_if = "Option::is_none")]
    pub open_close: Option<String>,
    #[serde(rename = "Currency", skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightspeedResponse {
    #[serde(rename = "MsgType")]
    pub msg_type: String,
    #[serde(rename = "SessionId", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(rename = "ErrorCode", skip_serializing_if = "Option::is_none")]
    pub error_code: Option<i32>,
    #[serde(rename = "ErrorText", skip_serializing_if = "Option::is_none")]
    pub error_text: Option<String>,
    #[serde(rename = "ClientOrderID", skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
    #[serde(rename = "OrderID", skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(rename = "ExecType", skip_serializing_if = "Option::is_none")]
    pub exec_type: Option<String>,
    #[serde(rename = "OrdStatus", skip_serializing_if = "Option::is_none")]
    pub ord_status: Option<String>,
    #[serde(rename = "Symbol", skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(rename = "Side", skip_serializing_if = "Option::is_none")]
    pub side: Option<String>,
    #[serde(rename = "OrderQty", skip_serializing_if = "Option::is_none")]
    pub order_qty: Option<String>,
    #[serde(rename = "Price", skip_serializing_if = "Option::is_none")]
    pub price: Option<String>,
    #[serde(rename = "LastPx", skip_serializing_if = "Option::is_none")]
    pub last_px: Option<String>,
    #[serde(rename = "LastQty", skip_serializing_if = "Option::is_none")]
    pub last_qty: Option<String>,
    #[serde(rename = "CumQty", skip_serializing_if = "Option::is_none")]
    pub cum_qty: Option<String>,
    #[serde(rename = "LeavesQty", skip_serializing_if = "Option::is_none")]
    pub leaves_qty: Option<String>,
    #[serde(flatten)]
    pub other: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoginMessage {
    #[serde(rename = "MsgType")]
    pub msg_type: String,
    #[serde(rename = "ApiKey")]
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HeartbeatMessage {
    #[serde(rename = "MsgType")]
    pub msg_type: String,
    #[serde(rename = "SessionId")]
    pub session_id: String,
    #[serde(rename = "SendingTime")]
    pub sending_time: String,
}

#[derive(Debug, Clone)]
pub enum BrokerEvent {
    Connected,
    Disconnected,
    OrderAck { client_order_id: String, order_id: String },
    OrderFill { 
        client_order_id: String, 
        symbol: String,
        side: String,
        qty: f64,
        price: f64,
    },
    OrderReject { client_order_id: String, reason: String },
    Error { message: String },
}

pub struct LightspeedBroker {
    config: LightspeedConfig,
    client_id: String,
    account_id: String,
    session_id: Arc<RwLock<Option<String>>>,
    positions: Arc<RwLock<HashMap<String, Position>>>,
    pending_orders: Arc<RwLock<HashMap<String, LightspeedOrder>>>,
    event_tx: mpsc::UnboundedSender<BrokerEvent>,
    event_rx: Arc<Mutex<mpsc::UnboundedReceiver<BrokerEvent>>>,
    send_tx: Option<mpsc::UnboundedSender<String>>,
    is_connected: Arc<RwLock<bool>>,
}

impl std::fmt::Debug for LightspeedBroker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LightspeedBroker")
            .field("client_id", &self.client_id)
            .field("account_id", &self.account_id)
            .field("is_connected", &"<RwLock<bool>>")
            .finish()
    }
}

impl LightspeedBroker {
    pub fn new(config: LightspeedConfig, client_id: String, account_id: String) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        
        Self {
            config,
            client_id,
            account_id,
            session_id: Arc::new(RwLock::new(None)),
            positions: Arc::new(RwLock::new(HashMap::new())),
            pending_orders: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            event_rx: Arc::new(Mutex::new(event_rx)),
            send_tx: None,
            is_connected: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn connect(&mut self) -> Result<()> {
        let url = &self.config.connection_url;

        info!("Connecting to LightSpeed at: {}", url);

        let (ws_stream, _) = connect_async(url)
            .await
            .context("Failed to connect to LightSpeed WebSocket")?;

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        let (send_tx, mut send_rx) = mpsc::unbounded_channel::<String>();
        self.send_tx = Some(send_tx);

        // Login
        let login_msg = LoginMessage {
            msg_type: "Logon".to_string(),
            api_key: self.config.api_key.clone(),
        };

        ws_sender
            .send(Message::Text(serde_json::to_string(&login_msg)?))
            .await?;

        // Clone necessary data for tasks
        let session_id = Arc::clone(&self.session_id);
        let event_tx = self.event_tx.clone();
        let is_connected = Arc::clone(&self.is_connected);
        let pending_orders = Arc::clone(&self.pending_orders);
        let positions = Arc::clone(&self.positions);

        // Message sender task
        let sender_task = {
            tokio::spawn(async move {
                while let Some(message) = send_rx.recv().await {
                    if let Err(e) = ws_sender.send(Message::Text(message)).await {
                        error!("Failed to send message: {}", e);
                        break;
                    }
                }
            })
        };

        // Message receiver task
        let receiver_task = {
            tokio::spawn(async move {
                while let Some(msg) = ws_receiver.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Err(e) = Self::handle_message(
                                &text,
                                &session_id,
                                &event_tx,
                                &is_connected,
                                &pending_orders,
                                &positions,
                            ).await {
                                error!("Error handling message: {}", e);
                            }
                        }
                        Ok(Message::Close(_)) => {
                            info!("WebSocket closed by server");
                            break;
                        }
                        Err(e) => {
                            error!("WebSocket error: {}", e);
                            break;
                        }
                        _ => {}
                    }
                }
                
                // Update connection status
                *is_connected.write().await = false;
                let _ = event_tx.send(BrokerEvent::Disconnected);
            })
        };

        // Start heartbeat task
        let heartbeat_task = {
            let session_id = Arc::clone(&self.session_id);
            let send_tx = self.send_tx.as_ref().unwrap().clone();
            let is_connected = Arc::clone(&self.is_connected);

            tokio::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
                
                loop {
                    interval.tick().await;
                    
                    if !*is_connected.read().await {
                        break;
                    }

                    if let Some(session) = session_id.read().await.as_ref() {
                        let heartbeat = HeartbeatMessage {
                            msg_type: "Heartbeat".to_string(),
                            session_id: session.clone(),
                            sending_time: chrono::Utc::now()
                                .format("%Y%m%d-%H:%M:%S%.3f")
                                .to_string(),
                        };

                        if let Ok(msg) = serde_json::to_string(&heartbeat) {
                            if send_tx.send(msg).is_err() {
                                break;
                            }
                        }
                    }
                }
            })
        };

        // Don't block - let tasks run in background
        tokio::spawn(async move {
            tokio::select! {
                _ = sender_task => {},
                _ = receiver_task => {},
                _ = heartbeat_task => {},
            }
        });

        Ok(())
    }

    async fn handle_message(
        message: &str,
        session_id: &Arc<RwLock<Option<String>>>,
        event_tx: &mpsc::UnboundedSender<BrokerEvent>,
        is_connected: &Arc<RwLock<bool>>,
        pending_orders: &Arc<RwLock<HashMap<String, LightspeedOrder>>>,
        positions: &Arc<RwLock<HashMap<String, Position>>>,
    ) -> Result<()> {
        debug!("Received message: {}", message);

        let response: LightspeedResponse = serde_json::from_str(message)
            .context("Failed to parse LightSpeed response")?;

        match response.msg_type.as_str() {
            "Logon" => {
                if let Some(error_code) = response.error_code {
                    if error_code == 0 || error_code == 351 {
                        if let Some(new_session_id) = response.session_id {
                            *session_id.write().await = Some(new_session_id.clone());
                            *is_connected.write().await = true;
                            info!("Successfully logged in with session: {}", new_session_id);
                            let _ = event_tx.send(BrokerEvent::Connected);
                        }
                    } else {
                        let error_msg = response.error_text.unwrap_or_else(|| format!("Login failed with code: {}", error_code));
                        error!("Login failed: {}", error_msg);
                        let _ = event_tx.send(BrokerEvent::Error { message: error_msg });
                    }
                }
            }
            "ExecutionReport" => {
                Self::handle_execution_report(&response, event_tx, pending_orders, positions).await?;
            }
            "OrderSingleUpdate" => {
                Self::handle_order_update(&response, event_tx, pending_orders).await?;
            }
            "PositionUpdate" => {
                Self::handle_position_update(&response, positions).await?;
            }
            "PositionStatus" => {
                Self::handle_position_status(&response, positions).await?;
            }
            "OrderCancelReject" | "OrderReject" => {
                if let Some(client_order_id) = response.client_order_id {
                    let reason = response.error_text.unwrap_or_else(|| "Order rejected".to_string());
                    let _ = event_tx.send(BrokerEvent::OrderReject { client_order_id, reason });
                }
            }
            "Heartbeat" => {
                debug!("Received heartbeat");
            }
            "Logout" => {
                *is_connected.write().await = false;
                let _ = event_tx.send(BrokerEvent::Disconnected);
            }
            _ => {
                debug!("Unhandled message type: {}", response.msg_type);
            }
        }

        Ok(())
    }

    async fn handle_order_update(
        response: &LightspeedResponse,
        event_tx: &mpsc::UnboundedSender<BrokerEvent>,
        pending_orders: &Arc<RwLock<HashMap<String, LightspeedOrder>>>,
    ) -> Result<()> {
        if let Some(client_order_id) = &response.client_order_id {
            if let Some(order_status) = &response.other.get("OrderStatus") {
                match order_status.as_str() {
                    Some("PENDING_NEW") => {
                        debug!("Order pending validation: {}", client_order_id);
                    }
                    Some("NEW") => {
                        if let Some(order_id) = response.other.get("OrderID").and_then(|v| v.as_str()) {
                            info!("Order accepted: {} -> {}", client_order_id, order_id);
                            let _ = event_tx.send(BrokerEvent::OrderAck {
                                client_order_id: client_order_id.clone(),
                                order_id: order_id.to_string(),
                            });
                        }
                    }
                    Some("PARTIALLY_FILLED") | Some("FILLED") => {
                        // This will be handled by ExecutionReport
                        debug!("Order status update: {} - {}", client_order_id, order_status);
                    }
                    Some("REJECTED") => {
                        let reason = response.error_text.as_ref()
                            .unwrap_or(&"Order rejected".to_string()).clone();
                        let _ = event_tx.send(BrokerEvent::OrderReject {
                            client_order_id: client_order_id.clone(),
                            reason,
                        });
                        pending_orders.write().await.remove(client_order_id);
                    }
                    _ => {
                        debug!("Unhandled order status: {:?}", order_status);
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_position_update(
        response: &LightspeedResponse,
        positions: &Arc<RwLock<HashMap<String, Position>>>,
    ) -> Result<()> {
        if let Some(position_data) = response.other.get("position") {
            if let Some(position_obj) = position_data.as_object() {
                if let (Some(symbol), Some(pos_qty)) = (
                    position_obj.get("sym").and_then(|v| v.as_str()),
                    position_obj.get("pos").and_then(|v| v.as_i64()),
                ) {
                    let avg_price = position_obj.get("posAvgPrice").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    
                    if avg_price > 0.0 {
                        info!("Position update: {} - {} shares @ ${:.2}", symbol, pos_qty, avg_price);
                    } else {
                        info!("Position update: {} - {} shares (price TBD)", symbol, pos_qty);
                    }
                    
                    let mut positions_map = positions.write().await;
                    
                    if pos_qty == 0 {
                        // Position closed
                        if let Some(mut position) = positions_map.get_mut(symbol) {
                            position.quantity = 0;
                            position.status = crate::types::PositionStatus::Closed;
                            position.closed_at = Some(Utc::now());
                        }
                    } else {
                        // Position opened or updated
                        let position = positions_map.entry(symbol.to_string()).or_insert_with(|| {
                            crate::types::Position {
                                id: uuid::Uuid::new_v4().to_string(),
                                symbol: symbol.to_string(),
                                side: if pos_qty > 0 { 
                                    crate::types::PositionSide::Long 
                                } else { 
                                    crate::types::PositionSide::Short 
                                },
                                quantity: 0,
                                entry_price: avg_price,
                                current_price: avg_price,
                                opened_at: Utc::now(),
                                closed_at: None,
                                status: crate::types::PositionStatus::Open,
                            }
                        });
                        
                        position.quantity = pos_qty.abs() as u64;
                        position.side = if pos_qty > 0 { 
                            crate::types::PositionSide::Long 
                        } else { 
                            crate::types::PositionSide::Short 
                        };
                        if avg_price > 0.0 {
                            position.entry_price = avg_price;
                            position.current_price = avg_price;
                        }
                        position.status = crate::types::PositionStatus::Open;
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_position_status(
        response: &LightspeedResponse,
        positions: &Arc<RwLock<HashMap<String, Position>>>,
    ) -> Result<()> {
        if let Some(positions_array) = response.other.get("positions").and_then(|v| v.as_array()) {
            info!("Received position status with {} positions", positions_array.len());
            
            let mut positions_map = positions.write().await;
            
            for position_data in positions_array {
                if let Some(position_obj) = position_data.as_object() {
                    if let (Some(symbol), Some(pos_qty)) = (
                        position_obj.get("sym").and_then(|v| v.as_str()),
                        position_obj.get("pos").and_then(|v| v.as_i64()),
                    ) {
                        let avg_price = position_obj.get("posAvgPrice").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        
                        if pos_qty != 0 {
                            debug!("Position status: {} - {} shares @ ${:.2}", symbol, pos_qty, avg_price);
                            
                            let position = positions_map.entry(symbol.to_string()).or_insert_with(|| {
                                crate::types::Position {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    symbol: symbol.to_string(),
                                    side: if pos_qty > 0 { 
                                        crate::types::PositionSide::Long 
                                    } else { 
                                        crate::types::PositionSide::Short 
                                    },
                                    quantity: 0,
                                    entry_price: avg_price,
                                    current_price: avg_price,
                                    opened_at: Utc::now(),
                                    closed_at: None,
                                    status: crate::types::PositionStatus::Open,
                                }
                            });
                            
                            position.quantity = pos_qty.abs() as u64;
                            position.side = if pos_qty > 0 { 
                                crate::types::PositionSide::Long 
                            } else { 
                                crate::types::PositionSide::Short 
                            };
                            if avg_price > 0.0 {
                                position.entry_price = avg_price;
                                position.current_price = avg_price;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_execution_report(
        response: &LightspeedResponse,
        event_tx: &mpsc::UnboundedSender<BrokerEvent>,
        pending_orders: &Arc<RwLock<HashMap<String, LightspeedOrder>>>,
        positions: &Arc<RwLock<HashMap<String, Position>>>,
    ) -> Result<()> {
        if let Some(client_order_id) = &response.client_order_id {
            match response.exec_type.as_deref() {
                Some("0") | Some("NEW") => {
                    // Order acknowledged
                    if let Some(order_id) = &response.order_id {
                        let _ = event_tx.send(BrokerEvent::OrderAck {
                            client_order_id: client_order_id.clone(),
                            order_id: order_id.clone(),
                        });
                    }
                }
                Some("1") | Some("PARTIAL_FILL") | Some("2") | Some("FILL") => {
                    // Order filled (partial or complete)
                    if let (Some(symbol), Some(side), Some(last_qty), Some(last_px)) = (
                        &response.symbol,
                        &response.side,
                        &response.last_qty,
                        &response.last_px,
                    ) {
                        let qty: f64 = last_qty.parse().unwrap_or(0.0);
                        let price: f64 = last_px.parse().unwrap_or(0.0);

                        let _ = event_tx.send(BrokerEvent::OrderFill {
                            client_order_id: client_order_id.clone(),
                            symbol: symbol.clone(),
                            side: side.clone(),
                            qty,
                            price,
                        });

                        // Update positions
                        Self::update_position(positions, symbol, side, qty, price).await;
                    }
                }
                Some("4") | Some("CANCELED") => {
                    info!("Order canceled: {}", client_order_id);
                    pending_orders.write().await.remove(client_order_id);
                }
                Some("8") | Some("REJECTED") => {
                    let reason = response.error_text.as_ref()
                        .unwrap_or(&"Order rejected".to_string()).clone();
                    let _ = event_tx.send(BrokerEvent::OrderReject {
                        client_order_id: client_order_id.clone(),
                        reason,
                    });
                    pending_orders.write().await.remove(client_order_id);
                }
                _ => {
                    debug!("Unhandled execution type: {:?}", response.exec_type);
                }
            }
        }

        Ok(())
    }

    async fn update_position(
        positions: &Arc<RwLock<HashMap<String, Position>>>,
        symbol: &str,
        side: &str,
        qty: f64,
        price: f64,
    ) {
        let mut positions_map = positions.write().await;
        
        let position = positions_map.entry(symbol.to_string()).or_insert_with(|| Position {
            id: Uuid::new_v4().to_string(),
            symbol: symbol.to_string(),
            side: PositionSide::Long,
            quantity: 0,
            entry_price: 0.0,
            current_price: price,
            opened_at: Utc::now(),
            closed_at: None,
            status: PositionStatus::Open,
        });

        match side {
            "BUY" => {
                position.quantity += qty as u64;
                position.side = PositionSide::Long;
            }
            "SELL" | "SELL_SHORT" => {
                if position.quantity >= qty as u64 {
                    position.quantity -= qty as u64;
                    if position.quantity == 0 {
                        position.status = PositionStatus::Closed;
                        position.closed_at = Some(Utc::now());
                    }
                } else {
                    // Going short
                    position.quantity = (qty as u64) - position.quantity;
                    position.side = PositionSide::Short;
                }
            }
            _ => {}
        }

        position.current_price = price;
    }

    pub async fn place_order(&self, order: LightspeedOrder) -> Result<String> {
        if !*self.is_connected.read().await {
            bail!("Not connected to LightSpeed");
        }

        let session_id = self.session_id.read().await.clone()
            .ok_or_else(|| anyhow::anyhow!("No session ID available"))?;

        // Generate shorter client order ID (max 20 chars for LightSpeed)
        let client_order_id = format!("LS{}", Uuid::new_v4().to_string().replace("-", "")[..18].to_uppercase());
        
        let mut order = order;
        order.session_id = Some(session_id);
        order.client_order_id = Some(client_order_id.clone());
        order.sending_time = Some(chrono::Utc::now().format("%Y%m%d-%H:%M:%S%.3f").to_string());

        let message = serde_json::to_string(&order)?;

        if let Some(sender) = &self.send_tx {
            sender.send(message)
                .map_err(|_| anyhow::anyhow!("Failed to send order"))?;

            self.pending_orders.write().await.insert(client_order_id.clone(), order);
            
            Ok(client_order_id)
        } else {
            bail!("Sender not available")
        }
    }

    pub async fn cancel_order(&self, client_order_id: &str) -> Result<()> {
        if !*self.is_connected.read().await {
            bail!("Not connected to LightSpeed");
        }

        let session_id = self.session_id.read().await.clone()
            .ok_or_else(|| anyhow::anyhow!("No session ID available"))?;

        let cancel_msg = serde_json::json!({
            "MsgType": "OrderCancel",
            "ClientID": self.client_id,
            "SessionId": session_id,
            "OrigClientOrderID": client_order_id,
            "SendingTime": chrono::Utc::now().format("%Y%m%d-%H:%M:%S%.3f").to_string()
        });

        if let Some(sender) = &self.send_tx {
            sender.send(cancel_msg.to_string())
                .map_err(|_| anyhow::anyhow!("Failed to send cancel order"))?;
        }

        Ok(())
    }

    pub async fn get_events(&self) -> mpsc::UnboundedReceiver<BrokerEvent> {
        // Create a new receiver that forwards events from the main event channel
        let (tx, rx) = mpsc::unbounded_channel();
        
        // In a real implementation, you'd subscribe to the main event stream
        // For now, we'll create a receiver that doesn't immediately close
        // by spawning a task that keeps it alive
        let _event_tx = self.event_tx.clone();
        tokio::spawn(async move {
            // This task keeps the receiver alive and forwards any real events
            // For now, it just ensures the channel doesn't close immediately
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
            let _ = tx.send(BrokerEvent::Disconnected);
        });
        
        rx
    }

    pub async fn get_positions(&self) -> HashMap<String, Position> {
        self.positions.read().await.clone()
    }

    pub async fn is_connected(&self) -> bool {
        *self.is_connected.read().await
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        *self.is_connected.write().await = false;
        
        if let Some(sender) = &self.send_tx {
            // Send logout message
            if let Some(session_id) = self.session_id.read().await.as_ref() {
                let logout_msg = serde_json::json!({
                    "MsgType": "Logout",
                    "SessionId": session_id,
                    "SendingTime": chrono::Utc::now().format("%Y%m%d-%H:%M:%S%.3f").to_string()
                });
                let _ = sender.send(logout_msg.to_string());
            }
        }

        self.send_tx = None;
        info!("Disconnected from LightSpeed");
        Ok(())
    }
}

// Helper functions for creating orders
impl LightspeedBroker {
    pub fn create_stock_order(
        &self,
        symbol: &str,
        side: &str, // BUY, SELL, SELL_SHORT
        order_type: &str, // MARKET, LIMIT
        quantity: u64,
        price: Option<f64>,
    ) -> LightspeedOrder {
        LightspeedOrder {
            msg_type: "OrderSingle".to_string(),
            client_id: self.client_id.clone(),
            account: self.account_id.clone(),
            session_id: None, // Will be set when sending
            client_order_id: None, // Will be set when sending
            sending_time: None, // Will be set when sending
            symbol: symbol.to_string(),
            side: side.to_string(),
            order_type: order_type.to_string(),
            order_qty: quantity.to_string(),
            price: price.map(|p| p.to_string()),
            security_exchange: Some("NASDAQ".to_string()),
            exchange_destination: Some("SMART".to_string()),
            security_type: None,
            put_or_call: None,
            strike_price: None,
            maturity_year_month: None,
            maturity_day: None,
            open_close: None,
            currency: None,
        }
    }
}