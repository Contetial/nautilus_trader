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

//! Configuration system test for Zerodha adapter.
//!
//! This binary demonstrates the new configuration file system and helps
//! users set up their credentials securely.

use nautilus_zerodha::{
    config::credentials::{ZerodhaCredentialsConfig, ZerodhaCredentialsBuilder},
    config::{ZerodhaExecutionConfig, ZerodhaHttpConfig},
    execution::ZerodhaExecutionClient,
    http::ZerodhaHttpClient,
};
use std::{env, path::PathBuf};
use tracing::{info, warn, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("🔧 Zerodha Configuration System Test");
    info!("=====================================");

    // Show configuration file locations
    info!("📁 Configuration file search paths:");
    let config_paths = get_config_search_paths();
    for (i, path) in config_paths.iter().enumerate() {
        let status = if path.exists() { "✅ EXISTS" } else { "❌ Not found" };
        info!("  {}. {} - {}", i + 1, path.display(), status);
    }

    // Try to load configuration
    info!("\n🔍 Attempting to load configuration...");
    
    match ZerodhaCredentialsConfig::load() {
        Ok(config) => {
            info!("✅ Configuration loaded successfully!");
            info!("📊 Configuration summary:");
            info!("  {}", config.display_safe());
            
            // Check if access token is missing
            if config.api.access_token.is_empty() || config.api.access_token == "your_access_token_here" {
                warn!("⚠️  No valid access token found in configuration");
                info!("💡 To generate an access token, run:");
                info!("   cargo run --bin zerodha-token-generator");
                info!("");
            }
            
            // Test the configuration
            test_configuration(config).await?;
        }
        Err(e) => {
            warn!("⚠️  Failed to load configuration: {}", e);
            info!("💡 Let's create an example configuration file...");
            
            create_example_config().await?;
        }
    }

    Ok(())
}

async fn test_configuration(config: ZerodhaCredentialsConfig) -> Result<(), Box<dyn std::error::Error>> {
    info!("\n🧪 Testing configuration...");
    
    // Create HTTP client configuration
    let http_config = ZerodhaHttpConfig {
        base_url: "https://api.kite.trade".to_string(),
        timeout_seconds: 30,
        rate_limit_per_second: 3,
        max_retries: 3,
        retry_delay_ms: 1000,
    };

    // Create execution configuration
    let exec_config = if config.is_paper_trading() {
        info!("📋 Using paper trading configuration");
        ZerodhaExecutionConfig {
            max_order_quantity: 1000,
            enable_validation: config.risk_management.enable_validation,
            max_position_value: config.risk_management.max_position_value,
            enable_margin_check: true,
            default_validity: crate::enums::Validity::Day,
            default_product_type: crate::enums::ProductType::MIS,
            paper_trading: true,
            paper_balance: config.paper_trading.initial_balance,
            paper_commission: config.paper_trading.commission_per_trade,
            simulate_latency: true,
            execution_delay_ms: config.paper_trading.execution_delay_ms,
        }
    } else {
        info!("🔴 Using live trading configuration");
        ZerodhaExecutionConfig {
            max_order_quantity: 1000,
            enable_validation: config.risk_management.enable_validation,
            max_position_value: config.risk_management.max_position_value,
            enable_margin_check: true,
            default_validity: crate::enums::Validity::Day,
            default_product_type: crate::enums::ProductType::MIS,
            paper_trading: false,
            paper_balance: 0.0,
            paper_commission: 0.0,
            simulate_latency: false,
            execution_delay_ms: 0,
        }
    };

    // Create HTTP client
    let http_client = ZerodhaHttpClient::new(http_config);

    // Create execution client
    let exec_client = ZerodhaExecutionClient::new(exec_config, http_client);

    info!("✅ Execution client created successfully");
    info!("📊 Paper trading mode: {}", exec_client.is_paper_trading());

    // Test basic functionality based on mode
    if config.is_paper_trading() {
        info!("🎮 Testing paper trading functionality...");
        
        // Update some market prices for paper trading
        exec_client.update_paper_market_price("RELIANCE", 2800.0).await?;
        exec_client.update_paper_market_price("TCS", 3500.0).await?;
        
        info!("📈 Market prices updated for paper trading");
        
        // Get paper trading stats
        if let Some(stats) = exec_client.get_paper_trading_stats().await? {
            info!("💰 Paper trading balance: ₹{:.2}", stats.current_balance);
            info!("💳 Available margin: ₹{:.2}", stats.available_margin);
        }
        
        info!("✅ Paper trading test completed successfully");
    } else {
        warn!("🔴 Live trading mode detected!");
        warn!("⚠️  For safety, we'll only test account information retrieval");
        
        // Test account information (safe for live mode)
        match exec_client.get_account_info().await {
            Ok((balance, margin)) => {
                info!("💰 Account Balance: ₹{:.2}", balance);
                info!("💳 Available Margin: ₹{:.2}", margin);
                info!("✅ Live account information retrieved successfully");
            }
            Err(e) => {
                warn!("❌ Failed to retrieve account information: {}", e);
                warn!("💡 Check your API credentials and access token");
            }
        }
    }

    // Test configuration validation
    info!("\n🔍 Configuration validation:");
    match config.validate() {
        Ok(_) => info!("✅ Configuration is valid"),
        Err(e) => warn!("❌ Configuration validation failed: {}", e),
    }

    // Display configuration summary
    info!("\n📋 Configuration Summary:");
    info!("  Trading Mode: {}", config.settings.trading_mode);
    info!("  Sandbox Mode: {}", config.settings.sandbox);
    info!("  Default Product: {}", config.settings.default_product_type);
    info!("  Default Validity: {}", config.settings.default_validity);
    info!("  Max Order Value: ₹{:.2}", config.risk_management.max_order_value);
    info!("  Max Orders/Min: {}", config.risk_management.max_orders_per_minute);
    info!("  Paper Balance: ₹{:.2}", config.paper_trading.initial_balance);
    info!("  Test Duration: {}s", config.testing.test_duration);
    info!("  Test Instruments: {:?}", config.testing.test_instruments);

    Ok(())
}

