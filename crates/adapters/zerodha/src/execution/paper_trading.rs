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

//! Paper trading simulation engine for Zerodha adapter.
//!
//! This module provides a complete paper trading implementation that simulates
//! order execution, position tracking, and P&L calculation without real money.

use crate::{
    config::ZerodhaExecutionConfig,
    error::{ZerodhaError, ZerodhaResult},
    execution::{OrderRequest, OrderModifyRequest},
    types::{ZerodhaOrder, ZerodhaOrderResponse, ZerodhaOrderStatus, ZerodhaPosition},
    enums::{OrderType, ProductType},
};
use chrono::{DateTime, Utc};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use tokio::time::{sleep, Instant};
use tracing::{debug, info, warn};

/// Paper trading engine that simulates order execution
#[derive(Debug)]
pub struct PaperTradingEngine {
    /// Configuration
    config: ZerodhaExecutionConfig,
    /// Current balance
    balance: f64,
    /// Available margin
    available_margin: f64,
    /// Active orders
    orders: HashMap<String, PaperOrder>,
    /// Positions
    positions: HashMap<String, PaperPosition>,
    /// Order ID counter
    order_counter: AtomicU64,
    /// Total P&L
    total_pnl: f64,
    /// Total commission paid
    total_commission: f64,
    /// Market data for execution simulation
    market_prices: HashMap<String, f64>,
}

/// Paper trading order representation
#[derive(Debug, Clone)]
struct PaperOrder {
    /// Order details
    order: ZerodhaOrder,
    /// Creation time
    created_at: Instant,
    /// Execution time (if filled)
    executed_at: Option<Instant>,
    /// Commission charged
    commission: f64,
}

/// Paper trading position representation
#[derive(Debug, Clone)]
struct PaperPosition {
    /// Symbol
    symbol: String,
    /// Exchange
    exchange: String,
    /// Net quantity (positive = long, negative = short)
    quantity: i32,
    /// Average price
    average_price: f64,
    /// Realized P&L
    realized_pnl: f64,
    /// Unrealized P&L
    unrealized_pnl: f64,
    /// Last price for P&L calculation
    last_price: f64,
}

impl PaperTradingEngine {
    /// Create new paper trading engine
    pub fn new(config: ZerodhaExecutionConfig) -> Self {
        if !config.paper_trading {
            panic!("Paper trading engine requires paper_trading config enabled");
        }
        
        Self {
            balance: config.paper_balance,
            available_margin: config.paper_balance,
            config,
            orders: HashMap::new(),
            positions: HashMap::new(),
            order_counter: AtomicU64::new(1),
            total_pnl: 0.0,
            total_commission: 0.0,
            market_prices: HashMap::new(),
        }
    }
    
    /// Update market price for a symbol (for execution simulation)
    pub fn update_market_price(&mut self, symbol: &str, price: f64) {
        self.market_prices.insert(symbol.to_string(), price);
        
        // Update unrealized P&L for existing positions
        for position in self.positions.values_mut() {
            if position.symbol == symbol {
                position.last_price = price;
                position.unrealized_pnl = self.calculate_unrealized_pnl(position);
            }
        }
    }
    
    /// Place a paper order
    pub async fn place_order(&mut self, request: OrderRequest) -> ZerodhaResult<ZerodhaOrderResponse> {
        info!("📋 Placing paper order: {} {} {} @ {:?}", 
              request.transaction_type, request.quantity, request.tradingsymbol, request.price);
        
        // Validate order
        self.validate_order_request(&request)?;
        
        // Generate order ID
        let order_id = self.generate_order_id();
        
        // Create paper order
        let order = ZerodhaOrder {
            order_id: order_id.clone(),
            client_order_id: request.tag.unwrap_or_else(|| format!("PAPER_{}", order_id)),
            tradingsymbol: request.tradingsymbol.clone(),
            exchange: request.exchange,
            transaction_type: request.transaction_type.clone(),
            order_type: request.order_type.unwrap_or(OrderType::Limit),
            product: request.product.unwrap_or(ProductType::MIS),
            validity: request.validity.unwrap_or(crate::enums::Validity::Day),
            quantity: request.quantity,
            price: request.price,
            trigger_price: request.trigger_price,
            disclosed_quantity: request.disclosed_quantity,
            status: ZerodhaOrderStatus::Open,
            filled_quantity: 0,
            pending_quantity: request.quantity,
            average_price: None,
            placed_by: "PAPER_TRADER".to_string(),
            order_timestamp: Utc::now(),
            exchange_timestamp: None,
            exchange_order_id: Some(format!("EX_{}", order_id)),
            parent_order_id: None,
            status_message: Some("Order placed in paper trading mode".to_string()),
            tag: request.tag,
        };
        
        // Reserve margin for buy orders
        if request.transaction_type == "BUY" {
            let required_margin = self.calculate_required_margin(&order)?;
            if required_margin > self.available_margin {
                return Err(ZerodhaError::validation_error(&format!(
                    "Insufficient margin: required ₹{:.2}, available ₹{:.2}",
                    required_margin, self.available_margin
                )));
            }
            self.available_margin -= required_margin;
        }
        
        // Store order
        let paper_order = PaperOrder {
            order: order.clone(),
            created_at: Instant::now(),
            executed_at: None,
            commission: self.config.paper_commission,
        };
        self.orders.insert(order_id.clone(), paper_order);
        
        // Simulate execution delay
        if self.config.simulate_latency {
            let delay = Duration::from_millis(self.config.execution_delay_ms);
            tokio::spawn(async move {
                sleep(delay).await;
            });
        }
        
        // Check for immediate execution (market orders or price match)
        self.try_execute_order(&order_id).await?;
        
        Ok(ZerodhaOrderResponse {
            order_id,
            status: "success".to_string(),
            message: "Paper order placed successfully".to_string(),
        })
    }
    
