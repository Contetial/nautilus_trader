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

//! Authentication module for Zerodha Kite Connect.
//!
//! This module provides utilities for generating access tokens programmatically
//! through the Kite Connect authentication flow.

pub mod token_generator;

pub use token_generator::{AccessTokenGenerator, LoginSession, SessionToken};

use crate::error::{ZerodhaError, ZerodhaResult};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Kite Connect authentication URLs
pub const KITE_LOGIN_URL: &str = "https://kite.zerodha.com/connect/login";
pub const KITE_SESSION_TOKEN_URL: &str = "https://api.kite.trade/session/token";
pub const KITE_LOGOUT_URL: &str = "https://api.kite.trade/session/token";

/// Response from session token endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTokenResponse {
    /// Status of the request
    pub status: String,
    /// Session data
    pub data: SessionData,
}

/// Session data containing access token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    /// User ID
    pub user_id: String,
    /// User name
    pub user_name: String,
    /// User shortname
    pub user_shortname: String,
    /// Email
    pub email: String,
    /// User type
    pub user_type: String,
    /// Broker
    pub broker: String,
    /// Exchanges enabled
    pub exchanges: Vec<String>,
    /// Products enabled
    pub products: Vec<String>,
    /// Order types enabled
    pub order_types: Vec<String>,
    /// Avatar URL
    pub avatar_url: Option<String>,
    /// Access token (the key we need!)
    pub access_token: String,
    /// Public token
    pub public_token: String,
    /// Refresh token
    pub refresh_token: Option<String>,
    /// Enctoken
    pub enctoken: Option<String>,
    /// Login time
    pub login_time: String,
    /// API key
    pub api_key: String,
}

/// Generate checksum for token exchange
pub fn generate_checksum(api_key: &str, request_token: &str, api_secret: &str) -> String {
    let data = format!("{}{}{}", api_key, request_token, api_secret);
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

/// Generate login URL for manual authentication
pub fn generate_login_url(api_key: &str, redirect_url: Option<&str>) -> String {
    let mut url = format!("{}?v=3&api_key={}", KITE_LOGIN_URL, api_key);
    
    if let Some(redirect) = redirect_url {
        url.push_str(&format!("&redirect_params={}", urlencoding::encode(redirect)));
    }
    
    url
}

/// Exchange request token for access token
pub async fn exchange_request_token(
    api_key: &str,
    api_secret: &str,
    request_token: &str,
) -> ZerodhaResult<SessionTokenResponse> {
    info!("🔄 Exchanging request token for access token...");
    
    let checksum = generate_checksum(api_key, request_token, api_secret);
    
    let mut params = HashMap::new();
    params.insert("api_key", api_key);
    params.insert("request_token", request_token);
    params.insert("checksum", &checksum);
    
    let client = Client::new();
    let response = client
        .post(KITE_SESSION_TOKEN_URL)
        .header("X-Kite-Version", "3")
        .form(&params)
        .send()
        .await
        .map_err(|e| ZerodhaError::network_error(&format!("Failed to exchange token: {}", e)))?;
    
    if response.status().is_success() {
        let session_response: SessionTokenResponse = response
            .json()
            .await
            .map_err(|e| ZerodhaError::parse_error(&format!("Failed to parse response: {}", e)))?;
        
        info!("✅ Access token generated successfully for user: {}", session_response.data.user_name);
        debug!("🔑 Access token: {}***", &session_response.data.access_token[..8]);
        
        Ok(session_response)
    } else {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        
        Err(ZerodhaError::auth_error(&format!(
            "Token exchange failed: HTTP {} - {}",
            response.status(),
            error_text
        )))
    }
}

/// Logout and invalidate access token
pub async fn logout(api_key: &str, access_token: &str) -> ZerodhaResult<()> {
    info!("🚪 Logging out and invalidating access token...");
    
    let logout_url = format!("{}?api_key={}&access_token={}", KITE_LOGOUT_URL, api_key, access_token);
    
    let client = Client::new();
    let response = client
        .delete(&logout_url)
        .header("X-Kite-Version", "3")
        .send()
        .await
        .map_err(|e| ZerodhaError::network_error(&format!("Failed to logout: {}", e)))?;
    
    if response.status().is_success() {
        info!("✅ Successfully logged out");
        Ok(())
    } else {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        
        warn!("⚠️  Logout failed: HTTP {} - {}", response.status(), error_text);
        
        // Don't treat logout failure as critical error
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_checksum() {
        let api_key = "test_key";
        let request_token = "test_request_token";
        let api_secret = "test_secret";
        
        let checksum = generate_checksum(api_key, request_token, api_secret);
        
        // Should be 64 character hex string (SHA-256)
        assert_eq!(checksum.len(), 64);
        assert!(checksum.chars().all(|c| c.is_ascii_hexdigit()));
        
        // Should be deterministic
        let checksum2 = generate_checksum(api_key, request_token, api_secret);
        assert_eq!(checksum, checksum2);
    }

    #[test]
    fn test_generate_login_url() {
        let api_key = "test_key";
        
        // Without redirect URL
        let url = generate_login_url(api_key, None);
        assert_eq!(url, "https://kite.zerodha.com/connect/login?v=3&api_key=test_key");
        
        // With redirect URL
        let redirect = "http://localhost:3000/callback";
        let url_with_redirect = generate_login_url(api_key, Some(redirect));
        assert!(url_with_redirect.contains("redirect_params="));
    }
}