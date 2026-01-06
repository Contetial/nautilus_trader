// -------------------------------------------------------------------------------------------------
//  Copyright (C) 2015-2025 Nautech Systems Pty Ltd. All rights reserved.
//  https://nautechsystems.io
//
//  Licensed under the GNU Lesser General Public License Version 3.0 (the "License");
//  You may not use this file except in compliance with the License.
//  You may obtain a copy of the License at https://www.gnu.org/licenses/lgpl-3.0.en.html
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.
// -------------------------------------------------------------------------------------------------

//! Broker adapter trait and registry for multi-broker support.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Common order types across brokers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit,
    StopLoss,
    StopLossMarket,
}

/// Common product types across brokers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProductType {
    Intraday,      // MIS equivalent
    Delivery,      // CNC equivalent
    Normal,        // NRML equivalent
}

/// Common transaction types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionType {
    Buy,
    Sell,
}

/// Standardized order request across brokers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerOrderRequest {
    pub symbol: String,
    pub exchange: String,
    pub transaction_type: TransactionType,
    pub order_type: OrderType,
    pub product_type: ProductType,
    pub quantity: u32,
    pub price: Option<f64>,
    pub trigger_price: Option<f64>,
    pub disclosed_quantity: Option<u32>,
    pub tag: Option<String>,
}

/// Standardized order response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerOrderResponse {
    pub order_id: String,
    pub broker_order_id: Option<String>,
    pub status: String,
    pub message: Option<String>,
}

/// Standardized order status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerOrderStatus {
    pub order_id: String,
    pub symbol: String,
    pub exchange: String,
    pub transaction_type: TransactionType,
    pub order_type: OrderType,
    pub quantity: u32,
    pub filled_quantity: u32,
    pub pending_quantity: u32,
    pub price: f64,
    pub average_price: Option<f64>,
    pub status: String,
    pub status_message: Option<String>,
    pub placed_at: String,
}

/// Standardized position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerPosition {
    pub symbol: String,
    pub exchange: String,
    pub product_type: ProductType,
    pub quantity: i32,
    pub average_price: f64,
    pub last_price: f64,
    pub pnl: f64,
    pub pnl_percent: f64,
    pub buy_value: f64,
    pub sell_value: f64,
}

/// Standardized holding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerHolding {
    pub symbol: String,
    pub exchange: String,
    pub quantity: i32,
    pub average_price: f64,
    pub last_price: f64,
    pub pnl: f64,
    pub pnl_percent: f64,
    pub day_change: f64,
    pub day_change_percent: f64,
}

/// Standardized account info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerAccountInfo {
    pub broker_name: String,
    pub user_id: String,
    pub user_name: String,
    pub email: Option<String>,
    pub balance: f64,
    pub available_margin: f64,
    pub used_margin: f64,
    pub is_authenticated: bool,
}

/// Standardized quote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerQuote {
    pub symbol: String,
    pub exchange: String,
    pub last_price: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
    pub change: f64,
    pub change_percent: f64,
    pub timestamp: String,
}

/// Standardized historical candle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerCandle {
    pub timestamp: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}

/// Broker authentication credentials
#[derive(Debug, Clone)]
pub struct BrokerCredentials {
    pub api_key: String,
    pub api_secret: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub user_id: Option<String>,
    pub password: Option<String>,
    pub totp_secret: Option<String>,
}

/// Broker adapter trait - implement this for each broker
#[async_trait]
pub trait BrokerAdapter: Send + Sync {
    /// Get the broker name
    fn name(&self) -> &str;

    /// Get the broker ID (lowercase, for routing)
    fn id(&self) -> &str;

    /// Check if the adapter is authenticated
    async fn is_authenticated(&self) -> bool;

    /// Authenticate with the broker
    async fn authenticate(&self, credentials: &BrokerCredentials) -> Result<()>;

    /// Logout from the broker
    async fn logout(&self) -> Result<()>;

    /// Get account information
    async fn get_account_info(&self) -> Result<BrokerAccountInfo>;

    // Order Management

    /// Place a new order
    async fn place_order(&self, request: BrokerOrderRequest) -> Result<BrokerOrderResponse>;

    /// Modify an existing order
    async fn modify_order(&self, order_id: &str, request: BrokerOrderRequest) -> Result<BrokerOrderResponse>;

    /// Cancel an order
    async fn cancel_order(&self, order_id: &str) -> Result<BrokerOrderResponse>;

    /// Get order status
    async fn get_order_status(&self, order_id: &str) -> Result<BrokerOrderStatus>;

