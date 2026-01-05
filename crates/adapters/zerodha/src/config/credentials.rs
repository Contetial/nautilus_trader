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

//! Zerodha credentials configuration management.
//!
//! This module provides secure loading and management of Zerodha API credentials
//! from configuration files with fallback to environment variables.

use crate::error::{ZerodhaError, ZerodhaResult};
use serde::{Deserialize, Serialize};
use std::{
    env,
    fs,
    path::{Path, PathBuf},
};
use tracing::{debug, info, warn};

/// Complete Zerodha configuration including credentials and settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZerodhaCredentialsConfig {
    /// API credentials
    pub api: ApiCredentials,
    /// General settings
    pub settings: GeneralSettings,
    /// Paper trading configuration
    pub paper_trading: PaperTradingConfig,
    /// Risk management settings
    pub risk_management: RiskManagementConfig,
    /// Logging configuration
    pub logging: LoggingConfig,
    /// Testing configuration
    pub testing: TestingConfig,
}

/// API credentials section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCredentials {
    /// Kite Connect API key
    pub api_key: String,
    /// Kite Connect API secret
    pub api_secret: String,
    /// Access token (optional, can be generated)
    pub access_token: Option<String>,
}

/// General settings section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralSettings {
    /// Trading mode: "live" or "paper"
    pub trading_mode: String,
    /// Sandbox mode
    pub sandbox: bool,
    /// Default product type
    pub default_product_type: String,
    /// Default validity
    pub default_validity: String,
}

/// Paper trading configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperTradingConfig {
    /// Initial balance in INR
    pub initial_balance: f64,
    /// Commission per trade in INR
    pub commission_per_trade: f64,
    /// Execution delay in milliseconds
    pub execution_delay_ms: u64,
    /// Enable market data simulation
    pub simulate_market_data: bool,
}

/// Risk management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskManagementConfig {
    /// Maximum order value in INR
    pub max_order_value: f64,
    /// Maximum position value in INR
    pub max_position_value: f64,
    /// Maximum orders per minute
    pub max_orders_per_minute: u32,
    /// Enable pre-trade validation
    pub enable_validation: bool,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    pub log_level: String,
    /// Enable file logging
    pub log_to_file: bool,
    /// Log file path
    pub log_file: String,
}

/// Testing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestingConfig {
    /// Test duration in seconds
    pub test_duration: u64,
    /// Test instruments
    pub test_instruments: Vec<String>,
    /// Minimum data quality percentage
    pub min_data_quality: f64,
    /// Maximum error rate percentage
    pub max_error_rate: f64,
}

impl Default for ZerodhaCredentialsConfig {
    fn default() -> Self {
        Self {
            api: ApiCredentials {
                api_key: String::new(),
                api_secret: String::new(),
                access_token: None,
            },
            settings: GeneralSettings {
                trading_mode: "paper".to_string(),
                sandbox: true,
                default_product_type: "MIS".to_string(),
                default_validity: "DAY".to_string(),
            },
            paper_trading: PaperTradingConfig {
                initial_balance: 1_000_000.0,
                commission_per_trade: 20.0,
                execution_delay_ms: 100,
                simulate_market_data: true,
            },
            risk_management: RiskManagementConfig {
                max_order_value: 100_000.0,
                max_position_value: 1_000_000.0,
                max_orders_per_minute: 10,
                enable_validation: true,
            },
            logging: LoggingConfig {
                log_level: "INFO".to_string(),
                log_to_file: true,
                log_file: "logs/zerodha_adapter.log".to_string(),
            },
            testing: TestingConfig {
                test_duration: 300,
                test_instruments: vec![
                    "RELIANCE".to_string(),
                    "TCS".to_string(),
                    "HDFCBANK".to_string(),
                    "INFY".to_string(),
                    "ICICIBANK".to_string(),
                ],
                min_data_quality: 85.0,
                max_error_rate: 5.0,
            },
        }
    }
}

impl ZerodhaCredentialsConfig {
    /// Load configuration from file with fallbacks
    pub fn load() -> ZerodhaResult<Self> {
        // Try to load from various possible locations
        let config_paths = Self::get_config_paths();
        
        for path in &config_paths {
            if path.exists() {
                info!("📁 Loading Zerodha config from: {}", path.display());
                return Self::load_from_file(path);
            }
        }
        
        // If no config file found, try environment variables
        warn!("📁 No config file found, attempting to load from environment variables");
        Self::load_from_env()
    }
    
