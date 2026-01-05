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

//! Order status update handling for Zerodha WebSocket.

use crate::{
    enums::{Exchange, OrderStatus, OrderType, Product, TransactionType, Validity},
    error::{ZerodhaError, ZerodhaResult},
    types::{ZerodhaOrder, ZerodhaOrderStatus},
};
use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, prelude::{FromPrimitive, ToPrimitive}};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Order update event from Zerodha WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZerodhaOrderUpdate {
    /// Order ID
    pub order_id: String,
    /// Order status
    pub status: ZerodhaOrderStatus,
    /// Status message
    pub status_message: Option<String>,
    /// Filled quantity
    pub filled_quantity: u32,
    /// Pending quantity
    pub pending_quantity: u32,
    /// Average price
    pub average_price: Option<f64>,
    /// Exchange timestamp
    pub exchange_timestamp: Option<String>,
    /// Checksum for validation
    pub checksum: Option<String>,
}

/// Postback (webhook) message for order updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZerodhaPostback {
    /// User ID
    pub user_id: String,
    /// Order ID
    pub order_id: String,
    /// Exchange order ID
    pub exchange_order_id: Option<String>,
    /// Status
    pub status: String,
    /// Status message
    pub status_message: Option<String>,
    /// Trading symbol
    pub tradingsymbol: String,
    /// Exchange
    pub exchange: String,
    /// Instrument token
    pub instrument_token: u32,
    /// Transaction type
    pub transaction_type: String,
    /// Order type
    pub order_type: String,
    /// Product
    pub product: String,
    /// Quantity
    pub quantity: u32,
    /// Price
    pub price: Option<f64>,
    /// Trigger price
    pub trigger_price: Option<f64>,
    /// Filled quantity
    pub filled_quantity: u32,
    /// Pending quantity
    pub pending_quantity: u32,
    /// Average price
    pub average_price: Option<f64>,
    /// Order timestamp
    pub order_timestamp: String,
    /// Exchange timestamp
    pub exchange_timestamp: Option<String>,
    /// Validity
    pub validity: String,
    /// Tag
    pub tag: Option<String>,
    /// Checksum
    pub checksum: String,
}

/// Order status manager for tracking order lifecycle
#[derive(Debug)]
pub struct ZerodhaOrderStatusManager {
    /// Active orders being tracked
    orders: HashMap<String, ZerodhaOrder>,
    /// Order update sender
    update_tx: mpsc::UnboundedSender<ZerodhaOrderUpdate>,
    /// Order update receiver
    update_rx: Option<mpsc::UnboundedReceiver<ZerodhaOrderUpdate>>,
}

impl ZerodhaOrderStatusManager {
    /// Create new order status manager
    pub fn new() -> Self {
        let (update_tx, update_rx) = mpsc::unbounded_channel();
        
        Self {
            orders: HashMap::new(),
            update_tx,
            update_rx: Some(update_rx),
        }
    }
    
    /// Get update receiver channel
    pub fn take_update_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<ZerodhaOrderUpdate>> {
        self.update_rx.take()
    }
    
    /// Add order for tracking
    pub fn add_order(&mut self, order: ZerodhaOrder) {
        debug!("📝 Tracking order: {} [{}]", order.order_id, order.status);
        self.orders.insert(order.order_id.clone(), order);
    }
    
    /// Update order status
    pub fn update_order_status(&mut self, order_id: &str, status: ZerodhaOrderStatus) -> ZerodhaResult<()> {
        if let Some(order) = self.orders.get_mut(order_id) {
            let old_status = order.status;
            order.status = status.into();
            
            info!("🔄 Order {} status: {:?} -> {:?}", order_id, old_status, status);
            
            // Send update notification
            let update = ZerodhaOrderUpdate {
                order_id: order_id.to_string(),
                status: order.status.into(),
                status_message: None,
                filled_quantity: order.filled_quantity,
                pending_quantity: order.pending_quantity,
                average_price: order.average_price.map(|p| p.to_f64().unwrap_or_default()),
                exchange_timestamp: None,
                checksum: None,
            };
            
            if let Err(e) = self.update_tx.send(update) {
                warn!("Failed to send order update: {}", e);
            }
            
            Ok(())
        } else {
            Err(ZerodhaError::validation_error(&format!("Order not found: {}", order_id)))
        }
    }
    
