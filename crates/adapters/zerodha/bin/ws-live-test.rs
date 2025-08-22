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

//! Live WebSocket testing for Zerodha Kite Connect.
//!
//! This binary tests real-time market data connectivity and validates
//! data accuracy during live market hours.

use nautilus_zerodha::{
    config::ZerodhaConfig,
    websocket::ZerodhaWebSocketClient,
};
use std::{collections::HashMap, env, time::{Duration, Instant}};
use tokio::time::{sleep, timeout};
use tracing::{error, info, warn, Level};
use tracing_subscriber;

/// Test configuration
struct TestConfig {
    /// Instruments to test (token -> symbol)
    test_instruments: HashMap<u32, String>,
    /// Test duration in seconds
    test_duration: u64,
    /// Expected minimum ticks per instrument
    min_ticks_per_instrument: u32,
    /// Memory usage check interval (seconds)
    memory_check_interval: u64,
}

impl Default for TestConfig {
    fn default() -> Self {
        let mut test_instruments = HashMap::new();
        
        // Popular NSE instruments for testing
        test_instruments.insert(256265, "RELIANCE".to_string());   // Reliance Industries
        test_instruments.insert(779521, "TCS".to_string());        // TCS
        test_instruments.insert(738561, "HDFCBANK".to_string());   // HDFC Bank
        test_instruments.insert(895745, "INFY".to_string());       // Infosys
        test_instruments.insert(884737, "ICICIBANK".to_string());  // ICICI Bank
        test_instruments.insert(969473, "KOTAKBANK".to_string());  // Kotak Bank
        test_instruments.insert(81153, "AXISBANK".to_string());    // Axis Bank
        test_instruments.insert(3365889, "BHARTIARTL".to_string()); // Bharti Airtel
        test_instruments.insert(140033, "BAJFINANCE".to_string()); // Bajaj Finance
        test_instruments.insert(225537, "LT".to_string());         // L&T
        
        // Nifty indices
        test_instruments.insert(256265, "NIFTY50".to_string());    // Nifty 50
        test_instruments.insert(260105, "BANKNIFTY".to_string());  // Bank Nifty
        
        Self {
            test_instruments,
            test_duration: 300, // 5 minutes
            min_ticks_per_instrument: 10,
            memory_check_interval: 30,
        }
    }
}

/// Statistics tracker for testing
#[derive(Debug, Default)]
struct TestStats {
    /// Total ticks received
    total_ticks: u64,
    /// Ticks per instrument
    ticks_per_instrument: HashMap<u32, u64>,
    /// Last update time per instrument
    last_update: HashMap<u32, Instant>,
    /// Connection attempts
    connection_attempts: u32,
    /// Disconnection count
    disconnections: u32,
    /// Error count
    errors: u32,
    /// Start time
    start_time: Instant,
}

impl TestStats {
    fn new() -> Self {
        Self {
            start_time: Instant::now(),
            ..Default::default()
        }
    }
    
    fn record_tick(&mut self, instrument_token: u32) {
        self.total_ticks += 1;
        *self.ticks_per_instrument.entry(instrument_token).or_insert(0) += 1;
        self.last_update.insert(instrument_token, Instant::now());
    }
    
    fn record_connection(&mut self) {
        self.connection_attempts += 1;
        info!("🔗 WebSocket connection attempt #{}", self.connection_attempts);
    }
    
    fn record_disconnection(&mut self) {
        self.disconnections += 1;
        warn!("🔌 WebSocket disconnection #{}", self.disconnections);
    }
    
    fn record_error(&mut self) {
        self.errors += 1;
        error!("❌ Error count: {}", self.errors);
    }
    
