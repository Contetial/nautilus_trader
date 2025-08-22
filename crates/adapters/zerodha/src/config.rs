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

//! Configuration structures for the Zerodha adapter.

pub mod credentials;

use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for the Zerodha Kite Connect API client.
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(setter(into), default)]
pub struct ZerodhaConfig {
    /// Zerodha API key (from Kite Connect app)
    pub api_key: String,
    
    /// Zerodha API secret (from Kite Connect app)
    pub api_secret: String,
    
    /// Access token for API authentication
    /// If None, will need to be generated through login flow
    pub access_token: Option<String>,
    
    /// Whether to use sandbox/demo mode
    pub sandbox: bool,
    
    /// Base URL for REST API endpoints
    pub base_url: String,
    
    /// WebSocket URL for real-time data
    pub websocket_url: String,
    
    /// Request timeout duration
    pub request_timeout: Duration,
    
    /// Maximum number of API requests per second
    pub rate_limit_per_second: u32,
    
    /// Maximum number of retries for failed requests
    pub max_retries: u32,
    
    /// Enable detailed logging of API requests/responses
    pub debug_mode: bool,
}

impl Default for ZerodhaConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_secret: String::new(),
            access_token: None,
            sandbox: true,
            base_url: "https://api.kite.trade".to_string(),
            websocket_url: "wss://ws.kite.trade".to_string(),
            request_timeout: Duration::from_secs(30),
            rate_limit_per_second: 3, // Zerodha limit is 3 requests/second
            max_retries: 3,
            debug_mode: false,
        }
    }
}

impl ZerodhaConfig {
    /// Create a new Zerodha configuration with required parameters
    pub fn new(api_key: String, api_secret: String, access_token: Option<String>) -> Self {
        Self {
            api_key,
            api_secret,
            access_token,
            ..Default::default()
        }
    }
    
    /// Set sandbox mode
    pub fn with_sandbox(mut self, sandbox: bool) -> Self {
        self.sandbox = sandbox;
        if sandbox {
            // Note: Zerodha doesn't have a separate sandbox URL
            // Sandbox mode is handled by using paper trading account
            self.base_url = "https://api.kite.trade".to_string();
        }
        self
    }
    
    /// Set request timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }
    
    /// Set rate limiting
    pub fn with_rate_limit(mut self, requests_per_second: u32) -> Self {
        self.rate_limit_per_second = requests_per_second;
        self
    }
    
    /// Enable debug mode
    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug_mode = debug;
        self
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.api_key.is_empty() {
            return Err("API key is required".to_string());
        }
        
        if self.api_secret.is_empty() {
            return Err("API secret is required".to_string());
        }
        
        if self.request_timeout.as_secs() == 0 {
            return Err("Request timeout must be greater than 0".to_string());
        }
        
        if self.rate_limit_per_second == 0 {
            return Err("Rate limit must be greater than 0".to_string());
        }
        
        Ok(())
    }
    
    /// Get the login URL for manual token generation
    pub fn get_login_url(&self) -> String {
        format!(
            "https://kite.trade/connect/login?api_key={}&v=3",
            self.api_key
        )
    }
}

/// Configuration for WebSocket connections
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(setter(into), default)]
pub struct ZerodhaWebSocketConfig {
    /// API key for authentication
    pub api_key: String,
    
    /// Access token for authentication
    pub access_token: String,
    
    /// WebSocket connection URL
    pub url: String,
    
    /// Ping interval to keep connection alive
    pub ping_interval: Duration,
    
    /// Connection timeout
    pub connect_timeout: Duration,
    
    /// Automatic reconnection on disconnect
    pub auto_reconnect: bool,
    
    /// Maximum reconnection attempts
    pub max_reconnect_attempts: u32,
    
    /// Delay between reconnection attempts
    pub reconnect_delay: Duration,
}

impl Default for ZerodhaWebSocketConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            access_token: String::new(),
            url: "wss://ws.kite.trade".to_string(),
            ping_interval: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            auto_reconnect: true,
            max_reconnect_attempts: 5,
            reconnect_delay: Duration::from_secs(5),
        }
    }
}

