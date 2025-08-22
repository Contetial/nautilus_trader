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

//! Paper trading demonstration for Zerodha adapter.
//!
//! This binary demonstrates the paper trading functionality including
//! order execution, position tracking, and P&L calculation.

use nautilus_zerodha::{
    config::{ZerodhaConfig, ZerodhaExecutionConfig, ZerodhaHttpConfig},
    enums::{Exchange, OrderType, ProductType, Validity},
    execution::{OrderRequest, ZerodhaExecutionClient},
    http::ZerodhaHttpClient,
};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("🎮 Starting Zerodha paper trading demonstration");

    // Create paper trading configuration
    let exec_config = ZerodhaExecutionConfig::paper_trading_config();
    
    info!("📋 Paper Trading Configuration:");
    info!("  Balance: ₹{:.2}", exec_config.paper_balance);
    info!("  Commission: ₹{:.2} per trade", exec_config.paper_commission);
    info!("  Max Order Value: ₹{:.2}", exec_config.max_position_value);
    info!("  Execution Delay: {}ms", exec_config.execution_delay_ms);

    // Create HTTP client (not used in paper trading but required)
    let http_config = ZerodhaHttpConfig {
        base_url: "https://api.kite.trade".to_string(),
        timeout_seconds: 30,
        rate_limit_per_second: 3,
        max_retries: 3,
        retry_delay_ms: 1000,
    };
    let http_client = ZerodhaHttpClient::new(http_config);

    // Create execution client with paper trading
    let exec_client = ZerodhaExecutionClient::new(exec_config, http_client);

    info!("✅ Paper trading client created successfully");
    info!("📊 Paper trading mode: {}", exec_client.is_paper_trading());

    // Simulate market prices for testing
    let test_symbols = vec![
        ("RELIANCE", 2800.0),
        ("TCS", 3500.0),
        ("HDFCBANK", 1600.0),
        ("INFY", 1400.0),
    ];

    for (symbol, price) in &test_symbols {
        exec_client.update_paper_market_price(symbol, *price).await?;
        info!("📈 Updated market price: {} = ₹{:.2}", symbol, price);
    }

    // Test 1: Place buy order
    info!("\n🧪 Test 1: Placing buy orders");
    
    let buy_order = OrderRequest {
        tradingsymbol: "RELIANCE".to_string(),
        exchange: Exchange::NSE,
        transaction_type: "BUY".to_string(),
        order_type: Some(OrderType::Limit),
        product: Some(ProductType::MIS),
        validity: Some(Validity::Day),
        quantity: 10,
        price: Some(2800.0), // At market price for execution
        trigger_price: None,
        disclosed_quantity: None,
        tag: Some("PAPER_BUY_1".to_string()),
    };

    let buy_response = exec_client.place_order(buy_order).await?;
    info!("✅ Buy order placed: {}", buy_response.order_id);

    // Wait for execution
    sleep(Duration::from_millis(200)).await;

    // Check account status
    let stats = exec_client.get_paper_trading_stats().await?.unwrap();
    info!("📊 Account Status After Buy:");
    info!("{}", stats);

    // Test 2: Place sell order at higher price
    info!("\n🧪 Test 2: Placing sell order for profit");
    
    // Update market price to simulate profit
    exec_client.update_paper_market_price("RELIANCE", 2850.0).await?;
    info!("📈 Market price moved to ₹2850.0");

    let sell_order = OrderRequest {
        tradingsymbol: "RELIANCE".to_string(),
        exchange: Exchange::NSE,
        transaction_type: "SELL".to_string(),
        order_type: Some(OrderType::Limit),
        product: Some(ProductType::MIS),
        validity: Some(Validity::Day),
        quantity: 5, // Partial sell
        price: Some(2850.0),
        trigger_price: None,
        disclosed_quantity: None,
        tag: Some("PAPER_SELL_1".to_string()),
    };

    let sell_response = exec_client.place_order(sell_order).await?;
    info!("✅ Sell order placed: {}", sell_response.order_id);

    // Wait for execution
    sleep(Duration::from_millis(200)).await;

    // Check updated status
    let stats = exec_client.get_paper_trading_stats().await?.unwrap();
    info!("📊 Account Status After Partial Sell:");
    info!("{}", stats);

    // Test 3: Market order
    info!("\n🧪 Test 3: Market order execution");
    
    let market_order = OrderRequest {
        tradingsymbol: "TCS".to_string(),
        exchange: Exchange::NSE,
        transaction_type: "BUY".to_string(),
        order_type: Some(OrderType::Market),
        product: Some(ProductType::MIS),
        validity: Some(Validity::Day),
        quantity: 5,
        price: None, // Market order - no price
        trigger_price: None,
        disclosed_quantity: None,
        tag: Some("PAPER_MARKET_1".to_string()),
    };

    let market_response = exec_client.place_order(market_order).await?;
    info!("✅ Market order placed: {}", market_response.order_id);

    // Wait for execution
    sleep(Duration::from_millis(200)).await;

    // Test 4: Order cancellation
    info!("\n🧪 Test 4: Order cancellation");
    
    let cancel_order = OrderRequest {
        tradingsymbol: "HDFCBANK".to_string(),
        exchange: Exchange::NSE,
        transaction_type: "BUY".to_string(),
        order_type: Some(OrderType::Limit),
        product: Some(ProductType::MIS),
        validity: Some(Validity::Day),
        quantity: 10,
        price: Some(1500.0), // Below market price - won't execute
        trigger_price: None,
        disclosed_quantity: None,
        tag: Some("PAPER_CANCEL_TEST".to_string()),
    };

    let cancel_response = exec_client.place_order(cancel_order).await?;
    info!("✅ Order to be cancelled placed: {}", cancel_response.order_id);

    // Cancel the order
    sleep(Duration::from_millis(100)).await;
    let cancel_result = exec_client.cancel_order(&cancel_response.order_id).await?;
    info!("❌ Order cancelled: {}", cancel_result.message);

    // Test 5: Check all orders and positions
    info!("\n🧪 Test 5: Final status check");
    
    let orders = exec_client.get_orders().await?;
    info!("📋 Total orders placed: {}", orders.len());
    
    for order in &orders {
        info!("  Order {}: {} {} {} @ {:?} [{}]",
              order.order_id,
              order.transaction_type,
              order.quantity,
              order.tradingsymbol,
              order.price,
              order.status);
    }

    let positions = exec_client.get_positions().await?;
    let total_positions: usize = positions.values().map(|v| v.len()).sum();
    info!("📊 Total positions: {}", total_positions);
    
    for (exchange, position_list) in positions {
        for position in position_list {
            if position.quantity != 0 {
                info!("  Position: {} {} qty: {} @ ₹{:.2} P&L: ₹{:.2}",
                      position.exchange,
                      position.tradingsymbol,
                      position.quantity,
                      position.average_price,
                      position.pnl);
            }
        }
    }

    // Final statistics
    let final_stats = exec_client.get_paper_trading_stats().await?.unwrap();
    info!("\n📈 FINAL PAPER TRADING RESULTS:");
    info!("{}", final_stats);

    // Performance metrics
    let profit_percentage = (final_stats.net_pnl / final_stats.initial_balance) * 100.0;
    info!("\n🎯 PERFORMANCE METRICS:");
    info!("💰 Return: {:.2}%", profit_percentage);
    info!("📊 Orders Executed: {}", final_stats.total_orders);
    info!("🏦 Commission Paid: ₹{:.2}", final_stats.total_commission);
    info!("📈 Win/Loss Ratio: {}", 
          if final_stats.net_pnl > 0.0 { "Profitable" } else { "Loss" });

    if final_stats.net_pnl > 0.0 {
        info!("🎉 PAPER TRADING DEMONSTRATION SUCCESSFUL!");
        info!("💡 Ready for live trading with similar strategies");
    } else {
        info!("📚 PAPER TRADING COMPLETE - Consider strategy refinement");
        info!("💡 Use paper trading to test and improve strategies");
    }

    info!("\n✨ Paper trading demonstration completed successfully!");
    info!("🚀 Paper trading mode provides:");
    info!("  • Risk-free strategy testing");
    info!("  • Real-time P&L calculation");
    info!("  • Commission simulation");
    info!("  • Order execution modeling");
    info!("  • Position tracking");

    Ok(())
}