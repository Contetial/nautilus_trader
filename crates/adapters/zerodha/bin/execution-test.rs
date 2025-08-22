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

//! Test binary for Zerodha execution client.

use nautilus_zerodha::{
    config::{ZerodhaConfig, ZerodhaExecutionConfig, ZerodhaHttpConfig},
    enums::{Exchange, OrderType, ProductType, Validity},
    execution::{OrderRequest, ZerodhaExecutionClient},
    http::ZerodhaHttpClient,
};
use std::env;
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("🚀 Starting Zerodha execution client test");

    // Load configuration from environment
    let api_key = env::var("ZERODHA_API_KEY")
        .expect("ZERODHA_API_KEY environment variable not set");
    let api_secret = env::var("ZERODHA_API_SECRET")
        .expect("ZERODHA_API_SECRET environment variable not set");
    let access_token = env::var("ZERODHA_ACCESS_TOKEN").ok();

    // Create HTTP client configuration
    let http_config = ZerodhaHttpConfig {
        base_url: "https://api.kite.trade".to_string(),
        timeout_seconds: 30,
        rate_limit_per_second: 3,
        max_retries: 3,
        retry_delay_ms: 1000,
    };

    // Create main configuration
    let config = ZerodhaConfig::new(api_key, api_secret, access_token);

    // Create HTTP client
    let http_client = ZerodhaHttpClient::new(http_config);

    // Create execution configuration
    let exec_config = ZerodhaExecutionConfig {
        max_order_quantity: 1000,
        enable_validation: true,
        max_position_value: 100_000.0, // 1 Lakh INR
        enable_margin_check: true,
        default_validity: Validity::Day,
        default_product_type: ProductType::MIS,
    };

    // Create execution client
    let exec_client = ZerodhaExecutionClient::new(exec_config, http_client);

    info!("✅ Execution client created successfully");

    // Test account information retrieval
    info!("📊 Fetching account information...");
    match exec_client.get_account_info().await {
        Ok((balance, margin)) => {
            info!("💰 Account Balance: ₹{:.2}", balance);
            info!("💳 Available Margin: ₹{:.2}", margin);
        }
        Err(e) => {
            eprintln!("❌ Failed to fetch account info: {}", e);
        }
    }

    // Test user profile retrieval
    info!("👤 Fetching user profile...");
    match exec_client.get_user_profile().await {
        Ok(profile) => {
            info!("✅ User Profile: {} ({})", profile.user_name, profile.user_id);
            info!("📧 Email: {}", profile.email);
            info!("🏢 Broker: {}", profile.broker);
        }
        Err(e) => {
            eprintln!("❌ Failed to fetch user profile: {}", e);
        }
    }

    // Test account summary
    info!("📋 Fetching account summary...");
    match exec_client.get_account_summary().await {
        Ok(summary) => {
            info!("✅ Account Summary loaded");
            info!("👤 User: {}", summary.profile.user_name);
            info!("💰 Cash: ₹{:.2}", summary.margins.available_cash);
            info!("💳 Net Margin: ₹{:.2}", summary.margins.available.net);
            info!("📊 Used Margin: ₹{:.2}", summary.margins.utilised.total);
        }
        Err(e) => {
            eprintln!("❌ Failed to fetch account summary: {}", e);
        }
    }

    // Test account health check
    info!("🩺 Checking account health...");
    match exec_client.check_account_health().await {
        Ok(health) => {
            info!("✅ Account Health: {:?}", health);
        }
        Err(e) => {
            eprintln!("❌ Failed to check account health: {}", e);
        }
    }

    // Test margin utilization
    info!("📊 Checking margin utilization...");
    match exec_client.get_margin_utilization().await {
        Ok(utilization) => {
            info!("📈 Margin Utilization: {:.1}%", utilization);
        }
        Err(e) => {
            eprintln!("❌ Failed to get margin utilization: {}", e);
        }
    }

    // Test fetching existing orders
    info!("📋 Fetching existing orders...");
    match exec_client.get_orders().await {
        Ok(orders) => {
            info!("📄 Found {} existing orders", orders.len());
            for order in orders.iter().take(5) {
                info!("  Order: {} {} {} @ {:?} [{}]", 
                      order.transaction_type,
                      order.quantity,
                      order.tradingsymbol,
                      order.price,
                      order.status);
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to fetch orders: {}", e);
        }
    }

    // Test fetching positions
    info!("📊 Fetching current positions...");
    match exec_client.get_positions().await {
        Ok(positions) => {
            let total_positions: usize = positions.values().map(|v| v.len()).sum();
            info!("🎯 Found {} total positions", total_positions);
            
            for (segment, position_list) in positions.iter() {
                info!("  {}: {} positions", segment, position_list.len());
                for position in position_list.iter().take(3) {
                    if position.quantity != 0 {
                        info!("    {} {} qty: {} @ ₹{:.2} P&L: ₹{:.2}",
                              position.exchange,
                              position.tradingsymbol,
                              position.quantity,
                              position.average_price,
                              position.pnl);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to fetch positions: {}", e);
        }
    }

    // Test order validation (without placing)
    info!("🔍 Testing order validation...");
    let test_order = OrderRequest {
        tradingsymbol: "SBIN".to_string(),
        exchange: Exchange::NSE,
        transaction_type: "BUY".to_string(),
        order_type: OrderType::Limit,
        product: ProductType::MIS,
        validity: Validity::Day,
        quantity: 1,
        price: Some(500.0),
        trigger_price: None,
        disclosed_quantity: None,
        tag: Some("TEST_ORDER".to_string()),
    };

    // Note: This will validate but not actually place the order
    // To test actual order placement, uncomment the following:
    /*
    match exec_client.place_order(test_order).await {
        Ok(response) => {
            info!("✅ Test order placed successfully: {}", response.order_id);
            
            // Test order cancellation
            info!("❌ Cancelling test order...");
            match exec_client.cancel_order(&response.order_id).await {
                Ok(_) => info!("✅ Order cancelled successfully"),
                Err(e) => eprintln!("❌ Failed to cancel order: {}", e),
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to place test order: {}", e);
        }
    }
    */

    info!("✅ Execution client test completed successfully");
    info!("ℹ️  To test actual order placement, uncomment the order placement code");

    Ok(())
}