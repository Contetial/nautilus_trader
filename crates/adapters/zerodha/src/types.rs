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

//! Data types for the Zerodha adapter.

use crate::enums::*;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Instrument information from Zerodha
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZerodhaInstrument {
    /// Instrument token (unique identifier)
    pub instrument_token: u32,
    /// Exchange token
    pub exchange_token: u32,
    /// Trading symbol
    pub tradingsymbol: String,
    /// Company/instrument name
    pub name: String,
    /// Last price
    pub last_price: Decimal,
    /// Expiry date (for derivatives)
    pub expiry: Option<NaiveDate>,
    /// Strike price (for options)
    pub strike: Option<Decimal>,
    /// Tick size
    pub tick_size: Decimal,
    /// Lot size
    pub lot_size: u32,
    /// Instrument type
    pub instrument_type: InstrumentType,
    /// Segment
    pub segment: Segment,
    /// Exchange
    pub exchange: Exchange,
}

/// Quote data for an instrument
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZerodhaQuote {
    /// Instrument token
    pub instrument_token: u32,
    /// Timestamp of the quote
    pub timestamp: Option<DateTime<Utc>>,
    /// Last traded price
    pub last_price: Decimal,
    /// Last traded quantity
    pub last_quantity: Option<u32>,
    /// Average traded price
    pub average_price: Option<Decimal>,
    /// Volume traded for the day
    pub volume: u32,
    /// Buy quantity
    pub buy_quantity: Option<u32>,
    /// Sell quantity
    pub sell_quantity: Option<u32>,
    /// Open price
    pub ohlc: OHLC,
    /// Net change from previous close
    pub net_change: Option<Decimal>,
    /// Open Interest (for F&O)
    pub oi: Option<u32>,
    /// Change in Open Interest
    pub oi_day_high: Option<u32>,
    /// Day's high OI
    pub oi_day_low: Option<u32>,
    /// Market depth
    pub depth: Option<MarketDepth>,
}

/// OHLC data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OHLC {
    /// Open price
    pub open: Decimal,
    /// High price
    pub high: Decimal,
    /// Low price
    pub low: Decimal,
    /// Close price
    pub close: Decimal,
}

/// Market depth (order book)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarketDepth {
    /// Buy side depth
    pub buy: Vec<DepthItem>,
    /// Sell side depth
    pub sell: Vec<DepthItem>,
}

/// Individual depth item
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DepthItem {
    /// Price level
    pub price: Decimal,
    /// Quantity at this level
    pub quantity: u32,
    /// Number of orders
    pub orders: u32,
}

/// Order information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZerodhaOrder {
    /// Account ID
    pub account_id: String,
    /// Placed by user ID
    pub placed_by: String,
    /// Order ID
    pub order_id: String,
    /// Exchange order ID
    pub exchange_order_id: Option<String>,
    /// Parent order ID (for bracket/cover orders)
    pub parent_order_id: Option<String>,
    /// Order status
    pub status: OrderStatus,
    /// Status message
    pub status_message: Option<String>,
    /// Order timestamp
    pub order_timestamp: DateTime<Utc>,
    /// Exchange timestamp
    pub exchange_timestamp: Option<DateTime<Utc>>,
    /// Variety (regular, amo, bo, co, iceberg, auction)
    pub variety: String,
    /// Exchange
    pub exchange: Exchange,
    /// Trading symbol
    pub tradingsymbol: String,
    /// Instrument token
    pub instrument_token: u32,
    /// Order type
    pub order_type: OrderType,
    /// Transaction type
    pub transaction_type: TransactionType,
    /// Validity
    pub validity: Validity,
    /// Product type
    pub product: Product,
    /// Quantity
    pub quantity: u32,
    /// Disclosed quantity
    pub disclosed_quantity: Option<u32>,
    /// Price
    pub price: Decimal,
    /// Trigger price
    pub trigger_price: Option<Decimal>,
    /// Average price
    pub average_price: Option<Decimal>,
    /// Filled quantity
    pub filled_quantity: u32,
    /// Pending quantity
    pub pending_quantity: u32,
    /// Cancelled quantity
    pub cancelled_quantity: u32,
    /// Market protection percentage
    pub market_protection: Option<Decimal>,
    /// Tag
    pub tag: Option<String>,
}

