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

//! Enumerations for the Zerodha adapter.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// Exchange identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
#[strum(serialize_all = "UPPERCASE")]
pub enum Exchange {
    /// National Stock Exchange
    NSE,
    /// Bombay Stock Exchange  
    BSE,
    /// NSE Futures & Options
    NFO,
    /// BSE Futures & Options
    BFO,
    /// Currency Derivatives Segment
    CDS,
    /// Multi Commodity Exchange
    MCX,
}

/// Market segments
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
#[strum(serialize_all = "UPPERCASE")]
pub enum Segment {
    /// Equity segment
    EQ,
    /// Futures & Options segment
    FO,
    /// Currency segment
    CD,
    /// Commodity segment
    COM,
}

/// Instrument types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
#[strum(serialize_all = "UPPERCASE")]
pub enum InstrumentType {
    /// Equity shares
    EQ,
    /// Index
    INDEX,
    /// Futures
    FUT,
    /// Call options
    CE,
    /// Put options
    PE,
    /// Currency futures
    #[strum(serialize = "CURRENCY")]
    Currency,
    /// Commodity futures
    #[strum(serialize = "COMMODITY")]
    Commodity,
}

/// Option types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
pub enum OptionType {
    /// Call option
    #[serde(rename = "CE")]
    #[strum(serialize = "CE")]
    Call,
    /// Put option  
    #[serde(rename = "PE")]
    #[strum(serialize = "PE")]
    Put,
}

/// Order types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
#[strum(serialize_all = "UPPERCASE")]
pub enum OrderType {
    /// Market order
    MARKET,
    /// Limit order
    LIMIT,
    /// Stop-loss order
    SL,
    /// Stop-loss market order
    #[serde(rename = "SL-M")]
    #[strum(serialize = "SL-M")]
    SLM,
}

/// Transaction types (buy/sell)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
#[strum(serialize_all = "UPPERCASE")]
pub enum TransactionType {
    /// Buy order
    BUY,
    /// Sell order
    SELL,
}

/// Order validity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
#[strum(serialize_all = "UPPERCASE")]
pub enum Validity {
    /// Day order
    DAY,
    /// Immediate or Cancel
    IOC,
    /// Fill or Kill
    TTL, // Zerodha uses TTL for GTC-like behavior
}

/// Product types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
#[strum(serialize_all = "UPPERCASE")]
pub enum Product {
    /// Cash & Carry (delivery)
    CNC,
    /// Normal (F&O, intraday equity)
    NRML,
    /// Margin Intraday Square-off
    MIS,
    /// Bracket Order
    BO,
    /// Cover Order
    CO,
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
#[strum(serialize_all = "UPPERCASE")]
pub enum OrderStatus {
    /// Order has been placed
    OPEN,
    /// Order has been completed
    COMPLETE,
    /// Order has been cancelled
    CANCELLED,
    /// Order has been rejected
    REJECTED,
    /// Order is being processed
    #[serde(rename = "TRIGGER PENDING")]
    #[strum(serialize = "TRIGGER PENDING")]
    TriggerPending,
}

/// Position types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum PositionType {
    /// Long position
    #[serde(rename = "long")]
    Long,
    /// Short position
    #[serde(rename = "short")]
    Short,
}

/// WebSocket modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TickerMode {
    /// Last trade price only
    LTP = 256,
    /// LTP + volume traded
    Quote = 512,
    /// Quote + market depth
    Full = 768,
}

impl TickerMode {
    /// Get the numeric value for the ticker mode
    pub fn value(&self) -> u32 {
        *self as u32
    }
}

impl std::fmt::Display for TickerMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TickerMode::LTP => write!(f, "ltp"),
            TickerMode::Quote => write!(f, "quote"),
            TickerMode::Full => write!(f, "full"),
        }
    }
}

/// Market status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum MarketStatus {
    /// Market is open
    Open,
    /// Market is closed
    Closed,
    /// Pre-market session
    #[serde(rename = "pre_open")]
    PreOpen,
    /// After market session
    #[serde(rename = "after_market")]
    AfterMarket,
}

/// Interval for historical data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display)]
pub enum Interval {
    /// 1 minute
    #[serde(rename = "minute")]
    #[strum(serialize = "minute")]
    Minute,
    /// 2 minutes
    #[serde(rename = "2minute")]
    #[strum(serialize = "2minute")]
    TwoMinute,
    /// 3 minutes
    #[serde(rename = "3minute")]
    #[strum(serialize = "3minute")]
    ThreeMinute,
    /// 5 minutes
    #[serde(rename = "5minute")]
    #[strum(serialize = "5minute")]
    FiveMinute,
    /// 10 minutes
    #[serde(rename = "10minute")]
    #[strum(serialize = "10minute")]
    TenMinute,
    /// 15 minutes
    #[serde(rename = "15minute")]
    #[strum(serialize = "15minute")]
    FifteenMinute,
    /// 30 minutes
    #[serde(rename = "30minute")]
    #[strum(serialize = "30minute")]
    ThirtyMinute,
    /// 60 minutes (1 hour)
    #[serde(rename = "60minute")]
    #[strum(serialize = "60minute")]
    Hourly,
    /// Daily
    #[serde(rename = "day")]
    #[strum(serialize = "day")]
    Daily,
}

impl Default for TickerMode {
    fn default() -> Self {
        Self::LTP
    }
}

impl Default for OrderType {
    fn default() -> Self {
        Self::MARKET
    }
}

impl Default for TransactionType {
    fn default() -> Self {
        Self::BUY
    }
}

impl Default for Validity {
    fn default() -> Self {
        Self::DAY
    }
}

impl Default for Product {
    fn default() -> Self {
        Self::MIS
    }
}

impl TransactionType {
    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            TransactionType::BUY => "BUY",
            TransactionType::SELL => "SELL",
        }
    }
}

/// Type alias for backward compatibility
pub type ProductType = Product;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ticker_mode_values() {
        assert_eq!(TickerMode::LTP.value(), 256);
        assert_eq!(TickerMode::Quote.value(), 512);
        assert_eq!(TickerMode::Full.value(), 768);
    }

    #[test]
    fn test_enum_serialization() {
        assert_eq!(serde_json::to_string(&OrderType::MARKET).unwrap(), "\"MARKET\"");
        assert_eq!(serde_json::to_string(&TransactionType::BUY).unwrap(), "\"BUY\"");
        assert_eq!(serde_json::to_string(&OptionType::Call).unwrap(), "\"CE\"");
    }

    #[test]
    fn test_enum_string_conversion() {
        assert_eq!(Exchange::NSE.to_string(), "NSE");
        assert_eq!(InstrumentType::EQ.to_string(), "EQ");
        
        assert_eq!("NSE".parse::<Exchange>().unwrap(), Exchange::NSE);
        assert_eq!("EQ".parse::<InstrumentType>().unwrap(), InstrumentType::EQ);
    }
}