    /// Modify a paper order
    pub async fn modify_order(&mut self, request: OrderModifyRequest) -> ZerodhaResult<ZerodhaOrderResponse> {
        info!("📝 Modifying paper order: {}", request.order_id);
        
        let order = self.orders.get_mut(&request.order_id)
            .ok_or_else(|| ZerodhaError::validation_error("Order not found"))?;
        
        // Check if order can be modified
        if !matches!(order.order.status, ZerodhaOrderStatus::Open | ZerodhaOrderStatus::Trigger) {
            return Err(ZerodhaError::validation_error("Order cannot be modified"));
        }
        
        // Apply modifications
        if let Some(quantity) = request.quantity {
            order.order.quantity = quantity;
            order.order.pending_quantity = quantity - order.order.filled_quantity;
        }
        
        if let Some(price) = request.price {
            order.order.price = Some(price);
        }
        
        if let Some(trigger_price) = request.trigger_price {
            order.order.trigger_price = Some(trigger_price);
        }
        
        // Try execution again with new parameters
        self.try_execute_order(&request.order_id).await?;
        
        Ok(ZerodhaOrderResponse {
            order_id: request.order_id,
            status: "success".to_string(),
            message: "Paper order modified successfully".to_string(),
        })
    }
    
    /// Cancel a paper order
    pub async fn cancel_order(&mut self, order_id: &str) -> ZerodhaResult<ZerodhaOrderResponse> {
        info!("❌ Cancelling paper order: {}", order_id);
        
        let order = self.orders.get_mut(order_id)
            .ok_or_else(|| ZerodhaError::validation_error("Order not found"))?;
        
        // Check if order can be cancelled
        if matches!(order.order.status, ZerodhaOrderStatus::Complete | ZerodhaOrderStatus::Cancelled) {
            return Err(ZerodhaError::validation_error("Order cannot be cancelled"));
        }
        
        // Release reserved margin
        if order.order.transaction_type == "BUY" && order.order.filled_quantity == 0 {
            let reserved_margin = self.calculate_required_margin(&order.order)?;
            self.available_margin += reserved_margin;
        }
        
        // Update order status
        order.order.status = ZerodhaOrderStatus::Cancelled;
        order.order.pending_quantity = 0;
        order.order.status_message = Some("Order cancelled in paper trading mode".to_string());
        
        Ok(ZerodhaOrderResponse {
            order_id: order_id.to_string(),
            status: "success".to_string(),
            message: "Paper order cancelled successfully".to_string(),
        })
    }
    
    /// Try to execute an order based on market conditions
    async fn try_execute_order(&mut self, order_id: &str) -> ZerodhaResult<()> {
        let order = self.orders.get(order_id)
            .ok_or_else(|| ZerodhaError::validation_error("Order not found"))?
            .order.clone();
        
        if order.pending_quantity == 0 {
            return Ok(()); // Order already filled
        }
        
        // Get market price
        let market_price = self.market_prices.get(&order.tradingsymbol)
            .copied()
            .unwrap_or_else(|| {
                // If no market price available, simulate one based on order price
                order.price.unwrap_or(100.0) * (1.0 + rand::random::<f64>() * 0.02 - 0.01)
            });
        
        // Check execution conditions
        let should_execute = match order.order_type {
            OrderType::Market => true, // Market orders execute immediately
            OrderType::Limit => {
                if let Some(limit_price) = order.price {
                    match order.transaction_type.as_str() {
                        "BUY" => market_price <= limit_price,  // Buy if market <= limit
                        "SELL" => market_price >= limit_price, // Sell if market >= limit
                        _ => false,
                    }
                } else {
                    false
                }
            },
            OrderType::StopLoss | OrderType::StopLossMarket => {
                if let Some(trigger_price) = order.trigger_price {
                    match order.transaction_type.as_str() {
                        "BUY" => market_price >= trigger_price,  // Buy stop triggered
                        "SELL" => market_price <= trigger_price, // Sell stop triggered
                        _ => false,
                    }
                } else {
                    false
                }
            },
        };
        
        if should_execute {
            self.execute_order(order_id, market_price).await?;
        }
        
        Ok(())
    }
    