/// Position information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZerodhaPosition {
    /// Account ID
    pub account_id: String,
    /// Exchange
    pub exchange: Exchange,
    /// Trading symbol
    pub tradingsymbol: String,
    /// Instrument token
    pub instrument_token: u32,
    /// Product type
    pub product: Product,
    /// Quantity (net)
    pub quantity: i32,
    /// Overnight quantity
    pub overnight_quantity: i32,
    /// T1 quantity
    pub t1_quantity: i32,
    /// Realised P&L
    pub realised: Decimal,
    /// Unrealised P&L
    pub unrealised: Decimal,
    /// Value of the position
    pub value: Decimal,
    /// P&L
    pub pnl: Decimal,
    /// M2M (Mark to Market)
    pub m2m: Decimal,
    /// Multiplier
    pub multiplier: Decimal,
    /// Average price
    pub average_price: Decimal,
    /// Last price
    pub last_price: Decimal,
    /// Close price
    pub close_price: Decimal,
    /// Buy quantity
    pub buy_quantity: u32,
    /// Buy price
    pub buy_price: Decimal,
    /// Buy value
    pub buy_value: Decimal,
    /// Buy M2M
    pub buy_m2m: Decimal,
    /// Sell quantity
    pub sell_quantity: u32,
    /// Sell price
    pub sell_price: Decimal,
    /// Sell value
    pub sell_value: Decimal,
    /// Sell M2M
    pub sell_m2m: Decimal,
    /// Day buy quantity
    pub day_buy_quantity: u32,
    /// Day buy price
    pub day_buy_price: Decimal,
    /// Day buy value
    pub day_buy_value: Decimal,
    /// Day sell quantity
    pub day_sell_quantity: u32,
    /// Day sell price
    pub day_sell_price: Decimal,
    /// Day sell value
    pub day_sell_value: Decimal,
}

/// Historical candle data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZerodhaCandle {
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Open price
    pub open: Decimal,
    /// High price
    pub high: Decimal,
    /// Low price
    pub low: Decimal,
    /// Close price
    pub close: Decimal,
    /// Volume
    pub volume: u32,
    /// Open Interest (for derivatives)
    pub oi: Option<u32>,
}

/// Margin information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZerodhaMargin {
    /// Available cash
    pub available: HashMap<String, Decimal>,
    /// Used margins
    pub utilised: HashMap<String, Decimal>,
    /// Net cash balance
    pub net: Decimal,
}

/// WebSocket tick data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZerodhaTick {
    /// Instrument token
    pub instrument_token: u32,
    /// Exchange timestamp
    pub exchange_timestamp: Option<DateTime<Utc>>,
    /// Last traded price
    pub last_price: Decimal,
    /// Last traded quantity
    pub last_quantity: Option<u32>,
    /// Average traded price
    pub average_price: Option<Decimal>,
    /// Volume traded
    pub volume_traded: Option<u32>,
    /// Total buy quantity
    pub total_buy_quantity: Option<u32>,
    /// Total sell quantity
    pub total_sell_quantity: Option<u32>,
    /// OHLC data
    pub ohlc: Option<OHLC>,
    /// Change from previous day close
    pub change: Option<Decimal>,
    /// Open Interest
    pub oi: Option<u32>,
    /// Change in Open Interest
    pub oi_change: Option<i32>,
    /// Market depth
    pub depth: Option<MarketDepth>,
    /// Tick mode
    pub mode: TickerMode,
}

/// API response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZerodhaResponse<T> {
    /// Status of the response
    pub status: String,
    /// Response data
    pub data: Option<T>,
    /// Error type (if any)
    pub error_type: Option<String>,
    /// Error message (if any)
    pub message: Option<String>,
}

impl<T> ZerodhaResponse<T> {
    /// Check if the response is successful
    pub fn is_success(&self) -> bool {
        self.status == "success"
    }
    
