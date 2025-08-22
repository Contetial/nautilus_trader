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

//! Live market data validation for Zerodha Kite Connect.
//!
//! This binary validates real-time market data accuracy, consistency,
//! and completeness during live market hours.

use nautilus_zerodha::{
    config::ZerodhaConfig,
    websocket::ZerodhaWebSocketClient,
};
use std::{
    collections::{HashMap, VecDeque},
    env,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::time::{sleep, timeout};
use tracing::{error, info, warn, Level};
use tracing_subscriber;

/// Market data validation configuration
#[derive(Debug)]
struct ValidationConfig {
    /// Instruments to validate
    instruments: Vec<InstrumentInfo>,
    /// Validation duration in seconds
    validation_duration: u64,
    /// Maximum allowed price deviation (percentage)
    max_price_deviation: f64,
    /// Maximum allowed timestamp lag (seconds)
    max_timestamp_lag: u64,
    /// Minimum expected tick rate (ticks/minute)
    min_tick_rate: u32,
    /// Price sanity check ranges
    price_ranges: HashMap<String, (f64, f64)>, // symbol -> (min, max)
}

/// Instrument information for validation
#[derive(Debug, Clone)]
struct InstrumentInfo {
    token: u32,
    symbol: String,
    exchange: String,
    expected_price_range: (f64, f64),
    lot_size: u32,
}

/// Tick data for validation
#[derive(Debug, Clone)]
struct ValidatedTick {
    instrument_token: u32,
    symbol: String,
    timestamp: u64,
    last_price: f64,
    volume: u64,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    received_at: Instant,
}

/// Validation statistics
#[derive(Debug, Default)]
struct ValidationStats {
    /// Total ticks received per instrument
    tick_counts: HashMap<u32, u64>,
    /// Price validation results
    price_violations: HashMap<u32, u64>,
    /// Timestamp validation results
    timestamp_violations: HashMap<u32, u64>,
    /// Missing tick alerts
    missing_ticks: HashMap<u32, u64>,
    /// Data quality scores (0-100)
    quality_scores: HashMap<u32, f64>,
    /// Overall validation status
    total_ticks: u64,
    total_violations: u64,
}

/// Market data validator
struct MarketDataValidator {
    config: ValidationConfig,
    stats: ValidationStats,
    /// Recent tick history for analysis
    tick_history: HashMap<u32, VecDeque<ValidatedTick>>,
    /// Last tick received per instrument
    last_ticks: HashMap<u32, ValidatedTick>,
    /// Validation start time
    start_time: Instant,
}

impl MarketDataValidator {
    fn new(config: ValidationConfig) -> Self {
        Self {
            config,
            stats: ValidationStats::default(),
            tick_history: HashMap::new(),
            last_ticks: HashMap::new(),
            start_time: Instant::now(),
        }
    }
    
    /// Validate incoming tick data
    fn validate_tick(&mut self, tick: ValidatedTick) -> ValidationResult {
        let mut result = ValidationResult::new();
        
        // Update stats
        *self.stats.tick_counts.entry(tick.instrument_token).or_insert(0) += 1;
        self.stats.total_ticks += 1;
        
        // Price validation
        if let Some((min_price, max_price)) = self.get_price_range(&tick.symbol) {
            if tick.last_price < min_price || tick.last_price > max_price {
                result.add_violation(ValidationViolation::PriceOutOfRange {
                    symbol: tick.symbol.clone(),
                    price: tick.last_price,
                    range: (min_price, max_price),
                });
                *self.stats.price_violations.entry(tick.instrument_token).or_insert(0) += 1;
            }
        }
        
        // Timestamp validation
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if tick.timestamp > 0 {
            let timestamp_lag = current_time.saturating_sub(tick.timestamp);
            if timestamp_lag > self.config.max_timestamp_lag {
                result.add_violation(ValidationViolation::StaleTimestamp {
                    symbol: tick.symbol.clone(),
                    lag_seconds: timestamp_lag,
                });
                *self.stats.timestamp_violations.entry(tick.instrument_token).or_insert(0) += 1;
            }
        }
        
        // Price consistency validation
        if let Some(last_tick) = self.last_ticks.get(&tick.instrument_token) {
            let price_change_pct = ((tick.last_price - last_tick.last_price) / last_tick.last_price).abs() * 100.0;
            if price_change_pct > self.config.max_price_deviation {
                result.add_violation(ValidationViolation::ExcessivePriceChange {
                    symbol: tick.symbol.clone(),
                    old_price: last_tick.last_price,
                    new_price: tick.last_price,
                    change_pct: price_change_pct,
                });
            }
        }
        
        // Bid-Ask spread validation
        if let (Some(bid), Some(ask)) = (tick.best_bid, tick.best_ask) {
            if bid >= ask {
                result.add_violation(ValidationViolation::InvalidSpread {
                    symbol: tick.symbol.clone(),
                    bid,
                    ask,
                });
            }
            
            // Check if last price is within bid-ask range
            if tick.last_price < bid || tick.last_price > ask {
                result.add_violation(ValidationViolation::PriceOutsideSpread {
                    symbol: tick.symbol.clone(),
                    last_price: tick.last_price,
                    bid,
                    ask,
                });
            }
        }
        
        // Update tick history
        let history = self.tick_history.entry(tick.instrument_token).or_insert_with(VecDeque::new);
        history.push_back(tick.clone());
        
        // Keep only recent history (last 100 ticks)
        if history.len() > 100 {
            history.pop_front();
        }
        
        // Update last tick
        self.last_ticks.insert(tick.instrument_token, tick);
        
        // Update total violations
        self.stats.total_violations += result.violations.len() as u64;
        
        result
    }
    
    /// Check for missing ticks (gaps in data)
    fn check_missing_ticks(&mut self) {
        let now = Instant::now();
        let expected_interval = Duration::from_secs(60 / self.config.min_tick_rate as u64);
        
        for instrument in &self.config.instruments {
            if let Some(last_tick) = self.last_ticks.get(&instrument.token) {
                let time_since_last = now.duration_since(last_tick.received_at);
                if time_since_last > expected_interval * 2 {
                    *self.stats.missing_ticks.entry(instrument.token).or_insert(0) += 1;
                    warn!("⚠️  Missing ticks for {}: {}s since last update", 
                          instrument.symbol, time_since_last.as_secs());
                }
            }
        }
    }
    
    /// Calculate data quality scores
    fn calculate_quality_scores(&mut self) {
        for instrument in &self.config.instruments {
            let token = instrument.token;
            let total_ticks = *self.stats.tick_counts.get(&token).unwrap_or(&0) as f64;
            
            if total_ticks == 0.0 {
                self.stats.quality_scores.insert(token, 0.0);
                continue;
            }
            
            let price_violations = *self.stats.price_violations.get(&token).unwrap_or(&0) as f64;
            let timestamp_violations = *self.stats.timestamp_violations.get(&token).unwrap_or(&0) as f64;
            let missing_ticks = *self.stats.missing_ticks.get(&token).unwrap_or(&0) as f64;
            
            // Calculate quality score (0-100)
            let price_score = ((total_ticks - price_violations) / total_ticks * 100.0).max(0.0);
            let timestamp_score = ((total_ticks - timestamp_violations) / total_ticks * 100.0).max(0.0);
            let continuity_score = 100.0 - (missing_ticks / total_ticks * 100.0).min(100.0);
            
            let overall_score = (price_score + timestamp_score + continuity_score) / 3.0;
            self.stats.quality_scores.insert(token, overall_score);
        }
    }
    
    /// Generate validation report
    fn generate_report(&mut self) -> ValidationReport {
        self.calculate_quality_scores();
        
        let duration = self.start_time.elapsed();
        let overall_quality = if !self.stats.quality_scores.is_empty() {
            self.stats.quality_scores.values().sum::<f64>() / self.stats.quality_scores.len() as f64
        } else {
            0.0
        };
        
        ValidationReport {
            duration,
            total_ticks: self.stats.total_ticks,
            total_violations: self.stats.total_violations,
            overall_quality,
            instrument_reports: self.config.instruments.iter().map(|instrument| {
                InstrumentReport {
                    token: instrument.token,
                    symbol: instrument.symbol.clone(),
                    tick_count: *self.stats.tick_counts.get(&instrument.token).unwrap_or(&0),
                    price_violations: *self.stats.price_violations.get(&instrument.token).unwrap_or(&0),
                    timestamp_violations: *self.stats.timestamp_violations.get(&instrument.token).unwrap_or(&0),
                    missing_ticks: *self.stats.missing_ticks.get(&instrument.token).unwrap_or(&0),
                    quality_score: *self.stats.quality_scores.get(&instrument.token).unwrap_or(&0.0),
                }
            }).collect(),
        }
    }
    
    fn get_price_range(&self, symbol: &str) -> Option<(f64, f64)> {
        self.config.price_ranges.get(symbol).copied()
    }
}

/// Validation result for a single tick
#[derive(Debug)]
struct ValidationResult {
    violations: Vec<ValidationViolation>,
}

impl ValidationResult {
    fn new() -> Self {
        Self {
            violations: Vec::new(),
        }
    }
    
    fn add_violation(&mut self, violation: ValidationViolation) {
        self.violations.push(violation);
    }
    
    fn is_valid(&self) -> bool {
        self.violations.is_empty()
    }
}

/// Types of validation violations
#[derive(Debug)]
enum ValidationViolation {
    PriceOutOfRange {
        symbol: String,
        price: f64,
        range: (f64, f64),
    },
    ExcessivePriceChange {
        symbol: String,
        old_price: f64,
        new_price: f64,
        change_pct: f64,
    },
    StaleTimestamp {
        symbol: String,
        lag_seconds: u64,
    },
    InvalidSpread {
        symbol: String,
        bid: f64,
        ask: f64,
    },
    PriceOutsideSpread {
        symbol: String,
        last_price: f64,
        bid: f64,
        ask: f64,
    },
}

/// Final validation report
#[derive(Debug)]
struct ValidationReport {
    duration: Duration,
    total_ticks: u64,
    total_violations: u64,
    overall_quality: f64,
    instrument_reports: Vec<InstrumentReport>,
}

/// Per-instrument validation report
#[derive(Debug)]
struct InstrumentReport {
    token: u32,
    symbol: String,
    tick_count: u64,
    price_violations: u64,
    timestamp_violations: u64,
    missing_ticks: u64,
    quality_score: f64,
}

impl ValidationReport {
    fn print_summary(&self) {
        info!("📊 MARKET DATA VALIDATION REPORT");
        info!("=" * 60);
        info!("⏱️  Duration: {:.1}s", self.duration.as_secs_f64());
        info!("📈 Total Ticks: {}", self.total_ticks);
        info!("❌ Total Violations: {}", self.total_violations);
        info!("📊 Overall Quality: {:.1}%", self.overall_quality);
        
        let error_rate = if self.total_ticks > 0 {
            (self.total_violations as f64 / self.total_ticks as f64) * 100.0
        } else {
            0.0
        };
        info!("🎯 Error Rate: {:.3}%", error_rate);
        
        info!("\n📋 PER-INSTRUMENT RESULTS:");
        for report in &self.instrument_reports {
            let status = if report.quality_score >= 95.0 {
                "✅ EXCELLENT"
            } else if report.quality_score >= 85.0 {
                "🟢 GOOD"
            } else if report.quality_score >= 70.0 {
                "🟡 FAIR"
            } else {
                "🔴 POOR"
            };
            
            info!("  {} ({}):", report.symbol, report.token);
            info!("    Ticks: {} | Quality: {:.1}% | {}", 
                  report.tick_count, report.quality_score, status);
            info!("    Violations: Price={} Timestamp={} Missing={}", 
                  report.price_violations, report.timestamp_violations, report.missing_ticks);
        }
        
        // Overall assessment
        if self.overall_quality >= 90.0 && error_rate < 1.0 {
            info!("\n🎉 VALIDATION PASSED: Market data quality is excellent!");
        } else if self.overall_quality >= 75.0 && error_rate < 5.0 {
            info!("\n⚠️  VALIDATION WARNING: Market data quality is acceptable but has issues");
        } else {
            info!("\n❌ VALIDATION FAILED: Market data quality is poor!");
        }
    }
}

/// Create default validation configuration
fn create_validation_config() -> ValidationConfig {
    let mut price_ranges = HashMap::new();
    
    // Define reasonable price ranges for major stocks (in INR)
    price_ranges.insert("RELIANCE".to_string(), (2000.0, 3500.0));
    price_ranges.insert("TCS".to_string(), (3000.0, 4500.0));
    price_ranges.insert("HDFCBANK".to_string(), (1200.0, 2000.0));
    price_ranges.insert("INFY".to_string(), (1000.0, 2000.0));
    price_ranges.insert("ICICIBANK".to_string(), (800.0, 1500.0));
    price_ranges.insert("KOTAKBANK".to_string(), (1500.0, 2500.0));
    price_ranges.insert("AXISBANK".to_string(), (600.0, 1200.0));
    price_ranges.insert("BHARTIARTL".to_string(), (800.0, 1200.0));
    price_ranges.insert("BAJFINANCE".to_string(), (6000.0, 9000.0));
    price_ranges.insert("LT".to_string(), (2500.0, 4000.0));
    
    // Index ranges
    price_ranges.insert("NIFTY50".to_string(), (15000.0, 30000.0));
    price_ranges.insert("BANKNIFTY".to_string(), (40000.0, 60000.0));
    
    let instruments = vec![
        InstrumentInfo {
            token: 738561,
            symbol: "RELIANCE".to_string(),
            exchange: "NSE".to_string(),
            expected_price_range: (2000.0, 3500.0),
            lot_size: 1,
        },
        InstrumentInfo {
            token: 2953217,
            symbol: "TCS".to_string(),
            exchange: "NSE".to_string(),
            expected_price_range: (3000.0, 4500.0),
            lot_size: 1,
        },
        InstrumentInfo {
            token: 341249,
            symbol: "HDFCBANK".to_string(),
            exchange: "NSE".to_string(),
            expected_price_range: (1200.0, 2000.0),
            lot_size: 1,
        },
        InstrumentInfo {
            token: 408065,
            symbol: "INFY".to_string(),
            exchange: "NSE".to_string(),
            expected_price_range: (1000.0, 2000.0),
            lot_size: 1,
        },
        InstrumentInfo {
            token: 1270529,
            symbol: "ICICIBANK".to_string(),
            exchange: "NSE".to_string(),
            expected_price_range: (800.0, 1500.0),
            lot_size: 1,
        },
    ];
    
    ValidationConfig {
        instruments,
        validation_duration: 300, // 5 minutes
        max_price_deviation: 10.0, // 10% max price change between ticks
        max_timestamp_lag: 30,     // 30 seconds max timestamp lag
        min_tick_rate: 2,          // At least 2 ticks per minute
        price_ranges,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("🔍 Starting Zerodha market data validation");

    // Load configuration from environment
    let api_key = env::var("ZERODHA_API_KEY")
        .expect("ZERODHA_API_KEY environment variable not set");
    let access_token = env::var("ZERODHA_ACCESS_TOKEN")
        .expect("ZERODHA_ACCESS_TOKEN environment variable not set");

    let zerodha_config = ZerodhaConfig::new(api_key, "".to_string(), Some(access_token));
    let validation_config = create_validation_config();
    
    info!("📋 Validation Configuration:");
    info!("  Duration: {}s", validation_config.validation_duration);
    info!("  Instruments: {}", validation_config.instruments.len());
    info!("  Max Price Deviation: {:.1}%", validation_config.max_price_deviation);
    info!("  Max Timestamp Lag: {}s", validation_config.max_timestamp_lag);
    info!("  Min Tick Rate: {} ticks/minute", validation_config.min_tick_rate);

    // Create WebSocket client
    let mut ws_client = ZerodhaWebSocketClient::new(zerodha_config);
    let mut validator = MarketDataValidator::new(validation_config);

    // Connect and subscribe
    info!("🔗 Connecting to Zerodha WebSocket...");
    ws_client.connect().await?;
    
    let tokens: Vec<u32> = validator.config.instruments.iter().map(|i| i.token).collect();
    ws_client.subscribe(&tokens).await?;
    
    info!("✅ Connected and subscribed to {} instruments", tokens.len());
    info!("📊 Starting validation for {}s...", validator.config.validation_duration);

    // Validation loop
    let validation_end = Instant::now() + Duration::from_secs(validator.config.validation_duration);
    let mut last_status_update = Instant::now();
    let mut last_missing_check = Instant::now();

    while Instant::now() < validation_end {
        // Check for incoming data
        match timeout(Duration::from_millis(1000), ws_client.next_message()).await {
            Ok(Ok(message)) => {
                // Parse message as tick (simplified)
                if let Some(tick) = parse_tick_message(&message, &validator.config.instruments) {
                    let result = validator.validate_tick(tick);
                    
                    // Log violations
                    for violation in result.violations {
                        match violation {
                            ValidationViolation::PriceOutOfRange { symbol, price, range } => {
                                warn!("💰 Price violation {}: ₹{:.2} outside range ₹{:.2}-₹{:.2}", 
                                      symbol, price, range.0, range.1);
                            }
                            ValidationViolation::ExcessivePriceChange { symbol, old_price, new_price, change_pct } => {
                                warn!("📈 Excessive price change {}: ₹{:.2} -> ₹{:.2} ({:.1}%)", 
                                      symbol, old_price, new_price, change_pct);
                            }
                            ValidationViolation::StaleTimestamp { symbol, lag_seconds } => {
                                warn!("⏰ Stale timestamp {}: {}s lag", symbol, lag_seconds);
                            }
                            ValidationViolation::InvalidSpread { symbol, bid, ask } => {
                                warn!("📊 Invalid spread {}: bid ₹{:.2} >= ask ₹{:.2}", symbol, bid, ask);
                            }
                            ValidationViolation::PriceOutsideSpread { symbol, last_price, bid, ask } => {
                                warn!("💹 Price outside spread {}: ₹{:.2} not in [₹{:.2}, ₹{:.2}]", 
                                      symbol, last_price, bid, ask);
                            }
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                warn!("⚠️  WebSocket error: {}", e);
            }
            Err(_) => {
                // Timeout - continue
            }
        }

        // Periodic status updates
        if last_status_update.elapsed() >= Duration::from_secs(30) {
            let remaining = (validation_end - Instant::now()).as_secs().max(0);
            info!("📊 Progress: {} ticks, {} violations, {}s remaining", 
                  validator.stats.total_ticks, validator.stats.total_violations, remaining);
            last_status_update = Instant::now();
        }

        // Check for missing ticks
        if last_missing_check.elapsed() >= Duration::from_secs(60) {
            validator.check_missing_ticks();
            last_missing_check = Instant::now();
        }
    }

    info!("⏹️  Validation period completed");

    // Disconnect
    if let Err(e) = ws_client.disconnect().await {
        warn!("⚠️  Error during disconnect: {}", e);
    }

    // Generate and display report
    let report = validator.generate_report();
    report.print_summary();

    // Determine exit code based on validation results
    if report.overall_quality >= 90.0 {
        info!("🎉 MARKET DATA VALIDATION SUCCESSFUL!");
        Ok(())
    } else if report.overall_quality >= 75.0 {
        warn!("⚠️  MARKET DATA VALIDATION COMPLETED WITH WARNINGS");
        Ok(())
    } else {
        error!("💥 MARKET DATA VALIDATION FAILED!");
        Err("Market data quality below acceptable threshold".into())
    }
}

/// Parse WebSocket message as tick data (simplified implementation)
fn parse_tick_message(message: &serde_json::Value, instruments: &[InstrumentInfo]) -> Option<ValidatedTick> {
    // This is a simplified parser - real implementation would match Zerodha's format
    if let (Some(token), Some(price)) = (
        message.get("instrument_token").and_then(|v| v.as_u64()),
        message.get("last_price").and_then(|v| v.as_f64())
    ) {
        let token = token as u32;
        
        // Find symbol for token
        let symbol = instruments.iter()
            .find(|i| i.token == token)
            .map(|i| i.symbol.clone())
            .unwrap_or_else(|| format!("UNKNOWN_{}", token));
        
        Some(ValidatedTick {
            instrument_token: token,
            symbol,
            timestamp: message.get("exchange_timestamp").and_then(|v| v.as_u64()).unwrap_or(0),
            last_price: price,
            volume: message.get("volume").and_then(|v| v.as_u64()).unwrap_or(0),
            best_bid: message.get("depth").and_then(|d| d.get("buy")).and_then(|b| b[0].get("price")).and_then(|p| p.as_f64()),
            best_ask: message.get("depth").and_then(|d| d.get("sell")).and_then(|s| s[0].get("price")).and_then(|p| p.as_f64()),
            received_at: Instant::now(),
        })
    } else {
        None
    }
}