    /// Execute an order at given price
    async fn execute_order(&mut self, order_id: &str, execution_price: f64) -> ZerodhaResult<()> {
        let paper_order = self.orders.get_mut(order_id)
            .ok_or_else(|| ZerodhaError::validation_error("Order not found"))?;
        
        let order = &mut paper_order.order;
        let fill_quantity = order.pending_quantity;
        
        info!("✅ Executing paper order: {} {} @ ₹{:.2}", 
              order_id, fill_quantity, execution_price);
        
        // Update order status
        order.filled_quantity += fill_quantity;
        order.pending_quantity = 0;
        order.average_price = Some(execution_price);
        order.status = ZerodhaOrderStatus::Complete;
        order.exchange_timestamp = Some(Utc::now());
        order.status_message = Some("Order executed in paper trading mode".to_string());
        paper_order.executed_at = Some(Instant::now());
        
        // Calculate commission
        let commission = self.config.paper_commission;
        self.total_commission += commission;
        
        // Update position
        self.update_position(order, execution_price, commission).await?;
        
        // Update balance for sell orders (buy orders already reserved margin)
        if order.transaction_type == "SELL" {
            let proceeds = execution_price * fill_quantity as f64 - commission;
            self.balance += proceeds;
            self.available_margin += proceeds;
        } else {
            // Deduct commission for buy orders
            self.balance -= commission;
        }
        
        debug!("📊 Paper trading balance: ₹{:.2}, margin: ₹{:.2}", 
               self.balance, self.available_margin);
        
        Ok(())
    }
    
    /// Update position after order execution
    async fn update_position(&mut self, order: &ZerodhaOrder, price: f64, commission: f64) -> ZerodhaResult<()> {
        let position_key = format!("{}_{}", order.tradingsymbol, order.exchange);
        
        let position = self.positions.entry(position_key.clone()).or_insert_with(|| {
            PaperPosition {
                symbol: order.tradingsymbol.clone(),
                exchange: order.exchange.to_string(),
                quantity: 0,
                average_price: 0.0,
                realized_pnl: 0.0,
                unrealized_pnl: 0.0,
                last_price: price,
            }
        });
        
        let trade_quantity = match order.transaction_type.as_str() {
            "BUY" => order.filled_quantity as i32,
            "SELL" => -(order.filled_quantity as i32),
            _ => 0,
        };
        
        // Update position
        if position.quantity == 0 {
            // Opening new position
            position.quantity = trade_quantity;
            position.average_price = price;
        } else if (position.quantity > 0 && trade_quantity > 0) || 
                  (position.quantity < 0 && trade_quantity < 0) {
            // Adding to existing position
            let total_value = position.average_price * position.quantity.abs() as f64 + 
                             price * trade_quantity.abs() as f64;
            let total_quantity = position.quantity.abs() + trade_quantity.abs();
            
            position.average_price = total_value / total_quantity as f64;
            position.quantity += trade_quantity;
        } else {
            // Reducing or closing position
            let closed_quantity = trade_quantity.abs().min(position.quantity.abs());
            let pnl_per_share = if position.quantity > 0 {
                price - position.average_price  // Long position
            } else {
                position.average_price - price  // Short position
            };
            
            let realized_pnl = pnl_per_share * closed_quantity as f64 - commission;
            position.realized_pnl += realized_pnl;
            position.quantity += trade_quantity;
            
            self.total_pnl += realized_pnl;
            
            info!("💰 Realized P&L: ₹{:.2} (Total: ₹{:.2})", realized_pnl, self.total_pnl);
        }
        
        position.last_price = price;
        position.unrealized_pnl = self.calculate_unrealized_pnl(position);
        
        // Remove position if quantity is zero
        if position.quantity == 0 {
            self.positions.remove(&position_key);
        }
        
        Ok(())
    }
    
    /// Calculate unrealized P&L for a position
    fn calculate_unrealized_pnl(&self, position: &PaperPosition) -> f64 {
        if position.quantity == 0 {
            return 0.0;
        }
        
        let pnl_per_share = if position.quantity > 0 {
            position.last_price - position.average_price  // Long position
        } else {
            position.average_price - position.last_price  // Short position
        };
        
        pnl_per_share * position.quantity.abs() as f64
    }
    