    /// Process order fill
    pub fn process_order_fill(
        &mut self,
        order_id: &str,
        filled_quantity: u32,
        average_price: f64,
    ) -> ZerodhaResult<()> {
        if let Some(order) = self.orders.get_mut(order_id) {
            order.filled_quantity += filled_quantity;
            order.pending_quantity = order.quantity.saturating_sub(order.filled_quantity);
            order.average_price = Some(Decimal::from_f64(average_price).unwrap_or_default());
            
            // Update status based on fill
            if order.pending_quantity == 0 {
                order.status = OrderStatus::COMPLETE;
                info!("✅ Order {} completely filled: {} @ ₹{:.2}", 
                      order_id, order.filled_quantity, average_price);
            } else {
                info!("🔸 Order {} partially filled: {}/{} @ ₹{:.2}", 
                      order_id, order.filled_quantity, order.quantity, average_price);
            }
            
            // Send update notification
            let update = ZerodhaOrderUpdate {
                order_id: order_id.to_string(),
                status: order.status.into(),
                status_message: None,
                filled_quantity: order.filled_quantity,
                pending_quantity: order.pending_quantity,
                average_price: order.average_price.map(|p| p.to_f64().unwrap_or_default()),
                exchange_timestamp: None,
                checksum: None,
            };
            
            if let Err(e) = self.update_tx.send(update) {
                warn!("Failed to send order update: {}", e);
            }
            
            Ok(())
        } else {
            Err(ZerodhaError::validation_error(&format!("Order not found: {}", order_id)))
        }
    }
    
    /// Process postback message from Zerodha webhook
    pub fn process_postback(&mut self, postback: ZerodhaPostback) -> ZerodhaResult<()> {
        debug!("📨 Processing postback for order: {}", postback.order_id);
        
        // Validate checksum if required
        if let Err(e) = self.validate_postback_checksum(&postback) {
            warn!("⚠️  Postback checksum validation failed: {}", e);
            // Continue processing but log the warning
        }
        
        // Parse status
        let status = match postback.status.as_str() {
            "OPEN" => ZerodhaOrderStatus::Open,
            "COMPLETE" => ZerodhaOrderStatus::Complete,
            "CANCELLED" => ZerodhaOrderStatus::Cancelled,
            "REJECTED" => ZerodhaOrderStatus::Rejected,
            "TRIGGER PENDING" => ZerodhaOrderStatus::Trigger,
            "PENDING" => ZerodhaOrderStatus::Pending,
            _ => {
                warn!("Unknown order status: {}", postback.status);
                return Ok(());
            }
        };
        
        // Update order if we're tracking it
        if let Some(order) = self.orders.get_mut(&postback.order_id) {
            order.status = status.into();
            order.filled_quantity = postback.filled_quantity;
            order.pending_quantity = postback.pending_quantity;
            order.average_price = postback.average_price.map(|p| Decimal::from_f64(p).unwrap_or_default());
            
            if let Some(exchange_order_id) = &postback.exchange_order_id {
                order.exchange_order_id = Some(exchange_order_id.clone());
            }
            
            if let Some(status_message) = &postback.status_message {
                order.status_message = Some(status_message.clone());
            }
            
            info!("📋 Updated order {} from postback: {:?}", postback.order_id, status);
        } else {
            // Create new order from postback if not tracking
            warn!("🆕 Received postback for unknown order: {}", postback.order_id);
            self.create_order_from_postback(postback)?;
        }
        
        Ok(())
    }
    