    /// Load configuration from specific file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> ZerodhaResult<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .map_err(|e| ZerodhaError::config_error(&format!("Failed to read config file {}: {}", path.display(), e)))?;
        
        let mut config: Self = toml::from_str(&content)
            .map_err(|e| ZerodhaError::config_error(&format!("Failed to parse config file {}: {}", path.display(), e)))?;
        
        // Override with environment variables if they exist
        config.apply_env_overrides();
        
        // Validate configuration
        config.validate()?;
        
        info!("✅ Configuration loaded successfully from {}", path.display());
        Ok(config)
    }
    
    /// Load configuration from environment variables
    pub fn load_from_env() -> ZerodhaResult<Self> {
        let mut config = Self::default();
        
        // Load API credentials from environment
        config.api.api_key = env::var("ZERODHA_API_KEY")
            .map_err(|_| ZerodhaError::config_error("ZERODHA_API_KEY environment variable not set"))?;
        
        config.api.api_secret = env::var("ZERODHA_API_SECRET")
            .map_err(|_| ZerodhaError::config_error("ZERODHA_API_SECRET environment variable not set"))?;
        
        config.api.access_token = env::var("ZERODHA_ACCESS_TOKEN").ok();
        
        // Optional overrides
        if let Ok(trading_mode) = env::var("ZERODHA_TRADING_MODE") {
            config.settings.trading_mode = trading_mode;
        }
        
        if let Ok(sandbox) = env::var("ZERODHA_SANDBOX") {
            config.settings.sandbox = sandbox.to_lowercase() == "true";
        }
        
        config.validate()?;
        
        info!("✅ Configuration loaded from environment variables");
        Ok(config)
    }
    
    /// Apply environment variable overrides to loaded config
    fn apply_env_overrides(&mut self) {
        // API credentials overrides
        if let Ok(api_key) = env::var("ZERODHA_API_KEY") {
            debug!("🔧 Overriding API key from environment");
            self.api.api_key = api_key;
        }
        
        if let Ok(api_secret) = env::var("ZERODHA_API_SECRET") {
            debug!("🔧 Overriding API secret from environment");
            self.api.api_secret = api_secret;
        }
        
        if let Ok(access_token) = env::var("ZERODHA_ACCESS_TOKEN") {
            debug!("🔧 Overriding access token from environment");
            self.api.access_token = Some(access_token);
        }
        
        // Settings overrides
        if let Ok(trading_mode) = env::var("ZERODHA_TRADING_MODE") {
            debug!("🔧 Overriding trading mode from environment: {}", trading_mode);
            self.settings.trading_mode = trading_mode;
        }
        
        if let Ok(sandbox) = env::var("ZERODHA_SANDBOX") {
            let sandbox_bool = sandbox.to_lowercase() == "true";
            debug!("🔧 Overriding sandbox mode from environment: {}", sandbox_bool);
            self.settings.sandbox = sandbox_bool;
        }
        
        // Logging overrides
        if let Ok(log_level) = env::var("RUST_LOG") {
            debug!("🔧 Overriding log level from environment: {}", log_level);
            self.logging.log_level = log_level;
        }
    }
    
    /// Validate configuration
    pub fn validate(&self) -> ZerodhaResult<()> {
        // Validate API credentials
        if self.api.api_key.is_empty() {
            return Err(ZerodhaError::config_error("API key cannot be empty"));
        }
        
        if self.api.api_secret.is_empty() {
            return Err(ZerodhaError::config_error("API secret cannot be empty"));
        }
        
        // Validate trading mode
        if !["live", "paper"].contains(&self.settings.trading_mode.as_str()) {
            return Err(ZerodhaError::config_error("Trading mode must be 'live' or 'paper'"));
        }
        
        // Validate paper trading settings
        if self.paper_trading.initial_balance <= 0.0 {
            return Err(ZerodhaError::config_error("Paper trading initial balance must be positive"));
        }
        
        // Validate risk management settings
        if self.risk_management.max_order_value <= 0.0 {
            return Err(ZerodhaError::config_error("Maximum order value must be positive"));
        }
        
        if self.risk_management.max_position_value <= 0.0 {
            return Err(ZerodhaError::config_error("Maximum position value must be positive"));
        }
        
        // Validate testing settings
        if self.testing.min_data_quality < 0.0 || self.testing.min_data_quality > 100.0 {
            return Err(ZerodhaError::config_error("Minimum data quality must be between 0 and 100"));
        }
        
        if self.testing.max_error_rate < 0.0 || self.testing.max_error_rate > 100.0 {
            return Err(ZerodhaError::config_error("Maximum error rate must be between 0 and 100"));
        }
        
        Ok(())
    }
    
    /// Get possible configuration file paths
    fn get_config_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        
        // 1. Current directory
        paths.push(PathBuf::from("zerodha_credentials.toml"));
        paths.push(PathBuf::from("config/zerodha_credentials.toml"));
        
        // 2. Project root (assuming we're in crates/adapters/zerodha)
        paths.push(PathBuf::from("../../../config/zerodha_credentials.toml"));
        paths.push(PathBuf::from("../../../../config/zerodha_credentials.toml"));
        
        // 3. Home directory
        if let Some(home_dir) = env::var("HOME").ok().or_else(|| env::var("USERPROFILE").ok()) {
            paths.push(PathBuf::from(&home_dir).join(".zerodha_credentials.toml"));
            paths.push(PathBuf::from(&home_dir).join(".config").join("zerodha_credentials.toml"));
        }
        
        // 4. System config directories
        paths.push(PathBuf::from("/etc/zerodha/credentials.toml"));
        paths.push(PathBuf::from("C:\\ProgramData\\Zerodha\\credentials.toml"));
        
        paths
    }
    
    /// Save configuration to file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> ZerodhaResult<()> {
        let path = path.as_ref();
        
        // Create directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| ZerodhaError::config_error(&format!("Failed to create config directory: {}", e)))?;
        }
        
        let content = toml::to_string_pretty(self)
            .map_err(|e| ZerodhaError::config_error(&format!("Failed to serialize config: {}", e)))?;
        
        fs::write(path, content)
            .map_err(|e| ZerodhaError::config_error(&format!("Failed to write config file: {}", e)))?;
        
        info!("💾 Configuration saved to {}", path.display());
        Ok(())
    }
    
    /// Create example configuration file
    pub fn create_example_file<P: AsRef<Path>>(path: P) -> ZerodhaResult<()> {
        let example_config = Self::default();
        example_config.save_to_file(path)
    }
    
    /// Check if running in paper trading mode
    pub fn is_paper_trading(&self) -> bool {
        self.settings.trading_mode == "paper"
    }
    
    /// Check if running in sandbox mode
    pub fn is_sandbox(&self) -> bool {
        self.settings.sandbox
    }
    
    /// Get display-safe version of config (hides sensitive data)
    pub fn display_safe(&self) -> String {
        format!(
            "ZerodhaConfig {{ \
                api_key: {}***, \
                trading_mode: {}, \
                sandbox: {}, \
                paper_balance: ₹{:.2}, \
                max_order_value: ₹{:.2} \
            }}",
            &self.api.api_key.chars().take(8).collect::<String>(),
            self.settings.trading_mode,
            self.settings.sandbox,
            self.paper_trading.initial_balance,
            self.risk_management.max_order_value
        )
    }
}