    /// Get all orders for today
    async fn get_orders(&self) -> Result<Vec<BrokerOrderStatus>>;

    // Portfolio

    /// Get all positions
    async fn get_positions(&self) -> Result<Vec<BrokerPosition>>;

    /// Get all holdings
    async fn get_holdings(&self) -> Result<Vec<BrokerHolding>>;

    // Market Data

    /// Get quote for a symbol
    async fn get_quote(&self, symbol: &str, exchange: &str) -> Result<BrokerQuote>;

    /// Get historical candles
    async fn get_historical_data(
        &self,
        symbol: &str,
        exchange: &str,
        from_date: &str,
        to_date: &str,
        interval: &str,
    ) -> Result<Vec<BrokerCandle>>;

    /// Search for instruments
    async fn search_instruments(&self, query: &str) -> Result<Vec<InstrumentInfo>>;
}

/// Instrument information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentInfo {
    pub symbol: String,
    pub exchange: String,
    pub name: String,
    pub instrument_type: String,
    pub lot_size: u32,
    pub tick_size: f64,
}

/// Broker registry for managing multiple brokers
pub struct BrokerRegistry {
    adapters: RwLock<HashMap<String, Arc<dyn BrokerAdapter>>>,
    default_broker: RwLock<Option<String>>,
}

impl BrokerRegistry {
    /// Create a new broker registry
    pub fn new() -> Self {
        Self {
            adapters: RwLock::new(HashMap::new()),
            default_broker: RwLock::new(None),
        }
    }

    /// Register a broker adapter
    pub async fn register(&self, adapter: Arc<dyn BrokerAdapter>) {
        let id = adapter.id().to_string();
        let mut adapters = self.adapters.write().await;

        // Set as default if it's the first one
        if adapters.is_empty() {
            *self.default_broker.write().await = Some(id.clone());
        }

        adapters.insert(id, adapter);
    }

    /// Unregister a broker adapter
    pub async fn unregister(&self, broker_id: &str) {
        let mut adapters = self.adapters.write().await;
        adapters.remove(broker_id);

        // Clear default if it was this broker
        let mut default = self.default_broker.write().await;
        if default.as_deref() == Some(broker_id) {
            *default = adapters.keys().next().cloned();
        }
    }

    /// Get a broker adapter by ID
    pub async fn get(&self, broker_id: &str) -> Option<Arc<dyn BrokerAdapter>> {
        let adapters = self.adapters.read().await;
        adapters.get(broker_id).cloned()
    }

    /// Get the default broker adapter
    pub async fn get_default(&self) -> Option<Arc<dyn BrokerAdapter>> {
        let default_id = self.default_broker.read().await;
        if let Some(id) = default_id.as_ref() {
            self.get(id).await
        } else {
            None
        }
    }

    /// Set the default broker
    pub async fn set_default(&self, broker_id: &str) -> Result<()> {
        let adapters = self.adapters.read().await;
        if adapters.contains_key(broker_id) {
            *self.default_broker.write().await = Some(broker_id.to_string());
            Ok(())
        } else {
            anyhow::bail!("Broker '{}' not registered", broker_id)
        }
    }

    /// Get all registered broker IDs
    pub async fn list_brokers(&self) -> Vec<String> {
        let adapters = self.adapters.read().await;
        adapters.keys().cloned().collect()
    }

    /// Get the current default broker ID
    pub async fn get_default_broker_id(&self) -> Option<String> {
        self.default_broker.read().await.clone()
    }

    /// Check if any broker is authenticated
    pub async fn has_authenticated_broker(&self) -> bool {
        let adapters = self.adapters.read().await;
        for adapter in adapters.values() {
            if adapter.is_authenticated().await {
                return true;
            }
        }
        false
    }

    /// Get all authenticated brokers
    pub async fn get_authenticated_brokers(&self) -> Vec<String> {
        let adapters = self.adapters.read().await;
        let mut result = Vec::new();
        for (id, adapter) in adapters.iter() {
            if adapter.is_authenticated().await {
                result.push(id.clone());
            }
        }
        result
    }
}

