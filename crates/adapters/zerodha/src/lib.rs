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

//! Nautilus Zerodha adapter for high-performance Indian market trading.
//!
//! This crate provides a comprehensive integration with Zerodha's Kite Connect API,
//! specifically optimized for options trading on NSE and BSE exchanges.
//!
//! # Features
//!
//! - Full REST API coverage for market data and trading
//! - Real-time WebSocket feeds for quotes and order updates
//! - Options trading with Greeks, implied volatility, and risk management
//! - SPAN margin calculations and position monitoring
//! - High-performance async Rust implementation
//!
//! # Quick Start
//!
//! ```no_run
//! use nautilus_zerodha::{ZerodhaHttpClient, ZerodhaConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ZerodhaConfig::new(
//!         "your_api_key".to_string(),
//!         "your_api_secret".to_string(),
//!         Some("your_access_token".to_string())
//!     );
//!     
//!     let client = ZerodhaHttpClient::new(config);
//!     let instruments = client.get_instruments().await?;
//!     
//!     println!("Loaded {} instruments from Zerodha", instruments.len());
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod enums;
pub mod error;
pub mod execution;
pub mod http;
pub mod types;
pub mod websocket;

#[cfg(feature = "python")]
pub mod python;

// Re-exports for convenience
pub use config::{credentials::ZerodhaCredentialsConfig, ZerodhaConfig, ZerodhaConfigBuilder};
pub use error::ZerodhaError;
pub use execution::{AccountHealthStatus, OrderModifyRequest, OrderRequest, PaperTradingEngine, PaperTradingStats, ZerodhaExecutionClient};
pub use http::ZerodhaHttpClient;
pub use types::*;

/// Current version of the Zerodha adapter
pub const ZERODHA_ADAPTER_VERSION: &str = env!("CARGO_PKG_VERSION");