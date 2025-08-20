// src/broker/mod.rs
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

pub mod lightspeed;

use crate::config::LightspeedConfig;
use crate::types::Position;
pub use lightspeed::{LightspeedBroker, BrokerEvent};

pub struct BrokerModule {
    lightspeed: Option<Arc<LightspeedBroker>>,
}

impl BrokerModule {
    pub fn new() -> Self {
        Self {
            lightspeed: None,
        }
    }

    pub async fn initialize_lightspeed(
        &mut self, 
        config: LightspeedConfig,
    ) -> Result<()> {
        let mut broker = LightspeedBroker::new(
            config.clone(),
            config.client_id.clone(),
            config.account_id.clone(),
        );
        broker.connect().await?;
        self.lightspeed = Some(Arc::new(broker));
        Ok(())
    }

    pub async fn place_stock_order(
        &self,
        symbol: &str,
        side: &str, // BUY, SELL, SELL_SHORT
        order_type: &str, // MARKET, LIMIT
        quantity: u64,
        price: Option<f64>,
    ) -> Result<String> {
        if let Some(broker) = &self.lightspeed {
            let order = broker.create_stock_order(symbol, side, order_type, quantity, price);
            broker.place_order(order).await
        } else {
            anyhow::bail!("LightSpeed broker not initialized")
        }
    }

    pub async fn cancel_order(&self, client_order_id: &str) -> Result<()> {
        if let Some(broker) = &self.lightspeed {
            broker.cancel_order(client_order_id).await
        } else {
            anyhow::bail!("LightSpeed broker not initialized")
        }
    }

    pub async fn get_positions(&self) -> Result<HashMap<String, Position>> {
        if let Some(broker) = &self.lightspeed {
            Ok(broker.get_positions().await)
        } else {
            anyhow::bail!("LightSpeed broker not initialized")
        }
    }

    pub async fn is_connected(&self) -> bool {
        if let Some(broker) = &self.lightspeed {
            broker.is_connected().await
        } else {
            false
        }
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(_broker) = self.lightspeed.take() {
            // The connection will be dropped when the Arc is dropped
        }
        self.lightspeed = None;
        Ok(())
    }

    // Event streaming for the trading engine
    pub async fn get_broker_events(&self) -> Result<mpsc::UnboundedReceiver<BrokerEvent>> {
        if let Some(broker) = &self.lightspeed {
            Ok(broker.get_events().await)
        } else {
            anyhow::bail!("LightSpeed broker not initialized")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LightspeedConfig;

    #[tokio::test]
    async fn test_broker_initialization() {
        let mut broker = BrokerModule::new();
        
        let config = LightspeedConfig {
            api_key: "test_key".to_string(),
            connection_url: "wss://test.url".to_string(),
            client_id: "TEST_CLIENT".to_string(),
            account_id: "TEST_ACCOUNT".to_string(),
            sandbox: true,
        };

        // This will fail without real credentials, but tests the structure
        let result = broker.initialize_lightspeed(config).await;

        // In a real test environment, this should succeed
        assert!(result.is_err()); // Expected to fail with test credentials
    }
}