impl Default for BrokerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock broker for testing
    struct MockBroker {
        name: String,
        id: String,
        authenticated: std::sync::atomic::AtomicBool,
    }

    impl MockBroker {
        fn new(id: &str, name: &str) -> Self {
            Self {
                name: name.to_string(),
                id: id.to_string(),
                authenticated: std::sync::atomic::AtomicBool::new(false),
            }
        }
    }

    #[async_trait]
    impl BrokerAdapter for MockBroker {
        fn name(&self) -> &str { &self.name }
        fn id(&self) -> &str { &self.id }

        async fn is_authenticated(&self) -> bool {
            self.authenticated.load(std::sync::atomic::Ordering::SeqCst)
        }

        async fn authenticate(&self, _credentials: &BrokerCredentials) -> Result<()> {
            self.authenticated.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }

        async fn logout(&self) -> Result<()> {
            self.authenticated.store(false, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }

        async fn get_account_info(&self) -> Result<BrokerAccountInfo> {
            Ok(BrokerAccountInfo {
                broker_name: self.name.clone(),
                user_id: "TEST123".to_string(),
                user_name: "Test User".to_string(),
                email: None,
                balance: 100000.0,
                available_margin: 90000.0,
                used_margin: 10000.0,
                is_authenticated: self.is_authenticated().await,
            })
        }

        async fn place_order(&self, _request: BrokerOrderRequest) -> Result<BrokerOrderResponse> {
            Ok(BrokerOrderResponse {
                order_id: "ORD123".to_string(),
                broker_order_id: Some("BROKER_ORD_123".to_string()),
                status: "OPEN".to_string(),
                message: None,
            })
        }

        async fn modify_order(&self, order_id: &str, _request: BrokerOrderRequest) -> Result<BrokerOrderResponse> {
            Ok(BrokerOrderResponse {
                order_id: order_id.to_string(),
                broker_order_id: None,
                status: "MODIFIED".to_string(),
                message: None,
            })
        }

        async fn cancel_order(&self, order_id: &str) -> Result<BrokerOrderResponse> {
            Ok(BrokerOrderResponse {
                order_id: order_id.to_string(),
                broker_order_id: None,
                status: "CANCELLED".to_string(),
                message: None,
            })
        }

        async fn get_order_status(&self, order_id: &str) -> Result<BrokerOrderStatus> {
            Ok(BrokerOrderStatus {
                order_id: order_id.to_string(),
                symbol: "RELIANCE".to_string(),
                exchange: "NSE".to_string(),
                transaction_type: TransactionType::Buy,
                order_type: OrderType::Market,
                quantity: 100,
                filled_quantity: 100,
                pending_quantity: 0,
                price: 2500.0,
                average_price: Some(2500.0),
                status: "COMPLETE".to_string(),
                status_message: None,
                placed_at: "2025-01-06T10:00:00Z".to_string(),
            })
        }

        async fn get_orders(&self) -> Result<Vec<BrokerOrderStatus>> {
            Ok(vec![])
        }

        async fn get_positions(&self) -> Result<Vec<BrokerPosition>> {
            Ok(vec![])
        }

        async fn get_holdings(&self) -> Result<Vec<BrokerHolding>> {
            Ok(vec![])
        }

        async fn get_quote(&self, symbol: &str, exchange: &str) -> Result<BrokerQuote> {
            Ok(BrokerQuote {
                symbol: symbol.to_string(),
                exchange: exchange.to_string(),
                last_price: 2500.0,
                open: 2480.0,
                high: 2520.0,
                low: 2470.0,
                close: 2490.0,
                volume: 1000000,
                change: 10.0,
                change_percent: 0.4,
                timestamp: "2025-01-06T10:00:00Z".to_string(),
            })
        }

        async fn get_historical_data(
            &self,
            _symbol: &str,
            _exchange: &str,
            _from_date: &str,
            _to_date: &str,
            _interval: &str,
        ) -> Result<Vec<BrokerCandle>> {
            Ok(vec![])
        }

        async fn search_instruments(&self, _query: &str) -> Result<Vec<InstrumentInfo>> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn test_broker_registry() {
        let registry = BrokerRegistry::new();

        // Register brokers
        let zerodha = Arc::new(MockBroker::new("zerodha", "Zerodha"));
        let kotak = Arc::new(MockBroker::new("kotak", "Kotak Neo"));

        registry.register(zerodha.clone()).await;
        registry.register(kotak.clone()).await;

        // Test list
        let brokers = registry.list_brokers().await;
        assert_eq!(brokers.len(), 2);

        // Test get
        let broker = registry.get("zerodha").await;
        assert!(broker.is_some());
        assert_eq!(broker.unwrap().name(), "Zerodha");

        // Test default
        let default = registry.get_default_broker_id().await;
        assert!(default.is_some());

        // Test set default
        registry.set_default("kotak").await.unwrap();
        let default = registry.get_default_broker_id().await;
        assert_eq!(default, Some("kotak".to_string()));
    }
}
