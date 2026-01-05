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

//! Execution client for Zerodha order management.

use crate::{
    config::ZerodhaExecutionConfig,
    enums::{Exchange, OrderStatus, OrderType, ProductType, TransactionType, Validity},
    error::{ZerodhaError, ZerodhaResult},
    execution::paper_trading::PaperTradingEngine,
    http::ZerodhaHttpClient,
    types::{ZerodhaOrder, ZerodhaOrderResponse, ZerodhaPosition},
};
use rust_decimal::{Decimal, prelude::FromPrimitive};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Execution client for Zerodha order management
#[derive(Debug)]
pub struct ZerodhaExecutionClient {
    config: ZerodhaExecutionConfig,
    http_client: ZerodhaHttpClient,
    
    // Order tracking
    orders: Arc<RwLock<HashMap<String, ZerodhaOrder>>>,
    order_counter: AtomicU64,
    
    // Position tracking
    positions: Arc<RwLock<HashMap<String, ZerodhaPosition>>>,
    
    // Account info
    account_balance: Arc<RwLock<f64>>,
    available_margin: Arc<RwLock<f64>>,
    
    // Paper trading engine (optional)
    paper_engine: Option<Arc<RwLock<PaperTradingEngine>>>,
}

#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub tradingsymbol: String,
    pub exchange: Exchange,
    pub transaction_type: String, // "BUY" or "SELL"
    pub order_type: OrderType,
    pub product: ProductType,
    pub validity: Validity,
    pub quantity: u32,
    pub price: Option<f64>,
    pub trigger_price: Option<f64>,
    pub disclosed_quantity: Option<u32>,
    pub tag: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OrderModifyRequest {
    pub order_id: String,
    pub quantity: Option<u32>,
    pub price: Option<f64>,
    pub trigger_price: Option<f64>,
    pub order_type: Option<OrderType>,
    pub validity: Option<Validity>,
}

impl ZerodhaExecutionClient {
    /// Create a new execution client
    pub fn new(config: ZerodhaExecutionConfig, http_client: ZerodhaHttpClient) -> Self {
        // Create paper trading engine if enabled
        let paper_engine = if config.paper_trading {
            info!("📋 Initializing paper trading mode with ₹{:.2} balance", config.paper_balance);
            Some(Arc::new(RwLock::new(PaperTradingEngine::new(config.clone()))))
        } else {
            None
        };
        
        Self {
            config,
            http_client,
            orders: Arc::new(RwLock::new(HashMap::new())),
            order_counter: AtomicU64::new(1),
            positions: Arc::new(RwLock::new(HashMap::new())),
            account_balance: Arc::new(RwLock::new(0.0)),
            available_margin: Arc::new(RwLock::new(0.0)),
            paper_engine,
        }
    }
    
    /// Place a new order
    pub async fn place_order(&self, request: OrderRequest) -> ZerodhaResult<ZerodhaOrderResponse> {
        info!("📤 Placing order: {} {} {} @ {:?}", 
              request.transaction_type, 
              request.quantity, 
              request.tradingsymbol,
              request.price);
        
        // Use paper trading if enabled
        if let Some(ref paper_engine) = self.paper_engine {
            return paper_engine.write().await.place_order(request).await;
        }
        
        // Validate order request
        self.validate_order_request(&request).await?;
        
        // Generate client order ID
        let client_order_id = self.generate_client_order_id();
        
        // Convert f64 prices to Decimal
        let price_decimal = request.price.map(|p| Decimal::from_f64(p).unwrap_or_default());
        let trigger_price_decimal = request.trigger_price.map(|p| Decimal::from_f64(p).unwrap_or_default());
        
        // Parse transaction type
        let transaction_type = match request.transaction_type.as_str() {
            "BUY" => TransactionType::BUY,
            "SELL" => TransactionType::SELL,
            _ => return Err(ZerodhaError::validation_error(format!("Invalid transaction type: {}", request.transaction_type))),
        };
        
        // Place order via HTTP API
        let response = self.http_client.place_order(
            request.exchange,
            &request.tradingsymbol,
            transaction_type,
            request.quantity,
            request.product,
            request.order_type,
            price_decimal,
            trigger_price_decimal,
            Some(request.validity),
            request.disclosed_quantity,
            Some(&client_order_id),
        ).await?;
        
        // Create order object for tracking
        let order = ZerodhaOrder {
            account_id: "USER123".to_string(), // Placeholder account ID
            placed_by: "API".to_string(),
            order_id: response.clone(),
            exchange_order_id: None,
            parent_order_id: None,
            status: OrderStatus::OPEN,
            status_message: None,
            order_timestamp: chrono::Utc::now(),
            exchange_timestamp: None,
            variety: "regular".to_string(),
            exchange: request.exchange,
            tradingsymbol: request.tradingsymbol.clone(),
            instrument_token: 0, // Will be populated later
            order_type: request.order_type,
            transaction_type: transaction_type,
            validity: request.validity,
            product: request.product,
            quantity: request.quantity,
            disclosed_quantity: request.disclosed_quantity,
            price: price_decimal.unwrap_or_default(),
            trigger_price: trigger_price_decimal,
            average_price: None,
            filled_quantity: 0,
            pending_quantity: request.quantity,
            cancelled_quantity: 0,
            market_protection: None,
            tag: Some(client_order_id.clone()),
        };
        
        // Store order in local cache
        {
            let mut orders = self.orders.write().await;
            orders.insert(response.clone(), order);
        }
        
        info!("✅ Order placed successfully: {}", response);
        
        // Construct proper response
        let order_response = ZerodhaOrderResponse {
            order_id: response,
        };
        
        Ok(order_response)
    }
    