impl From<&ZerodhaConfig> for ZerodhaWebSocketConfig {
    fn from(config: &ZerodhaConfig) -> Self {
        Self {
            api_key: config.api_key.clone(),
            access_token: config.access_token.clone().unwrap_or_default(),
            url: config.websocket_url.clone(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let mut config = ZerodhaConfig::default();
        
        // Should fail with empty API key
        assert!(config.validate().is_err());
        
        config.api_key = "test_key".to_string();
        // Should fail with empty API secret
        assert!(config.validate().is_err());
        
        config.api_secret = "test_secret".to_string();
        // Should pass with both key and secret
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_config_builder() {
        let config = ZerodhaConfigBuilder::default()
            .api_key("test_key".to_string())
            .api_secret("test_secret".to_string())
            .sandbox(true)
            .debug_mode(true)
            .build()
            .unwrap();
            
        assert_eq!(config.api_key, "test_key");
        assert_eq!(config.api_secret, "test_secret");
        assert!(config.sandbox);
        assert!(config.debug_mode);
    }
    
    #[test]
    fn test_login_url_generation() {
        let config = ZerodhaConfig::new(
            "test_api_key".to_string(),
            "test_secret".to_string(),
            None
        );
        
        let login_url = config.get_login_url();
        assert!(login_url.contains("test_api_key"));
        assert!(login_url.contains("kite.trade/connect/login"));
    }
}

/// Configuration for Zerodha execution client
#[derive(Debug, Clone)]
pub struct ZerodhaExecutionConfig {
    /// Maximum order quantity per trade
    pub max_order_quantity: u32,
    /// Enable pre-trade validation
    pub enable_validation: bool,
    /// Maximum position value (in INR)
    pub max_position_value: f64,
    /// Enable margin checks before order placement
    pub enable_margin_check: bool,
    /// Default order validity
    pub default_validity: crate::enums::Validity,
    /// Default product type
    pub default_product_type: crate::enums::ProductType,
    /// Enable paper trading mode (no real orders)
    pub paper_trading: bool,
    /// Paper trading initial balance (in INR)
    pub paper_balance: f64,
    /// Paper trading commission per trade (in INR)
    pub paper_commission: f64,
    /// Enable order simulation delays
    pub simulate_latency: bool,
    /// Simulated order execution delay (ms)
    pub execution_delay_ms: u64,
}

impl Default for ZerodhaExecutionConfig {
    fn default() -> Self {
        Self {
            max_order_quantity: 10000,
            enable_validation: true,
            max_position_value: 1_000_000.0, // 10 Lakh INR
            enable_margin_check: true,
            default_validity: crate::enums::Validity::Day,
            default_product_type: crate::enums::ProductType::MIS,
            paper_trading: false,
            paper_balance: 1_000_000.0, // 10 Lakh INR starting balance
            paper_commission: 20.0, // ₹20 per trade
            simulate_latency: true,
            execution_delay_ms: 100, // 100ms simulated execution delay
        }
    }
}

impl ZerodhaExecutionConfig {
    /// Create new execution configuration
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Enable paper trading mode with default settings
    pub fn with_paper_trading(mut self) -> Self {
        self.paper_trading = true;
        self.simulate_latency = true;
        self
    }
    
    /// Set paper trading balance
    pub fn with_paper_balance(mut self, balance: f64) -> Self {
        self.paper_balance = balance;
        self
    }
    
    /// Set paper trading commission per trade
    pub fn with_paper_commission(mut self, commission: f64) -> Self {
        self.paper_commission = commission;
        self
    }
    
    /// Set simulated execution delay
    pub fn with_execution_delay(mut self, delay_ms: u64) -> Self {
        self.execution_delay_ms = delay_ms;
        self
    }
    
    /// Create paper trading configuration for testing
    pub fn paper_trading_config() -> Self {
        Self::default()
            .with_paper_trading()
            .with_paper_balance(100_000.0) // 1 Lakh for testing
            .with_paper_commission(10.0)   // Lower commission for testing
            .with_execution_delay(50)      // Faster execution for testing
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.max_order_quantity == 0 {
            return Err("Maximum order quantity cannot be zero".to_string());
        }
        
        if self.max_position_value <= 0.0 {
            return Err("Maximum position value must be positive".to_string());
        }
        
        if self.paper_trading && self.paper_balance <= 0.0 {
            return Err("Paper trading balance must be positive".to_string());
        }
        
        if self.paper_commission < 0.0 {
            return Err("Paper commission cannot be negative".to_string());
        }
        
        Ok(())
    }
}