    /// Get the data or return an error
    pub fn into_result(self) -> Result<T, crate::error::ZerodhaError> {
        if self.is_success() {
            self.data.ok_or_else(|| {
                crate::error::ZerodhaError::Internal("No data in successful response".to_string())
            })
        } else {
            Err(crate::error::ZerodhaError::api_error(
                self.error_type.unwrap_or_else(|| "UnknownError".to_string()),
                self.message.unwrap_or_else(|| "Unknown error".to_string()),
                self.status,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_response_deserialization() {
        let json = r#"{"status":"success","data":{"instrument_token":256265}}"#;
        let response: ZerodhaResponse<serde_json::Value> = serde_json::from_str(json).unwrap();
        
        assert!(response.is_success());
        assert!(response.data.is_some());
    }

    #[test]
    fn test_error_response() {
        let json = r#"{"status":"error","error_type":"TokenException","message":"Invalid token"}"#;
        let response: ZerodhaResponse<serde_json::Value> = serde_json::from_str(json).unwrap();
        
        assert!(!response.is_success());
        assert_eq!(response.error_type, Some("TokenException".to_string()));
        assert_eq!(response.message, Some("Invalid token".to_string()));
    }
}

/// Order status enumeration
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ZerodhaOrderStatus {
    /// Order is open/pending
    #[serde(rename = "OPEN")]
    Open,
    /// Order is complete/filled
    #[serde(rename = "COMPLETE")]
    Complete,
    /// Order is cancelled
    #[serde(rename = "CANCELLED")]
    Cancelled,
    /// Order is rejected
    #[serde(rename = "REJECTED")]
    Rejected,
    /// Order is pending trigger
    #[serde(rename = "TRIGGER PENDING")]
    Trigger,
    /// Order is pending at exchange
    #[serde(rename = "PENDING")]
    Pending,
}

impl From<crate::enums::OrderStatus> for ZerodhaOrderStatus {
    fn from(status: crate::enums::OrderStatus) -> Self {
        match status {
            crate::enums::OrderStatus::OPEN => ZerodhaOrderStatus::Open,
            crate::enums::OrderStatus::COMPLETE => ZerodhaOrderStatus::Complete,
            crate::enums::OrderStatus::CANCELLED => ZerodhaOrderStatus::Cancelled,
            crate::enums::OrderStatus::REJECTED => ZerodhaOrderStatus::Rejected,
            crate::enums::OrderStatus::TriggerPending => ZerodhaOrderStatus::Trigger,
        }
    }
}

impl From<ZerodhaOrderStatus> for crate::enums::OrderStatus {
    fn from(status: ZerodhaOrderStatus) -> Self {
        match status {
            ZerodhaOrderStatus::Open => crate::enums::OrderStatus::OPEN,
            ZerodhaOrderStatus::Complete => crate::enums::OrderStatus::COMPLETE,
            ZerodhaOrderStatus::Cancelled => crate::enums::OrderStatus::CANCELLED,
            ZerodhaOrderStatus::Rejected => crate::enums::OrderStatus::REJECTED,
            ZerodhaOrderStatus::Trigger => crate::enums::OrderStatus::TriggerPending,
            ZerodhaOrderStatus::Pending => crate::enums::OrderStatus::OPEN, // Map pending to open
        }
    }
}

impl std::fmt::Display for ZerodhaOrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZerodhaOrderStatus::Open => write!(f, "OPEN"),
            ZerodhaOrderStatus::Complete => write!(f, "COMPLETE"),
            ZerodhaOrderStatus::Cancelled => write!(f, "CANCELLED"),
            ZerodhaOrderStatus::Rejected => write!(f, "REJECTED"),
            ZerodhaOrderStatus::Trigger => write!(f, "TRIGGER PENDING"),
            ZerodhaOrderStatus::Pending => write!(f, "PENDING"),
        }
    }
}


/// Order response from API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZerodhaOrderResponse {
    /// Order ID assigned by Zerodha
    pub order_id: String,
}

/// Margin response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZerodhaMarginResponse {
    /// Available cash
    pub available_cash: f64,
    /// Net available balance
    pub available: MarginDetail,
    /// Used margins
    pub utilised: MarginDetail,
}

/// Margin detail structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarginDetail {
    /// Adhoc margin
    pub adhoc_margin: f64,
    /// Cash balance
    pub cash: f64,
    /// Collateral value
    pub collateral: f64,
    /// Intraday payin
    pub intraday_payin: f64,
    /// Live balance
    pub live_balance: f64,
    /// Opening balance
    pub opening_balance: f64,
    /// Payin amount
    pub payin: f64,
    /// Payout amount  
    pub payout: f64,
    /// SPAN margin
    pub span: f64,
    /// Total amount
    pub total: f64,
    /// Net amount
    pub net: f64,
}

/// User profile information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZerodhaUserProfile {
    /// User ID
    pub user_id: String,
    /// User shortname
    pub user_shortname: String,
    /// Full name
    pub user_name: String,
    /// User type
    pub user_type: String,
    /// Avatar URL
    pub avatar_url: Option<String>,
    /// Email
    pub email: String,
    /// Broker
    pub broker: String,
    /// Exchanges enabled
    pub exchanges: Vec<String>,
    /// Products enabled
    pub products: Vec<String>,
    /// Order types enabled
    pub order_types: Vec<String>,
    /// Account access
    pub account_access: AccountAccess,
}

/// Account access details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountAccess {
    /// Kite enabled
    pub kite: bool,
    /// Console enabled
    pub console: bool,
    /// Coin enabled
    pub coin: bool,
}

/// Account summary combining profile and margin data
#[derive(Debug, Clone)]
pub struct ZerodhaAccountSummary {
    /// User profile
    pub profile: ZerodhaUserProfile,
    /// Margin information
    pub margins: ZerodhaMarginResponse,
    /// Last updated timestamp
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Account balance information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZerodhaAccountBalance {
    /// Available cash balance
    pub available_cash: f64,
    /// Used margin
    pub used_margin: f64,
    /// Net balance
    pub net_balance: f64,
    /// Opening balance
    pub opening_balance: f64,
    /// Live balance
    pub live_balance: f64,
    /// Last updated
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

// Type aliases for backward compatibility and alternative naming
pub type ZerodhaOHLC = OHLC;
pub type ZerodhaDepth = MarketDepth;
pub type ZerodhaDepthItem = DepthItem;