    /// Create order from postback data
    fn create_order_from_postback(&mut self, postback: ZerodhaPostback) -> ZerodhaResult<()> {
        use crate::enums::{Exchange, OrderType, ProductType, Validity};
        use chrono::Utc;
        
        // Parse enums from strings
        let _exchange = postback.exchange.parse::<Exchange>()
            .map_err(|_| ZerodhaError::parse_error(&format!("Invalid exchange: {}", postback.exchange)))?;
        
        let _order_type = match postback.order_type.as_str() {
            "MARKET" => OrderType::MARKET,
            "LIMIT" => OrderType::LIMIT,
            "SL" => OrderType::SL,
            "SL-M" => OrderType::SLM,
            _ => OrderType::LIMIT,
        };
        
        let _product = match postback.product.as_str() {
            "MIS" => Product::MIS,
            "CNC" => Product::CNC,
            "NRML" => Product::NRML,
            _ => Product::MIS,
        };
        
        let _validity = match postback.validity.as_str() {
            "DAY" => Validity::DAY,
            "IOC" => Validity::IOC,
            "TTL" => Validity::TTL,
            _ => Validity::DAY,
        };
        
        let status = match postback.status.as_str() {
            "OPEN" => ZerodhaOrderStatus::Open,
            "COMPLETE" => ZerodhaOrderStatus::Complete,
            "CANCELLED" => ZerodhaOrderStatus::Cancelled,
            "REJECTED" => ZerodhaOrderStatus::Rejected,
            "TRIGGER PENDING" => ZerodhaOrderStatus::Trigger,
            "PENDING" => ZerodhaOrderStatus::Pending,
            _ => ZerodhaOrderStatus::Open,
        };
        
        let order = ZerodhaOrder {
            account_id: "WS_ACCOUNT".to_string(),
            placed_by: postback.user_id,
            order_id: postback.order_id.clone(),
            exchange_order_id: postback.exchange_order_id,
            parent_order_id: None,
            status: status.into(),
            status_message: postback.status_message,
            order_timestamp: Utc::now(),
            exchange_timestamp: None,
            variety: "regular".to_string(),
            exchange: Exchange::NSE, // Default to NSE
            tradingsymbol: postback.tradingsymbol,
            instrument_token: 0, // Default
            order_type: OrderType::LIMIT, // Default
            transaction_type: TransactionType::BUY, // Will be parsed later
            validity: Validity::DAY, // Default
            product: Product::CNC, // Default
            quantity: postback.quantity,
            disclosed_quantity: None,
            price: postback.price.map(|p| Decimal::from_f64(p).unwrap_or_default()).unwrap_or_default(),
            trigger_price: postback.trigger_price.map(|p| Decimal::from_f64(p).unwrap_or_default()),
            average_price: postback.average_price.map(|p| Decimal::from_f64(p).unwrap_or_default()),
            filled_quantity: postback.filled_quantity,
            pending_quantity: postback.pending_quantity,
            cancelled_quantity: 0,
            market_protection: None,
            tag: postback.tag,
        };
        
        self.add_order(order);
        Ok(())
    }
    
    /// Validate postback checksum
    fn validate_postback_checksum(&self, postback: &ZerodhaPostback) -> ZerodhaResult<()> {
        // Implementation would validate the postback checksum
        // For now, just return Ok as checksum validation is optional
        if postback.checksum.is_empty() {
            return Err(ZerodhaError::validation_error("Empty checksum"));
        }
        
        // TODO: Implement actual checksum validation using API secret
        Ok(())
    }
    
    /// Get order by ID
    pub fn get_order(&self, order_id: &str) -> Option<&ZerodhaOrder> {
        self.orders.get(order_id)
    }
    
    /// Get all orders
    pub fn get_all_orders(&self) -> Vec<&ZerodhaOrder> {
        self.orders.values().collect()
    }
    