async fn create_example_config() -> Result<(), Box<dyn std::error::Error>> {
    info!("📝 Creating example configuration file...");
    
    // Create example configuration using builder
    let example_config = ZerodhaCredentialsBuilder::new()
        .with_api_credentials(
            "your_api_key_here".to_string(),
            "your_api_secret_here".to_string(),
            Some("your_access_token_here".to_string())
        )
        .with_trading_mode("paper")
        .with_sandbox(true)
        .with_paper_balance(1_000_000.0)
        .with_max_order_value(100_000.0)
        .build()?;

    // Save to current directory
    let config_path = PathBuf::from("zerodha_credentials.toml");
    example_config.save_to_file(&config_path)?;
    
    info!("✅ Example configuration file created: {}", config_path.display());
    info!("");
    info!("📋 Next steps:");
    info!("  1. Edit the file: {}", config_path.display());
    info!("  2. Replace 'your_api_key_here' with your actual Kite Connect API key");
    info!("  3. Replace 'your_api_secret_here' with your actual API secret");
    info!("  4. Generate access token: cargo run --bin zerodha-token-generator");
    info!("  5. Adjust other settings as needed");
    info!("  6. Run this test again to validate your configuration");
    info!("");
    info!("💡 Tip: Start with paper trading mode for safe testing!");
    info!("💡 Tip: Add 'zerodha_credentials.toml' to your .gitignore file");

    Ok(())
}

fn get_config_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    
    // Current directory
    paths.push(PathBuf::from("zerodha_credentials.toml"));
    paths.push(PathBuf::from("config/zerodha_credentials.toml"));
    
    // Project root
    paths.push(PathBuf::from("../../../config/zerodha_credentials.toml"));
    
    // Home directory
    if let Some(home_dir) = env::var("HOME").ok().or_else(|| env::var("USERPROFILE").ok()) {
        paths.push(PathBuf::from(home_dir).join(".zerodha_credentials.toml"));
        paths.push(PathBuf::from(home_dir).join(".config").join("zerodha_credentials.toml"));
    }
    
    paths
}