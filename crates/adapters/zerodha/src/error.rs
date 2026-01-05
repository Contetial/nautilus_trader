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

//! Error types for the Zerodha adapter.

use thiserror::Error;

/// Errors that can occur when interacting with the Zerodha API.
#[derive(Error, Debug)]
pub enum ZerodhaError {
    /// HTTP client error
    #[error("HTTP client error: {0}")]
    Http(#[from] reqwest::Error),
    
    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    /// URL parsing error
    #[error("URL parsing error: {0}")]
    UrlParse(#[from] url::ParseError),
    
    /// WebSocket error
    #[error("WebSocket error: {0}")]
    WebSocket(String),
    
    /// Authentication error
    #[error("Authentication error: {message}")]
    Authentication { message: String },
    
    /// API error response from Zerodha
    #[error("Zerodha API error: {error_type} - {message} (status: {status})")]
    ApiError {
        error_type: String,
        message: String,
        status: String,
    },
    
    /// Rate limit exceeded
    #[error("Rate limit exceeded. Please retry after some time")]
    RateLimit,
    
    /// Invalid input parameters
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    /// Network timeout
    #[error("Network timeout: {0}")]
    Timeout(String),
    
    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),
    
    /// Instrument not found
    #[error("Instrument not found: {symbol}")]
    InstrumentNotFound { symbol: String },
    
    /// Trading not allowed (insufficient balance, position limits, etc.)
    #[error("Trading not allowed: {reason}")]
    TradingNotAllowed { reason: String },
    
    /// Order error
    #[error("Order error: {message}")]
    OrderError { message: String },
    
    /// Market closed
    #[error("Market is closed")]
    MarketClosed,
    
    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type alias for Zerodha operations
pub type ZerodhaResult<T> = Result<T, ZerodhaError>;

impl ZerodhaError {
    /// Create an API error from response
    pub fn api_error(error_type: String, message: String, status: String) -> Self {
        Self::ApiError {
            error_type,
            message,
            status,
        }
    }
    
    /// Create an authentication error
    pub fn auth_error(message: impl Into<String>) -> Self {
        Self::Authentication {
            message: message.into(),
        }
    }
    
    /// Create a configuration error
    pub fn config_error(message: impl Into<String>) -> Self {
        Self::Config(message.into())
    }
    
    /// Create an invalid input error
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput(message.into())
    }
    
    /// Create a WebSocket error
    pub fn websocket_error(message: impl Into<String>) -> Self {
        Self::WebSocket(message.into())
    }
    
    /// Create a validation error
    pub fn validation_error(message: impl Into<String>) -> Self {
        Self::InvalidInput(message.into())
    }
    
    /// Create a parse error
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }
    
    /// Create execution error
    pub fn execution_error(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }
    
    /// Check if error is recoverable (retry-able)
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            ZerodhaError::Http(_) |
            ZerodhaError::Timeout(_) |
            ZerodhaError::RateLimit |
            ZerodhaError::WebSocket(_)
        )
    }
    
    /// Check if error is due to authentication
    pub fn is_auth_error(&self) -> bool {
        matches!(self, ZerodhaError::Authentication { .. })
    }
    
    /// Check if error is due to rate limiting
    pub fn is_rate_limit(&self) -> bool {
        matches!(self, ZerodhaError::RateLimit)
    }
}

/// Parse API error response from Zerodha
pub fn parse_api_error(status: reqwest::StatusCode, body: &str) -> ZerodhaError {
    // Try to parse JSON error response
    if let Ok(error_response) = serde_json::from_str::<serde_json::Value>(body) {
        let error_type = error_response
            .get("error_type")
            .and_then(|v| v.as_str())
            .unwrap_or("UnknownError")
            .to_string();
            
        let message = error_response
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error occurred")
            .to_string();
            
        ZerodhaError::api_error(error_type, message, status.to_string())
    } else {
        // Fallback for non-JSON responses
        match status {
            reqwest::StatusCode::UNAUTHORIZED => {
                ZerodhaError::auth_error("Invalid API credentials or access token")
            }
            reqwest::StatusCode::FORBIDDEN => {
                ZerodhaError::auth_error("Access forbidden. Check API permissions")
            }
            reqwest::StatusCode::TOO_MANY_REQUESTS => ZerodhaError::RateLimit,
            reqwest::StatusCode::BAD_REQUEST => {
                ZerodhaError::invalid_input(format!("Bad request: {}", body))
            }
            reqwest::StatusCode::INTERNAL_SERVER_ERROR => {
                ZerodhaError::Internal("Zerodha server error".to_string())
            }
            _ => ZerodhaError::api_error(
                "HttpError".to_string(),
                format!("HTTP {} - {}", status, body),
                status.to_string(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;

    #[test]
    fn test_error_creation() {
        let error = ZerodhaError::auth_error("Invalid token");
        assert!(error.is_auth_error());
        assert!(!error.is_rate_limit());
        assert!(!error.is_recoverable());
    }

    #[test]
    fn test_rate_limit_error() {
        let error = ZerodhaError::RateLimit;
        assert!(error.is_rate_limit());
        assert!(error.is_recoverable());
    }

    #[test]
    fn test_parse_api_error_json() {
        let json_error = r#"{"status":"error","error_type":"TokenException","message":"Invalid access token"}"#;
        let error = parse_api_error(StatusCode::UNAUTHORIZED, json_error);
        
        match error {
            ZerodhaError::ApiError { error_type, message, .. } => {
                assert_eq!(error_type, "TokenException");
                assert_eq!(message, "Invalid access token");
            }
            _ => panic!("Expected ApiError"),
        }
    }

    #[test]
    fn test_parse_api_error_fallback() {
        let error = parse_api_error(StatusCode::UNAUTHORIZED, "Unauthorized");
        assert!(error.is_auth_error());
    }
}