    /// Modify an existing order
    pub async fn modify_order(&self, request: OrderModifyRequest) -> ZerodhaResult<ZerodhaOrderResponse> {
        info!("🔄 Modifying order: {}", request.order_id);
        
        // Use paper trading if enabled
        if let Some(ref paper_engine) = self.paper_engine {
            return paper_engine.write().await.modify_order(request).await;
        }
        
        // Check if order exists and is modifiable
        {
            let orders = self.orders.read().await;
            if let Some(order) = orders.get(&request.order_id) {
                if !self.is_order_modifiable(&order.status) {
                    return Err(ZerodhaError::execution_error(
                        format!("Order {} is not modifiable in status {:?}", request.order_id, order.status)
                    ));
                }
            } else {
                return Err(ZerodhaError::execution_error(
                    format!("Order {} not found", request.order_id)
                ));
            }
        }
        
        // Prepare modification parameters
        let mut params = HashMap::new();
        params.insert("order_id".to_string(), request.order_id.clone());
        
        if let Some(quantity) = request.quantity {
            params.insert("quantity".to_string(), quantity.to_string());
        }
        
        if let Some(price) = request.price {
            params.insert("price".to_string(), price.to_string());
        }
        
        if let Some(trigger_price) = request.trigger_price {
            params.insert("trigger_price".to_string(), trigger_price.to_string());
        }
        
        if let Some(order_type) = &request.order_type {
            params.insert("order_type".to_string(), order_type.to_string());
        }
        
        if let Some(validity) = &request.validity {
            params.insert("validity".to_string(), validity.to_string());
        }
        
        // Modify order via HTTP API
        let response = self.http_client.modify_order(params).await?;
        
        // Update local order cache
        {
            let mut orders = self.orders.write().await;
            if let Some(order) = orders.get_mut(&request.order_id) {
                if let Some(quantity) = request.quantity {
                    order.quantity = quantity;
                    order.pending_quantity = quantity.saturating_sub(order.filled_quantity);
                }
                if let Some(price) = request.price {
                    order.price = Decimal::from_f64(price).unwrap_or_default();
                }
                if let Some(trigger_price) = request.trigger_price {
                    order.trigger_price = Some(Decimal::from_f64(trigger_price).unwrap_or_default());
                }
                if let Some(order_type) = request.order_type {
                    order.order_type = order_type;
                }
                if let Some(validity) = request.validity {
                    order.validity = validity;
                }
            }
        }
        
        info!("✅ Order modified successfully: {}", request.order_id);
        Ok(response)
    }
    
