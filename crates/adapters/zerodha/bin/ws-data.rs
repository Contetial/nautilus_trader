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

//! Test binary for Zerodha WebSocket real-time data feed.
//!
//! This binary demonstrates real-time market data streaming using Zerodha's
//! Kite Ticker WebSocket API. It subscribes to popular instruments and
//! displays live tick data.
//!
//! Usage:
//! ```bash
//! export ZERODHA_API_KEY="your_api_key"
//! export ZERODHA_API_SECRET="your_api_secret"
//! export ZERODHA_ACCESS_TOKEN="your_access_token"
//! 
//! cargo run --bin zerodha-ws-data
//! ```

use nautilus_zerodha::{
    config::{ZerodhaConfig, ZerodhaWebSocketConfig},
    enums::TickerMode,
    websocket::ZerodhaWebSocketClient,
    ZerodhaHttpClient,
};
use std::{env, time::Duration};
use tokio::{sync::mpsc, time::timeout};
use tracing::{error, info, warn, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting Zerodha WebSocket data feed test");

    // Get configuration from environment variables
    let api_key = env::var("ZERODHA_API_KEY")
        .map_err(|_| "ZERODHA_API_KEY environment variable not set")?;
    
    let api_secret = env::var("ZERODHA_API_SECRET")
        .map_err(|_| "ZERODHA_API_SECRET environment variable not set")?;
    
    let access_token = env::var("ZERODHA_ACCESS_TOKEN")
        .map_err(|_| "ZERODHA_ACCESS_TOKEN environment variable not set")?;

    // Create HTTP client first to fetch instrument information
    let config = ZerodhaConfig::new(api_key.clone(), api_secret, Some(access_token.clone()))
        .with_debug(true);

    let http_client = ZerodhaHttpClient::new(config)?;

    // Get some popular instruments to subscribe to
    info!("📊 Fetching instruments for subscription...");
    let instruments = match http_client.get_instruments().await {
        Ok(instruments) => instruments,
        Err(e) => {
            error!("❌ Failed to fetch instruments: {}", e);
            if e.is_auth_error() {
                error!("💡 Check your API credentials and access token");
                return Err(e.into());
            }
            return Err(e.into());
        }
    };

    // Find popular instruments to subscribe to
    let mut subscription_tokens = Vec::new();
    let mut instrument_names = std::collections::HashMap::new();

    // Add Nifty 50 (most popular index)
    if let Some(nifty) = instruments.iter().find(|i| i.tradingsymbol == "NIFTY 50") {
        subscription_tokens.push(nifty.instrument_token);
        instrument_names.insert(nifty.instrument_token, "Nifty 50".to_string());
    }

    // Add Bank Nifty
    if let Some(banknifty) = instruments.iter().find(|i| i.tradingsymbol == "NIFTY BANK") {
        subscription_tokens.push(banknifty.instrument_token);
        instrument_names.insert(banknifty.instrument_token, "Bank Nifty".to_string());
    }

    // Add some popular stocks
    let popular_stocks = ["RELIANCE", "TCS", "HDFCBANK", "ICICIBANK", "INFY"];
    for stock in &popular_stocks {
        if let Some(instrument) = instruments.iter().find(|i| 
            i.tradingsymbol == *stock && 
            i.exchange == nautilus_zerodha::enums::Exchange::NSE
        ) {
            subscription_tokens.push(instrument.instrument_token);
            instrument_names.insert(instrument.instrument_token, stock.to_string());
        }
    }

    if subscription_tokens.is_empty() {
        warn!("⚠️  No instruments found for subscription");
        return Ok(());
    }

    info!("📋 Found {} instruments to subscribe to:", subscription_tokens.len());
    for (token, name) in &instrument_names {
        info!("  {} - {}", name, token);
    }

    // Create WebSocket configuration
    let ws_config = ZerodhaWebSocketConfig {
        api_key: api_key.clone(),
        access_token: access_token.clone(),
        url: "wss://ws.kite.trade".to_string(),
        auto_reconnect: true,
        max_reconnect_attempts: 5,
        ping_interval: Duration::from_secs(30),
        connect_timeout: Duration::from_secs(10),
        reconnect_delay: Duration::from_secs(5),
    };

    // Create tick data channel
    let (tick_tx, mut tick_rx) = mpsc::unbounded_channel();

    // Create and start WebSocket client
    let mut ws_client = ZerodhaWebSocketClient::new(ws_config, tick_tx);

    info!("🔌 Starting WebSocket connection...");
    if let Err(e) = ws_client.start().await {
        error!("❌ Failed to start WebSocket client: {}", e);
        return Err(e.into());
    }

    // Wait a moment for connection to establish
    tokio::time::sleep(Duration::from_secs(2)).await;

    if !ws_client.is_connected() {
        warn!("⚠️  WebSocket not connected yet, subscription may fail");
    }

    // Subscribe to instruments
    info!("📡 Subscribing to {} instruments...", subscription_tokens.len());
    
    // Subscribe with different modes for demonstration
    let basic_tokens = subscription_tokens[..std::cmp::min(2, subscription_tokens.len())].to_vec();
    let full_tokens = subscription_tokens[std::cmp::min(2, subscription_tokens.len())..].to_vec();

    if !basic_tokens.is_empty() {
        match ws_client.subscribe(basic_tokens.clone(), TickerMode::Quote).await {
            Ok(_) => info!("✅ Subscribed to {} instruments with Quote mode", basic_tokens.len()),
            Err(e) => error!("❌ Quote subscription failed: {}", e),
        }
    }

    if !full_tokens.is_empty() {
        match ws_client.subscribe(full_tokens.clone(), TickerMode::Full).await {
            Ok(_) => info!("✅ Subscribed to {} instruments with Full mode", full_tokens.len()),
            Err(e) => error!("❌ Full subscription failed: {}", e),
        }
    }

    // Display market status message
    info!("\n📊 Starting market data stream...");
    info!("💡 Note: Live data is only available during market hours (9:15 AM - 3:30 PM IST)");
    info!("   Outside market hours, you may see minimal or no tick data");
    info!("\n🎯 Press Ctrl+C to stop\n");

    // Statistics tracking
    let mut tick_count = 0;
    let mut last_stats = std::time::Instant::now();
    let mut instrument_stats = std::collections::HashMap::new();

    // Listen for tick data with timeout to show periodic status
    loop {
        // Use timeout to periodically show status even without ticks
        match timeout(Duration::from_secs(10), tick_rx.recv()).await {
            Ok(Some(tick)) => {
                tick_count += 1;
                
                // Update instrument statistics
                let counter = instrument_stats.entry(tick.instrument_token).or_insert(0);
                *counter += 1;

                // Display tick information
                let instrument_name = instrument_names
                    .get(&tick.instrument_token)
                    .unwrap_or(&format!("Token_{}", tick.instrument_token));

                info!("📊 {} ({}): LTP=₹{} | Volume={:?} | Mode={:?}",
                      instrument_name,
                      tick.instrument_token,
                      tick.last_price,
                      tick.volume_traded,
                      tick.mode);

                // Show detailed data for full mode ticks
                if tick.mode == TickerMode::Full {
                    if let Some(ohlc) = &tick.ohlc {
                        info!("    OHLC: {}/{}/{}/{} | Change: {:?}",
                              ohlc.open, ohlc.high, ohlc.low, ohlc.close,
                              tick.change);
                    }
                    
                    if let Some(oi) = tick.oi {
                        info!("    Open Interest: {} | OI Change: {:?}",
                              oi, tick.oi_change);
                    }

                    if let Some(depth) = &tick.depth {
                        info!("    Depth: {} bids, {} offers",
                              depth.buy.len(), depth.sell.len());
                        
                        // Show best bid/offer
                        if !depth.buy.is_empty() && !depth.sell.is_empty() {
                            info!("    Best: Bid ₹{} ({}) | Ask ₹{} ({})",
                                  depth.buy[0].price, depth.buy[0].quantity,
                                  depth.sell[0].price, depth.sell[0].quantity);
                        }
                    }
                }

                // Show statistics every 30 seconds
                if last_stats.elapsed() >= Duration::from_secs(30) {
                    show_statistics(tick_count, &instrument_stats, &instrument_names);
                    last_stats = std::time::Instant::now();
                }
            }
            Ok(None) => {
                info!("📡 Tick channel closed");
                break;
            }
            Err(_) => {
                // Timeout - show status
                if ws_client.is_connected() {
                    info!("📡 WebSocket connected | Received {} ticks so far", tick_count);
                    
                    if tick_count == 0 {
                        warn!("⚠️  No tick data received yet - this is normal outside market hours");
                        info!("   Market hours: Monday-Friday, 9:15 AM - 3:30 PM IST");
                        info!("   Current time: {}", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"));
                    }
                } else {
                    warn!("⚠️  WebSocket disconnected - attempting to reconnect...");
                }
            }
        }
    }

    info!("✅ WebSocket data feed test completed");
    Ok(())
}

fn show_statistics(
    total_ticks: u64, 
    instrument_stats: &std::collections::HashMap<u32, u32>,
    instrument_names: &std::collections::HashMap<u32, String>
) {
    info!("\n📈 Statistics (last 30 seconds):");
    info!("   Total ticks received: {}", total_ticks);
    
    if !instrument_stats.is_empty() {
        info!("   Per-instrument breakdown:");
        for (token, count) in instrument_stats.iter() {
            let name = instrument_names.get(token)
                .unwrap_or(&format!("Token_{}", token));
            info!("     {}: {} ticks", name, count);
        }
    }
    
    let rate = total_ticks as f64 / 30.0;
    info!("   Average rate: {:.1} ticks/second\n", rate);
}