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

//! Test binary for Zerodha order operations and portfolio data.
//!
//! This binary demonstrates order placement, position tracking, and margin queries
//! using the Zerodha Kite Connect API. It includes safety checks for paper trading.
//!
//! Usage:
//! ```bash
//! export ZERODHA_API_KEY="your_api_key"
//! export ZERODHA_API_SECRET="your_api_secret"
//! export ZERODHA_ACCESS_TOKEN="your_access_token"
//! 
//! cargo run --bin zerodha-http-orders
//! ```

use nautilus_zerodha::{ZerodhaConfig, ZerodhaHttpClient};
use std::env;
use tracing::{error, info, warn, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting Zerodha orders and portfolio test");

    // Get configuration from environment variables
    let api_key = env::var("ZERODHA_API_KEY")
        .map_err(|_| "ZERODHA_API_KEY environment variable not set")?;
    
    let api_secret = env::var("ZERODHA_API_SECRET")
        .map_err(|_| "ZERODHA_API_SECRET environment variable not set")?;
    
    let access_token = env::var("ZERODHA_ACCESS_TOKEN")
        .map_err(|_| "ZERODHA_ACCESS_TOKEN environment variable not set")?;

    // Create configuration
    let config = ZerodhaConfig::new(api_key, api_secret, Some(access_token))
        .with_debug(true)
        .with_sandbox(true); // Always use sandbox for testing

    // Create HTTP client
    let client = ZerodhaHttpClient::new(config)?;

    // Test 1: Check existing orders
    info!("\n📋 Testing order retrieval...");
    match client.get_orders().await {
        Ok(orders) => {
            info!("✅ Successfully fetched {} orders", orders.len());
            
            if !orders.is_empty() {
                info!("📊 Recent orders:");
                for (i, order) in orders.iter().take(5).enumerate() {
                    info!("  {}. {} {} {} @ ₹{} - Status: {}", 
                          i + 1,
                          order.transaction_type,
                          order.quantity,
                          order.tradingsymbol,
                          order.price,
                          order.status);
                }
            } else {
                info!("📝 No orders found (which is normal for a new account)");
            }
        }
        Err(e) => {
            error!("❌ Failed to fetch orders: {}", e);
            if e.is_auth_error() {
                error!("💡 Authentication failed. Check your access token.");
                return Err(e.into());
            }
        }
    }

    // Test 2: Check positions
    info!("\n📈 Testing position retrieval...");
    match client.get_positions().await {
        Ok(positions) => {
            info!("✅ Successfully fetched positions data");
            
            // Check both net and day positions
            if let Some(net_positions) = positions.get("net") {
                info!("📊 Net positions: {}", net_positions.len());
                
                // Show positions with non-zero quantity
                let active_positions: Vec<_> = net_positions.iter()
                    .filter(|p| p.quantity != 0)
                    .collect();
                    
                if !active_positions.is_empty() {
                    info!("🔢 Active positions:");
                    for pos in active_positions.iter().take(5) {
                        info!("  {} {}: {} @ ₹{} (P&L: ₹{})",
                              pos.exchange,
                              pos.tradingsymbol,
                              pos.quantity,
                              pos.average_price,
                              pos.pnl);
                    }
                } else {
                    info!("📝 No active positions (which is normal for a new account)");
                }
            }
        }
        Err(e) => {
            error!("❌ Failed to fetch positions: {}", e);
        }
    }

    // Test 3: Check margins
    info!("\n💰 Testing margin retrieval...");
    match client.get_margins(None).await {
        Ok(margins) => {
            info!("✅ Successfully fetched margin data");
            info!("📊 Available cash: ₹{}", margins.available.get("cash").unwrap_or(&0.into()));
            info!("📊 Net balance: ₹{}", margins.net);
            
            // Show breakdown by segment if available
            for (segment, amount) in &margins.available {
                if *amount > 0.into() {
                    info!("  {}: ₹{}", segment, amount);
                }
            }
        }
        Err(e) => {
            error!("❌ Failed to fetch margins: {}", e);
        }
    }

    // Test 4: Demo order placement (DISABLED by default for safety)
    let demo_order = env::var("ZERODHA_DEMO_ORDER").unwrap_or_default().to_lowercase() == "true";
    
    if demo_order {
        warn!("\n⚠️  DEMO ORDER PLACEMENT ENABLED");
        warn!("This will attempt to place a small demo order!");
        warn!("Make sure you're using a paper trading account!");
        
        // Place a very small demo order for Reliance (a liquid stock)
        info!("\n🔄 Attempting demo order placement...");
        
        match client.place_order(
            nautilus_zerodha::enums::Exchange::NSE,
            "RELIANCE",  // Reliance Industries - highly liquid
            nautilus_zerodha::enums::TransactionType::BUY,
            1,  // Minimum quantity
            nautilus_zerodha::enums::Product::MIS,  // Intraday
            nautilus_zerodha::enums::OrderType::LIMIT,
            Some(2000.0.into()),  // Limit price (adjust as needed)
            None,  // No trigger price
            None,  // Default validity (DAY)
            None,  // No disclosed quantity
            Some("nautilus_test"),  // Tag for identification
        ).await {
            Ok(order_id) => {
                info!("✅ Demo order placed successfully!");
                info!("📝 Order ID: {}", order_id);
                warn!("⚠️  Remember to cancel this demo order if not needed!");
            }
            Err(e) => {
                error!("❌ Demo order placement failed: {}", e);
                match &e {
                    nautilus_zerodha::ZerodhaError::TradingNotAllowed { reason } => {
                        info!("💡 Trading restriction: {}", reason);
                        info!("   This might be due to insufficient funds, market hours, or account restrictions");
                    }
                    _ => {}
                }
            }
        }
    } else {
        info!("\n🛡️  Demo order placement disabled (set ZERODHA_DEMO_ORDER=true to enable)");
        info!("   This is for safety to prevent accidental orders");
    }

    // Test 5: Historical data sample (if available)
    info!("\n📊 Testing historical data retrieval...");
    
    // Try to get some Nifty 50 historical data
    let nifty_token = 256265_u32;  // Nifty 50 token
    let from = chrono::Utc::now() - chrono::Duration::days(7);
    let to = chrono::Utc::now();
    
    match client.get_historical_data(
        nifty_token,
        nautilus_zerodha::enums::Interval::Daily,
        from,
        to,
        None,
        None,
    ).await {
        Ok(candles) => {
            info!("✅ Successfully fetched {} historical candles", candles.len());
            
            if !candles.is_empty() {
                let latest = &candles[candles.len() - 1];
                info!("📊 Latest Nifty 50 data:");
                info!("  Date: {}", latest.timestamp.format("%Y-%m-%d"));
                info!("  OHLC: {} / {} / {} / {}", 
                      latest.open, latest.high, latest.low, latest.close);
                info!("  Volume: {}", latest.volume);
            }
        }
        Err(e) => {
            warn!("⚠️  Historical data not available: {}", e);
            info!("   This might require additional permissions or subscriptions");
        }
    }

    // Final summary
    info!("\n✅ Zerodha orders and portfolio test completed");
    info!("💡 Next steps:");
    info!("   1. Verify all data looks correct for your account");
    info!("   2. Test with small positions if doing live trading");
    info!("   3. Implement proper error handling in production code");
    
    if demo_order {
        warn!("\n⚠️  Don't forget to check and cancel the demo order if needed!");
    }

    Ok(())
}

/// Display safety warnings for order testing
#[allow(dead_code)]
fn show_safety_warnings() {
    println!("\n🚨 SAFETY WARNINGS FOR ORDER TESTING:");
    println!("1. Always use paper trading / demo accounts for testing");
    println!("2. Start with very small quantities");
    println!("3. Use limit orders with reasonable prices");
    println!("4. Monitor positions and cancel test orders promptly");
    println!("5. Verify market hours before placing orders");
    println!("6. Check margin requirements and available balance");
    println!("\n💡 Set ZERODHA_DEMO_ORDER=true to enable order placement testing");
}