    /// Cancel an existing order
    pub async fn cancel_order(&self, order_id: &str) -> ZerodhaResult<ZerodhaOrderResponse> {
        info!("❌ Cancelling order: {}", order_id);
        
        // Use paper trading if enabled
        if let Some(ref paper_engine) = self.paper_engine {
            return paper_engine.write().await.cancel_order(order_id).await;
        }
        
        // Check if order exists and is cancellable
        {
            let orders = self.orders.read().await;
            if let Some(order) = orders.get(order_id) {
                if !self.is_order_cancellable(&order.status) {
                    return Err(ZerodhaError::execution_error(
                        format!("Order {} is not cancellable in status {:?}", order_id, order.status)
                    ));
                }
            } else {
                return Err(ZerodhaError::execution_error(
                    format!("Order {} not found", order_id)
                ));
            }
        }
        
        // Cancel order via HTTP API
        let response = self.http_client.cancel_order(order_id).await?;
        
        // Update local order status
        {
            let mut orders = self.orders.write().await;
            if let Some(order) = orders.get_mut(order_id) {
                order.status = OrderStatus::CANCELLED;
                order.pending_quantity = 0;
            }
        }
        
        info!("✅ Order cancelled successfully: {}", order_id);
        Ok(response)
    }
    
    /// Get order details
    pub async fn get_order(&self, order_id: &str) -> ZerodhaResult<Option<ZerodhaOrder>> {
        // Try local cache first
        {
            let orders = self.orders.read().await;
            if let Some(order) = orders.get(order_id) {
                return Ok(Some(order.clone()));
            }
        }
        
        // Fetch from API if not in cache
        match self.http_client.get_order_details(order_id).await {
            Ok(order) => {
                // Update local cache
                let mut orders = self.orders.write().await;
                orders.insert(order_id.to_string(), order.clone());
                Ok(Some(order))
            }
            Err(_) => Ok(None),
        }
    }
    
    /// Get order history
    pub async fn get_order_history(&self, order_id: &str) -> ZerodhaResult<Vec<ZerodhaOrder>> {
        self.http_client.get_order_history(order_id).await
    }
    
    
    /// Validate order request before placement
    async fn validate_order_request(&self, request: &OrderRequest) -> ZerodhaResult<()> {
        // Basic validation
        if request.quantity == 0 {
            return Err(ZerodhaError::validation_error("Order quantity cannot be zero"));
        }
        
        if request.tradingsymbol.is_empty() {
            return Err(ZerodhaError::validation_error("Trading symbol cannot be empty"));
        }
        
        // Validate price for limit orders
        if matches!(request.order_type, OrderType::LIMIT) && request.price.is_none() {
            return Err(ZerodhaError::validation_error("Price required for limit orders"));
        }
        
        // Validate trigger price for SL orders
        if matches!(request.order_type, OrderType::SL | OrderType::SLM) 
            && request.trigger_price.is_none() {
            return Err(ZerodhaError::validation_error("Trigger price required for stop loss orders"));
        }
        
        // Check available margin for buy orders
        if request.transaction_type == "BUY" {
            let available_margin = *self.available_margin.read().await;
            if let Some(price) = request.price {
                let required_margin = price * request.quantity as f64;
                if required_margin > available_margin {
                    return Err(ZerodhaError::validation_error(
                        format!("Insufficient margin: required {}, available {}", 
                               required_margin, available_margin)
                    ));
                }
            }
        }
        
        Ok(())
    }
    
    /// Check if order can be modified
    fn is_order_modifiable(&self, status: &OrderStatus) -> bool {
        matches!(status, OrderStatus::OPEN | OrderStatus::TriggerPending)
    }
    
    /// Check if order can be cancelled
    fn is_order_cancellable(&self, status: &OrderStatus) -> bool {
        matches!(status, 
                 OrderStatus::OPEN | 
                 OrderStatus::TriggerPending)
    }
    
    /// Generate unique client order ID
    fn generate_client_order_id(&self) -> String {
        let counter = self.order_counter.fetch_add(1, Ordering::SeqCst);
        format!("NAUTILUS_{}", counter)
    }
    
    /// Get cached order count
    pub async fn get_order_count(&self) -> usize {
        self.orders.read().await.len()
    }
    
    /// Get cached position count
    pub async fn get_position_count(&self) -> usize {
        self.positions.read().await.len()
    }
    
    /// Clear order cache
    pub async fn clear_order_cache(&self) {
        self.orders.write().await.clear();
    }
    
    /// Clear position cache
    pub async fn clear_position_cache(&self) {
        self.positions.write().await.clear();
    }
    
    // =========================================================================
    // Account Information Methods
    // =========================================================================
    
    /// Get user profile information
    pub async fn get_user_profile(&self) -> ZerodhaResult<crate::types::ZerodhaUserProfile> {
        info!("👤 Fetching user profile...");
        self.http_client.get_user_profile().await
    }
    
