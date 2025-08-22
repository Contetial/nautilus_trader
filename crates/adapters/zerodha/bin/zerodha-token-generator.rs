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

//! Zerodha Access Token Generator
//!
//! Interactive utility for generating Kite Connect access tokens.
//! This binary guides users through the complete authentication flow.

use nautilus_zerodha::auth::AccessTokenGenerator;
use nautilus_zerodha::config::credentials::ZerodhaCredentialsConfig;
use std::env;
use std::io::{self, Write};
use tracing::{error, info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    info!("🚀 Zerodha Access Token Generator");
    info!("================================");

    // Try to load existing configuration first
    let config_result = ZerodhaCredentialsConfig::load();
    
    let (api_key, api_secret) = match config_result {
        Ok(config) => {
            info!("✅ Found existing configuration");
            
            if !config.api.access_token.is_empty() && config.api.access_token != "your_access_token_here" {
                println!("\n⚠️  You already have an access token in your configuration:");
                println!("   Token: {}***", &config.api.access_token[..8.min(config.api.access_token.len())]);
                println!("\nDo you want to generate a new token? (y/N): ");
                
                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();
                
                if !input.trim().to_lowercase().starts_with('y') {
                    info!("✅ Keeping existing token. Exiting...");
                    return;
                }
            }
            
            (config.api.api_key, config.api.api_secret)
        }
        Err(_) => {
            info!("📝 No existing configuration found");
            get_credentials_from_user().unwrap_or_else(|e| {
                error!("❌ Failed to get credentials: {}", e);
                std::process::exit(1);
            })
        }
    };

    // Validate credentials
    if api_key.is_empty() || api_secret.is_empty() {
        error!("❌ API key and secret are required");
        println!("\n💡 Get your API credentials from:");
        println!("   https://kite.trade/connect/");
        std::process::exit(1);
    }

    // Create token generator
    let mut generator = AccessTokenGenerator::new(api_key.clone(), api_secret.clone());

    // Check for command line arguments
    let args: Vec<String> = env::args().collect();
    
    let result = if args.len() > 1 && args[1] == "--request-token" {
        // Direct mode with request token from command line
        if args.len() < 3 {
            error!("❌ Request token required when using --request-token flag");
            println!("Usage: {} --request-token YOUR_REQUEST_TOKEN", args[0]);
            std::process::exit(1);
        }
        
        let request_token = &args[2];
        generator.generate_token_with_request_token(request_token).await
    } else {
        // Interactive mode
        generator.generate_token_interactive().await
    };

    match result {
        Ok(session_token) => {
            println!("\n🎉 SUCCESS! Access token generated successfully.");
            println!("\n📋 Account Information:");
            println!("   User: {} ({})", session_token.user_name, session_token.user_id);
            println!("   Email: {}", session_token.email);
            println!("   Broker: {}", session_token.broker);
            println!("   Exchanges: {}", session_token.exchanges.join(", "));
            println!("   Products: {}", session_token.products.join(", "));

            // Save instructions
            generator.save_to_config(None).unwrap_or_else(|e| {
                error!("⚠️  Failed to display save instructions: {}", e);
            });

            println!("\n🧪 Next Steps:");
            println!("   1. Save the access token to your configuration file");
            println!("   2. Test your setup: cargo run --bin zerodha-config-test");
            println!("   3. Run integration tests: cargo run --bin zerodha-execution-test");
        }
        Err(e) => {
            error!("❌ Failed to generate access token: {}", e);
            println!("\n🔍 Troubleshooting:");
            println!("   1. Verify your API key and secret are correct");
            println!("   2. Make sure you copied the complete request_token");
            println!("   3. Check that your Kite Connect app is active");
            println!("   4. Ensure you completed the login flow correctly");
            println!("\n💡 Get help at: https://kite.trade/docs/connect/v3/");
            std::process::exit(1);
        }
    }
}

/// Get API credentials from user input
fn get_credentials_from_user() -> Result<(String, String), Box<dyn std::error::Error>> {
    println!("\n🔐 API Credentials Required");
    println!("===========================");
    println!("\nYou need API credentials from Kite Connect:");
    println!("1. Go to https://kite.trade/connect/");
    println!("2. Create a new app or use existing one");
    println!("3. Copy your API Key and API Secret");
    println!();

    // Get API key
    print!("Enter API Key: ");
    io::stdout().flush()?;
    let mut api_key = String::new();
    io::stdin().read_line(&mut api_key)?;
    let api_key = api_key.trim().to_string();

    // Get API secret
    print!("Enter API Secret: ");
    io::stdout().flush()?;
    let mut api_secret = String::new();
    io::stdin().read_line(&mut api_secret)?;
    let api_secret = api_secret.trim().to_string();

    Ok((api_key, api_secret))
}