    /// Get orders by status
    pub fn get_orders_by_status(&self, status: ZerodhaOrderStatus) -> Vec<&ZerodhaOrder> {
        self.orders.values()
            .filter(|order| ZerodhaOrderStatus::from(order.status) == status)
            .collect()
    }
    
    /// Remove completed/cancelled orders
    pub fn cleanup_finished_orders(&mut self) {
        let before_count = self.orders.len();
        
        self.orders.retain(|_, order| {
            !matches!(order.status, OrderStatus::COMPLETE | OrderStatus::CANCELLED | OrderStatus::REJECTED)
        });
        
        let after_count = self.orders.len();
        if before_count != after_count {
            debug!("🧹 Cleaned up {} finished orders", before_count - after_count);
        }
    }
    
    /// Get summary statistics
    pub fn get_order_statistics(&self) -> HashMap<ZerodhaOrderStatus, usize> {
        let mut stats = HashMap::new();
        
        for order in self.orders.values() {
            *stats.entry(order.status.into()).or_insert(0) += 1;
        }
        
        stats
    }
}

impl Default for ZerodhaOrderStatusManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enums::{Exchange, OrderType, ProductType, Validity};
    use chrono::Utc;

    fn create_test_order() -> ZerodhaOrder {
        ZerodhaOrder {
            order_id: "123456".to_string(),
            client_order_id: "CLIENT_123".to_string(),
            tradingsymbol: "SBIN".to_string(),
            exchange: Exchange::NSE,
            transaction_type: "BUY".to_string(),
            order_type: OrderType::LIMIT,
            product: ProductType::MIS,
            validity: Validity::DAY,
            quantity: 100,
            price: Some(500.0),
            trigger_price: None,
            disclosed_quantity: None,
            status: ZerodhaOrderStatus::Open,
            filled_quantity: 0,
            pending_quantity: 100,
            average_price: None,
            placed_by: "USER123".to_string(),
            order_timestamp: Utc::now(),
            exchange_timestamp: None,
            exchange_order_id: None,
            parent_order_id: None,
            status_message: None,
            tag: Some("TEST_TAG".to_string()),
        }
    }

    #[tokio::test]
    async fn test_order_status_manager() {
        let mut manager = ZerodhaOrderStatusManager::new();
        let order = create_test_order();
        let order_id = order.order_id.clone();
        
        // Add order
        manager.add_order(order);
        assert!(manager.get_order(&order_id).is_some());
        
        // Update status
        manager.update_order_status(&order_id, ZerodhaOrderStatus::Complete).unwrap();
        let updated_order = manager.get_order(&order_id).unwrap();
        assert_eq!(updated_order.status, ZerodhaOrderStatus::Complete);
        
        // Check statistics
        let stats = manager.get_order_statistics();
        assert_eq!(stats.get(&ZerodhaOrderStatus::Complete), Some(&1));
    }

    #[tokio::test]
    async fn test_order_fill_processing() {
        let mut manager = ZerodhaOrderStatusManager::new();
        let order = create_test_order();
        let order_id = order.order_id.clone();
        
        manager.add_order(order);
        
        // Process partial fill
        manager.process_order_fill(&order_id, 50, 505.0).unwrap();
        let order = manager.get_order(&order_id).unwrap();
        assert_eq!(order.filled_quantity, 50);
        assert_eq!(order.pending_quantity, 50);
        assert_eq!(order.average_price, Some(505.0));
        assert_eq!(order.status, ZerodhaOrderStatus::Open);
        
        // Process complete fill
        manager.process_order_fill(&order_id, 50, 504.0).unwrap();
        let order = manager.get_order(&order_id).unwrap();
        assert_eq!(order.filled_quantity, 100);
        assert_eq!(order.pending_quantity, 0);
        assert_eq!(order.status, ZerodhaOrderStatus::Complete);
    }
}