    /// Get comprehensive account summary
    pub async fn get_account_summary(&self) -> ZerodhaResult<crate::types::ZerodhaAccountSummary> {
        info!("📊 Fetching account summary...");
        let summary = self.http_client.get_account_summary().await?;
        
        // Update local cache with latest margin info
        {
            *self.account_balance.write().await = summary.margins.available_cash;
            *self.available_margin.write().await = summary.margins.available.net;
        }
        
        info!("💰 Account Balance: ₹{:.2}", summary.margins.available_cash);
        info!("💳 Available Margin: ₹{:.2}", summary.margins.available.net);
        
        Ok(summary)
    }
    
    /// Get simplified account balance
    pub async fn get_account_balance_info(&self) -> ZerodhaResult<crate::types::ZerodhaAccountBalance> {
        info!("💰 Fetching account balance...");
        let balance = self.http_client.get_account_balance().await?;
        
        // Update local cache
        {
            *self.account_balance.write().await = balance.available_cash;
            *self.available_margin.write().await = balance.net_balance;
        }
        
        Ok(balance)
    }
    
    /// Refresh account information (balance + margin)
    pub async fn refresh_account_info(&self) -> ZerodhaResult<()> {
        info!("🔄 Refreshing account information...");
        
        let (balance, margin) = self.get_account_info().await?;
        info!("✅ Account info refreshed - Balance: ₹{:.2}, Margin: ₹{:.2}", balance, margin);
        
        Ok(())
    }
    
    /// Monitor account for margin calls and risk management
    pub async fn check_account_health(&self) -> ZerodhaResult<AccountHealthStatus> {
        let margins = self.http_client.get_margins().await?;
        
        let used_percentage = (margins.utilised.total / margins.available.total) * 100.0;
        let cash_percentage = (margins.available_cash / margins.available.total) * 100.0;
        
        let status = if used_percentage > 90.0 {
            AccountHealthStatus::Critical
        } else if used_percentage > 75.0 {
            AccountHealthStatus::Warning
        } else if cash_percentage < 10.0 {
            AccountHealthStatus::LowCash
        } else {
            AccountHealthStatus::Healthy
        };
        
        match status {
            AccountHealthStatus::Critical => {
                warn!("🚨 CRITICAL: High margin utilization {:.1}%", used_percentage);
            }
            AccountHealthStatus::Warning => {
                warn!("⚠️  WARNING: Margin utilization {:.1}%", used_percentage);
            }
            AccountHealthStatus::LowCash => {
                warn!("💸 LOW CASH: Only {:.1}% cash remaining", cash_percentage);
            }
            AccountHealthStatus::Healthy => {
                debug!("✅ Account health good - Margin: {:.1}%, Cash: {:.1}%", 
                      used_percentage, cash_percentage);
            }
        }
        
        Ok(status)
    }
    
    /// Get current margin utilization percentage
    pub async fn get_margin_utilization(&self) -> ZerodhaResult<f64> {
        let margins = self.http_client.get_margins().await?;
        let utilization = (margins.utilised.total / margins.available.total) * 100.0;
        Ok(utilization)
    }
    
    /// Check if sufficient funds are available for order
    pub async fn check_order_affordability(&self, order_value: f64, product_type: &ProductType) -> ZerodhaResult<bool> {
        let margins = self.http_client.get_margins().await?;
        
        // Different margin requirements based on product type
        let required_margin = match product_type {
            ProductType::MIS => order_value * 0.2,  // 20% for intraday
            ProductType::CNC => order_value,        // 100% for delivery
            ProductType::NRML => order_value * 0.5, // 50% for normal
            ProductType::BO => order_value * 0.3,   // 30% for bracket orders
            ProductType::CO => order_value * 0.3,   // 30% for cover orders
        };
        
        let available = margins.available.net;
        let can_afford = available >= required_margin;
        
        if !can_afford {
            warn!("💰 Insufficient funds: Required ₹{:.2}, Available ₹{:.2}", 
                  required_margin, available);
        }
        
        Ok(can_afford)
    }
    
    // =========================================================================
    // Paper Trading Methods
    // =========================================================================
    
    /// Check if paper trading is enabled
    pub fn is_paper_trading(&self) -> bool {
        self.paper_engine.is_some()
    }
    
    /// Update market price for paper trading simulation
    pub async fn update_paper_market_price(&self, symbol: &str, price: f64) -> ZerodhaResult<()> {
        if let Some(ref paper_engine) = self.paper_engine {
            paper_engine.write().await.update_market_price(symbol, price);
        }
        Ok(())
    }
    
