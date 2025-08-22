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

//! Test binary for fetching instruments from Zerodha API.
//!
//! This binary demonstrates basic connectivity to the Zerodha Kite Connect API
//! and fetches the complete list of tradable instruments.
//!
//! Usage:
//! ```bash
//! export ZERODHA_API_KEY="your_api_key"
//! export ZERODHA_API_SECRET="your_api_secret"  
//! export ZERODHA_ACCESS_TOKEN="your_access_token"
//! 
//! cargo run --bin zerodha-http-instruments
//! ```

use nautilus_zerodha::{ZerodhaConfig, ZerodhaHttpClient};
use std::env;
use tracing::{error, info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting Zerodha instruments test");

    // Get configuration from environment variables
    let api_key = env::var("ZERODHA_API_KEY")
        .map_err(|_| "ZERODHA_API_KEY environment variable not set")?;
    
    let api_secret = env::var("ZERODHA_API_SECRET")
        .map_err(|_| "ZERODHA_API_SECRET environment variable not set")?;
    
    let access_token = env::var("ZERODHA_ACCESS_TOKEN")
        .map_err(|_| "ZERODHA_ACCESS_TOKEN environment variable not set")?;

    // Create configuration
    let config = ZerodhaConfig::new(api_key, api_secret, Some(access_token))
        .with_debug(true); // Enable debug logging

    // Create HTTP client
    let client = ZerodhaHttpClient::new(config)?;

    // Test basic connectivity
    info!("Testing connectivity by fetching instruments...");
    
    match client.get_instruments().await {
        Ok(instruments) => {
            info!("✅ Successfully fetched {} instruments", instruments.len());
            
            // Show some example instruments
            info!("📊 Sample instruments:");
            
            // Show first few equity instruments
            let equities: Vec<_> = instruments.iter()
                .filter(|i| i.exchange == nautilus_zerodha::enums::Exchange::NSE 
                       && i.instrument_type == nautilus_zerodha::enums::InstrumentType::EQ)
                .take(5)
                .collect();
                
            for instrument in equities {
                info!("  EQ: {} ({})", instrument.tradingsymbol, instrument.name);
            }
            
            // Show options instruments (if any)
            let options: Vec<_> = instruments.iter()
                .filter(|i| matches!(i.instrument_type, 
                                   nautilus_zerodha::enums::InstrumentType::CE | 
                                   nautilus_zerodha::enums::InstrumentType::PE))
                .take(5)
                .collect();
                
            if !options.is_empty() {
                info!("🎯 Sample options:");
                for option in options {
                    info!("  {}: {} strike={:?} expiry={:?}", 
                          option.instrument_type,
                          option.tradingsymbol, 
                          option.strike,
                          option.expiry);
                }
            }
            
            // Show instrument type breakdown
            let mut type_counts = std::collections::HashMap::new();
            for instrument in &instruments {
                *type_counts.entry(instrument.instrument_type).or_insert(0) += 1;
            }
            
            info!("📈 Instrument type breakdown:");
            for (inst_type, count) in type_counts {
                info!("  {}: {} instruments", inst_type, count);
            }
        }
        Err(e) => {
            error!("❌ Failed to fetch instruments: {}", e);
            
            match &e {
                nautilus_zerodha::ZerodhaError::Authentication { .. } => {
                    error!("💡 Check your API credentials:");
                    error!("   - API Key: correct?");
                    error!("   - Access Token: valid and not expired?");
                    error!("   - Try regenerating access token if needed");
                }
                nautilus_zerodha::ZerodhaError::RateLimit => {
                    error!("💡 Rate limit exceeded. Wait and retry.");
                }
                _ => {
                    error!("💡 Check your internet connection and API status");
                }
            }
            
            return Err(e.into());
        }
    }

    // Test exchange-specific instruments
    info!("\n🏛️  Testing exchange-specific fetching...");
    
    match client.get_instruments_for_exchange(nautilus_zerodha::enums::Exchange::NFO).await {
        Ok(nfo_instruments) => {
            info!("✅ NFO (F&O) instruments: {}", nfo_instruments.len());
            
            // Show some Nifty options
            let nifty_options: Vec<_> = nfo_instruments.iter()
                .filter(|i| i.tradingsymbol.contains("NIFTY") 
                       && matches!(i.instrument_type, 
                                 nautilus_zerodha::enums::InstrumentType::CE | 
                                 nautilus_zerodha::enums::InstrumentType::PE))
                .take(3)
                .collect();
                
            if !nifty_options.is_empty() {
                info!("🎯 Sample Nifty options:");
                for option in nifty_options {
                    info!("  {}: strike={:?} expiry={:?}", 
                          option.tradingsymbol,
                          option.strike,
                          option.expiry);
                }
            }
        }
        Err(e) => {
            error!("❌ Failed to fetch NFO instruments: {}", e);
        }
    }

    info!("\n✅ Zerodha instruments test completed");
    Ok(())
}

/// Helper function to display environment variable setup instructions
#[allow(dead_code)]
fn show_setup_instructions() {
    println!("\n🔧 Setup Instructions:");
    println!("1. Get Zerodha Kite Connect API credentials from: https://developers.kite.trade/");
    println!("2. Set environment variables:");
    println!("   export ZERODHA_API_KEY=\"your_api_key\"");
    println!("   export ZERODHA_API_SECRET=\"your_api_secret\"");
    println!("   export ZERODHA_ACCESS_TOKEN=\"your_access_token\"");
    println!("\n3. Generate access token:");
    println!("   - Use the login URL: https://kite.trade/connect/login?api_key=YOUR_API_KEY&v=3");
    println!("   - After login, extract request_token from redirect URL");
    println!("   - Use request_token with API secret to generate access_token");
    println!("\n📚 Documentation: https://kite.trade/docs/connect/v3/");
}