    /// Validate order request
    fn validate_order_request(&self, request: &OrderRequest) -> ZerodhaResult<()> {
        if request.quantity == 0 {
            return Err(ZerodhaError::validation_error("Order quantity cannot be zero"));
        }
        
        if request.quantity > self.config.max_order_quantity {
            return Err(ZerodhaError::validation_error(&format!(
                "Order quantity {} exceeds maximum {}", 
                request.quantity, self.config.max_order_quantity
            )));
        }
        
        // Validate price for limit orders
        if matches!(request.order_type, Some(OrderType::Limit)) && request.price.is_none() {
            return Err(ZerodhaError::validation_error("Price required for limit orders"));
        }
        
        Ok(())
    }
    
    /// Calculate required margin for an order
    fn calculate_required_margin(&self, order: &ZerodhaOrder) -> ZerodhaResult<f64> {
        let order_value = order.price.unwrap_or(100.0) * order.quantity as f64;
        
        // Different margin requirements based on product type
        let margin_factor = match order.product {
            ProductType::MIS => 0.2,   // 20% for intraday
            ProductType::CNC => 1.0,   // 100% for delivery
            ProductType::NRML => 0.5,  // 50% for normal
        };
        
        Ok(order_value * margin_factor)
    }
    
    /// Generate unique order ID
    fn generate_order_id(&self) -> String {
        let counter = self.order_counter.fetch_add(1, Ordering::SeqCst);
        format!("PAPER_{:06}", counter)
    }
    
    /// Get all orders
    pub fn get_orders(&self) -> Vec<ZerodhaOrder> {
        self.orders.values().map(|po| po.order.clone()).collect()
    }
    
    /// Get all positions
    pub fn get_positions(&self) -> Vec<ZerodhaPosition> {
        self.positions.values().map(|pp| ZerodhaPosition {
            tradingsymbol: pp.symbol.clone(),
            exchange: pp.exchange.clone(),
            instrument_token: 0, // Not used in paper trading
            product: ProductType::MIS,
            quantity: pp.quantity,
            overnight_quantity: 0,
            multiplier: 1.0,
            average_price: pp.average_price,
            close_price: pp.last_price,
            last_price: pp.last_price,
            value: pp.average_price * pp.quantity.abs() as f64,
            pnl: pp.realized_pnl + pp.unrealized_pnl,
            m2m: pp.unrealized_pnl,
            unrealised: pp.unrealized_pnl,
            realised: pp.realized_pnl,
        }).collect()
    }
    
    /// Get account balance
    pub fn get_balance(&self) -> f64 {
        self.balance
    }
    
    /// Get available margin
    pub fn get_available_margin(&self) -> f64 {
        self.available_margin
    }
    
    /// Get trading statistics
    pub fn get_statistics(&self) -> PaperTradingStats {
        let total_unrealized_pnl: f64 = self.positions.values()
            .map(|p| p.unrealized_pnl)
            .sum();
        
        PaperTradingStats {
            initial_balance: self.config.paper_balance,
            current_balance: self.balance,
            available_margin: self.available_margin,
            total_realized_pnl: self.total_pnl,
            total_unrealized_pnl,
            total_commission: self.total_commission,
            total_orders: self.orders.len(),
            active_positions: self.positions.len(),
            net_pnl: self.total_pnl + total_unrealized_pnl,
        }
    }
}

/// Paper trading statistics
#[derive(Debug, Clone)]
pub struct PaperTradingStats {
    pub initial_balance: f64,
    pub current_balance: f64,
    pub available_margin: f64,
    pub total_realized_pnl: f64,
    pub total_unrealized_pnl: f64,
    pub total_commission: f64,
    pub total_orders: usize,
    pub active_positions: usize,
    pub net_pnl: f64,
}

impl std::fmt::Display for PaperTradingStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, 
            "Paper Trading Stats:\n\
             Initial Balance: ₹{:.2}\n\
             Current Balance: ₹{:.2}\n\
             Available Margin: ₹{:.2}\n\
             Realized P&L: ₹{:.2}\n\
             Unrealized P&L: ₹{:.2}\n\
             Net P&L: ₹{:.2}\n\
             Total Commission: ₹{:.2}\n\
             Total Orders: {}\n\
             Active Positions: {}",
            self.initial_balance,
            self.current_balance,
            self.available_margin,
            self.total_realized_pnl,
            self.total_unrealized_pnl,
            self.net_pnl,
            self.total_commission,
            self.total_orders,
            self.active_positions
        )
    }
}

// Simple random number generation for price simulation
mod rand {
    use std::sync::atomic::{AtomicU64, Ordering};
    
    static SEED: AtomicU64 = AtomicU64::new(1);
    
    pub fn random<T>() -> T
    where
        T: From<f64>,
    {
        let seed = SEED.fetch_add(1, Ordering::Relaxed);
        let x = seed.wrapping_mul(1103515245).wrapping_add(12345);
        let normalized = (x as f64) / (u64::MAX as f64);
        T::from(normalized)
    }
}