/// Configuration builder for programmatic setup
pub struct ZerodhaCredentialsBuilder {
    config: ZerodhaCredentialsConfig,
}

impl ZerodhaCredentialsBuilder {
    /// Create new builder with defaults
    pub fn new() -> Self {
        Self {
            config: ZerodhaCredentialsConfig::default(),
        }
    }
    
    /// Set API credentials
    pub fn with_api_credentials(mut self, api_key: String, api_secret: String, access_token: Option<String>) -> Self {
        self.config.api.api_key = api_key;
        self.config.api.api_secret = api_secret;
        self.config.api.access_token = access_token;
        self
    }
    
    /// Set trading mode
    pub fn with_trading_mode(mut self, mode: &str) -> Self {
        self.config.settings.trading_mode = mode.to_string();
        self
    }
    
    /// Enable/disable sandbox mode
    pub fn with_sandbox(mut self, sandbox: bool) -> Self {
        self.config.settings.sandbox = sandbox;
        self
    }
    
    /// Set paper trading balance
    pub fn with_paper_balance(mut self, balance: f64) -> Self {
        self.config.paper_trading.initial_balance = balance;
        self
    }
    
    /// Set maximum order value
    pub fn with_max_order_value(mut self, value: f64) -> Self {
        self.config.risk_management.max_order_value = value;
        self
    }
    
    /// Build and validate configuration
    pub fn build(self) -> ZerodhaResult<ZerodhaCredentialsConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

impl Default for ZerodhaCredentialsBuilder {
    fn default() -> Self {
        Self::new()
    }
}