    /// Get paper trading statistics
    pub async fn get_paper_trading_stats(&self) -> ZerodhaResult<Option<crate::execution::paper_trading::PaperTradingStats>> {
        if let Some(ref paper_engine) = self.paper_engine {
            Ok(Some(paper_engine.read().await.get_statistics()))
        } else {
            Ok(None)
        }
    }
    
    /// Get orders (paper trading aware)
    pub async fn get_orders(&self) -> ZerodhaResult<Vec<ZerodhaOrder>> {
        if let Some(ref paper_engine) = self.paper_engine {
            Ok(paper_engine.read().await.get_orders())
        } else {
            // Use regular HTTP client for live trading
            self.http_client.get_orders().await
        }
    }
    
    /// Get positions (paper trading aware)
    pub async fn get_positions(&self) -> ZerodhaResult<HashMap<String, Vec<ZerodhaPosition>>> {
        if let Some(ref paper_engine) = self.paper_engine {
            let positions = paper_engine.read().await.get_positions();
            let mut result = HashMap::new();
            // Group by exchange for compatibility
            for position in positions {
                result.entry(position.exchange.to_string()).or_insert_with(Vec::new).push(position);
            }
            Ok(result)
        } else {
            // Use regular HTTP client for live trading
            self.http_client.get_positions().await
        }
    }
    
    /// Get account info (paper trading aware)
    pub async fn get_account_info(&self) -> ZerodhaResult<(f64, f64)> {
        if let Some(ref paper_engine) = self.paper_engine {
            let engine = paper_engine.read().await;
            Ok((engine.get_balance(), engine.get_available_margin()))
        } else {
            // Use regular HTTP client for live trading
            let margins = self.http_client.get_margins().await?;
            let balance = margins.available_cash;
            let available_margin = margins.available.net;
            
            // Update local cache
            {
                *self.account_balance.write().await = balance;
                *self.available_margin.write().await = available_margin;
            }
            
            Ok((balance, available_margin))
        }
    }
}

/// Account health status for risk management
#[derive(Debug, Clone, PartialEq)]
pub enum AccountHealthStatus {
    /// Account is healthy with good margin buffer
    Healthy,
    /// Low cash balance but margin OK
    LowCash,
    /// High margin utilization (75-90%)
    Warning,
    /// Critical margin utilization (>90%)
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ZerodhaHttpConfig;
    
    #[tokio::test]
    async fn test_execution_client_creation() {
        let config = ZerodhaExecutionConfig::default();
        let http_config = ZerodhaHttpConfig::default();
        let http_client = ZerodhaHttpClient::new(http_config);
        
        let client = ZerodhaExecutionClient::new(config, http_client);
        
        assert_eq!(client.get_order_count().await, 0);
        assert_eq!(client.get_position_count().await, 0);
    }
    
    #[test]
    fn test_order_status_checks() {
        let config = ZerodhaExecutionConfig::default();
        let http_config = ZerodhaHttpConfig::default();
        let http_client = ZerodhaHttpClient::new(http_config);
        
        let client = ZerodhaExecutionClient::new(config, http_client);
        
        // Test modifiable statuses
        assert!(client.is_order_modifiable(&OrderStatus::OPEN));
        assert!(client.is_order_modifiable(&OrderStatus::TriggerPending));
        assert!(!client.is_order_modifiable(&OrderStatus::COMPLETE));
        assert!(!client.is_order_modifiable(&OrderStatus::CANCELLED));
        
        // Test cancellable statuses
        assert!(client.is_order_cancellable(&OrderStatus::OPEN));
        assert!(client.is_order_cancellable(&OrderStatus::TriggerPending));
        assert!(!client.is_order_cancellable(&OrderStatus::COMPLETE));
        assert!(!client.is_order_cancellable(&OrderStatus::CANCELLED));
    }
    
    #[test]
    fn test_client_order_id_generation() {
        let config = ZerodhaExecutionConfig::default();
        let http_config = ZerodhaHttpConfig::default();
        let http_client = ZerodhaHttpClient::new(http_config);
        
        let client = ZerodhaExecutionClient::new(config, http_client);
        
        let id1 = client.generate_client_order_id();
        let id2 = client.generate_client_order_id();
        
        assert_ne!(id1, id2);
        assert!(id1.starts_with("NAUTILUS_"));
        assert!(id2.starts_with("NAUTILUS_"));
    }
}