    fn print_summary(&self, config: &TestConfig) {
        let elapsed = self.start_time.elapsed();
        let ticks_per_second = self.total_ticks as f64 / elapsed.as_secs_f64();
        
        info!("📊 TEST SUMMARY");
        info!("⏱️  Duration: {:.1}s", elapsed.as_secs_f64());
        info!("📈 Total Ticks: {}", self.total_ticks);
        info!("🚀 Ticks/Second: {:.1}", ticks_per_second);
        info!("🔗 Connections: {}", self.connection_attempts);
        info!("🔌 Disconnections: {}", self.disconnections);
        info!("❌ Errors: {}", self.errors);
        
        info!("📊 PER-INSTRUMENT STATS:");
        for (&token, symbol) in &config.test_instruments {
            let ticks = self.ticks_per_instrument.get(&token).unwrap_or(&0);
            let status = if *ticks >= config.min_ticks_per_instrument as u64 {
                "✅ PASS"
            } else {
                "❌ FAIL"
            };
            
            if let Some(last_update) = self.last_update.get(&token) {
                let age = last_update.elapsed().as_secs();
                info!("  {} {}: {} ticks (last: {}s ago) {}", 
                      token, symbol, ticks, age, status);
            } else {
                info!("  {} {}: {} ticks (no updates) {}", 
                      token, symbol, ticks, status);
            }
        }
    }
    
    fn validate_results(&self, config: &TestConfig) -> bool {
        let mut success = true;
        
        // Check minimum tick count per instrument
        for (&token, symbol) in &config.test_instruments {
            let ticks = self.ticks_per_instrument.get(&token).unwrap_or(&0);
            if *ticks < config.min_ticks_per_instrument as u64 {
                error!("❌ Instrument {} ({}) received only {} ticks, expected {}",
                       token, symbol, ticks, config.min_ticks_per_instrument);
                success = false;
            }
        }
        
        // Check overall tick rate
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let ticks_per_second = self.total_ticks as f64 / elapsed;
        if ticks_per_second < 1.0 {
            error!("❌ Low tick rate: {:.2} ticks/second", ticks_per_second);
            success = false;
        }
        
        // Check error rate
        let error_rate = self.errors as f64 / self.total_ticks as f64;
        if error_rate > 0.01 { // More than 1% error rate
            error!("❌ High error rate: {:.1}%", error_rate * 100.0);
            success = false;
        }
        
        // Check disconnection rate
        if self.disconnections > 3 {
            error!("❌ Too many disconnections: {}", self.disconnections);
            success = false;
        }
        
        success
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("🚀 Starting Zerodha WebSocket live data test");

    // Load configuration from environment
    let api_key = env::var("ZERODHA_API_KEY")
        .expect("ZERODHA_API_KEY environment variable not set");
    let access_token = env::var("ZERODHA_ACCESS_TOKEN")
        .expect("ZERODHA_ACCESS_TOKEN environment variable not set");

    let config = ZerodhaConfig::new(api_key, "".to_string(), Some(access_token));
    let test_config = TestConfig::default();
    let mut stats = TestStats::new();

    info!("📋 Test Configuration:");
    info!("  Duration: {}s", test_config.test_duration);
    info!("  Instruments: {}", test_config.test_instruments.len());
    info!("  Min ticks per instrument: {}", test_config.min_ticks_per_instrument);

    // Create WebSocket client
    let mut ws_client = ZerodhaWebSocketClient::new(config);
    
    // Test: Connection and subscription
    info!("🔗 Testing WebSocket connection...");
    stats.record_connection();
    
    match ws_client.connect().await {
        Ok(_) => info!("✅ WebSocket connected successfully"),
        Err(e) => {
            error!("❌ Failed to connect: {}", e);
            stats.record_error();
            return Err(e.into());
        }
    }

    // Subscribe to test instruments
    info!("📺 Subscribing to {} test instruments...", test_config.test_instruments.len());
    let tokens: Vec<u32> = test_config.test_instruments.keys().cloned().collect();
    
    match ws_client.subscribe(&tokens).await {
        Ok(_) => info!("✅ Subscribed to all instruments"),
        Err(e) => {
            error!("❌ Failed to subscribe: {}", e);
            stats.record_error();
            return Err(e.into());
        }
    }

    // Test: Data reception and validation
    info!("📊 Starting live data collection for {}s...", test_config.test_duration);
    let test_end = Instant::now() + Duration::from_secs(test_config.test_duration);
    let mut last_memory_check = Instant::now();
    let mut last_stats_print = Instant::now();

    while Instant::now() < test_end {
        // Check for incoming data
        match timeout(Duration::from_millis(1000), ws_client.next_message()).await {
            Ok(Ok(message)) => {
                if let Some(tick) = message.as_tick() {
                    stats.record_tick(tick.instrument_token);
                    
                    // Validate tick data
                    if tick.last_price <= 0.0 {
                        warn!("⚠️  Invalid tick price for {}: {}", 
                              tick.instrument_token, tick.last_price);
                        stats.record_error();
                    }
                }
            }
            Ok(Err(e)) => {
                warn!("⚠️  WebSocket error: {}", e);
                stats.record_error();
            }
            Err(_) => {
                // Timeout - check if we should reconnect
                if ws_client.is_connected().await {
                    // Still connected, just no data
                    continue;
                } else {
                    // Disconnected, attempt reconnection
                    warn!("🔌 WebSocket disconnected, attempting reconnection...");
                    stats.record_disconnection();
                    
                    match ws_client.reconnect().await {
                        Ok(_) => {
                            info!("✅ Reconnected successfully");
                            stats.record_connection();
                            
                            // Re-subscribe
                            if let Err(e) = ws_client.subscribe(&tokens).await {
                                error!("❌ Failed to re-subscribe: {}", e);
                                stats.record_error();
                            }
                        }
                        Err(e) => {
                            error!("❌ Reconnection failed: {}", e);
                            stats.record_error();
                        }
                    }
                }
            }
        }

        // Periodic status updates
        if last_stats_print.elapsed() >= Duration::from_secs(30) {
            info!("📊 Progress: {} ticks received, {:.1}s remaining", 
                  stats.total_ticks, 
                  (test_end - Instant::now()).as_secs_f64().max(0.0));
            last_stats_print = Instant::now();
        }

        // Memory usage check
        if last_memory_check.elapsed() >= Duration::from_secs(test_config.memory_check_interval) {
            // Basic memory usage info (platform-specific implementation would be better)
            info!("💾 Memory check - Active connections: {}", 
                  if ws_client.is_connected().await { 1 } else { 0 });
            last_memory_check = Instant::now();
        }
    }

    info!("⏹️  Test duration completed, analyzing results...");

    // Disconnect and cleanup
    if let Err(e) = ws_client.disconnect().await {
        warn!("⚠️  Error during disconnect: {}", e);
    }

    // Print comprehensive summary
    stats.print_summary(&test_config);

    // Validate results
    let success = stats.validate_results(&test_config);
    
    if success {
        info!("🎉 ALL TESTS PASSED! WebSocket live data validation successful.");
    } else {
        error!("💥 TESTS FAILED! Issues detected in WebSocket data reception.");
        return Err("Test validation failed".into());
    }

    // Additional validation recommendations
    info!("📝 VALIDATION RECOMMENDATIONS:");
    info!("  1. Compare tick prices with Zerodha web interface");
    info!("  2. Verify timestamp accuracy (should be within 1-2 seconds)");
    info!("  3. Check for missing ticks during high-volume periods");
    info!("  4. Monitor latency during different market conditions");
    info!("  5. Test during market open/close for volatility handling");

    Ok(())
}

/// Helper trait for message processing
trait MessageExt {
    fn as_tick(&self) -> Option<TickData>;
}

/// Simplified tick data structure for testing
#[derive(Debug)]
struct TickData {
    instrument_token: u32,
    last_price: f64,
    timestamp: Instant,
}

impl MessageExt for serde_json::Value {
    fn as_tick(&self) -> Option<TickData> {
        // Parse WebSocket message as tick data
        // This would need to match the actual Zerodha WebSocket format
        if let (Some(token), Some(price)) = (
            self.get("instrument_token").and_then(|v| v.as_u64()),
            self.get("last_price").and_then(|v| v.as_f64())
        ) {
            Some(TickData {
                instrument_token: token as u32,
                last_price: price,
                timestamp: Instant::now(),
            })
        } else {
            None
        }
    }
}