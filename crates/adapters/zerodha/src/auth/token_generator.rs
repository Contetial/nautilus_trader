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

//! Interactive access token generator for Zerodha Kite Connect.
//!
//! This module provides a complete solution for generating access tokens
//! through the Kite Connect authentication flow programmatically.

use crate::auth::{exchange_request_token, generate_login_url, SessionTokenResponse};
use crate::error::{ZerodhaError, ZerodhaResult};
use std::io::{self, Write};
use tracing::{info, warn};

/// Login session state
#[derive(Debug, Clone)]
pub struct LoginSession {
    pub api_key: String,
    pub api_secret: String,
    pub login_url: String,
    pub request_token: Option<String>,
    pub access_token: Option<String>,
}

impl LoginSession {
    /// Create a new login session
    pub fn new(api_key: String, api_secret: String) -> Self {
        let login_url = generate_login_url(&api_key, None);
        
        Self {
            api_key,
            api_secret,
            login_url,
            request_token: None,
            access_token: None,
        }
    }

    /// Check if session has valid access token
    pub fn is_authenticated(&self) -> bool {
        self.access_token.is_some()
    }

    /// Get access token or error
    pub fn access_token(&self) -> ZerodhaResult<&str> {
        self.access_token
            .as_ref()
            .map(|s| s.as_str())
            .ok_or_else(|| ZerodhaError::auth_error("No access token available"))
    }
}

/// Session token information
#[derive(Debug, Clone)]
pub struct SessionToken {
    pub access_token: String,
    pub user_id: String,
    pub user_name: String,
    pub broker: String,
    pub email: String,
    pub exchanges: Vec<String>,
    pub products: Vec<String>,
    pub order_types: Vec<String>,
}

impl From<SessionTokenResponse> for SessionToken {
    fn from(response: SessionTokenResponse) -> Self {
        Self {
            access_token: response.data.access_token,
            user_id: response.data.user_id,
            user_name: response.data.user_name,
            broker: response.data.broker,
            email: response.data.email,
            exchanges: response.data.exchanges,
            products: response.data.products,
            order_types: response.data.order_types,
        }
    }
}

/// Interactive access token generator
pub struct AccessTokenGenerator {
    session: LoginSession,
}

impl AccessTokenGenerator {
    /// Create new generator with API credentials
    pub fn new(api_key: String, api_secret: String) -> Self {
        let session = LoginSession::new(api_key, api_secret);
        
        Self { session }
    }

    /// Generate access token interactively
    pub async fn generate_token_interactive(&mut self) -> ZerodhaResult<SessionToken> {
        info!("🚀 Starting Zerodha Kite Connect authentication flow...");
        
        // Step 1: Display login URL
        self.display_login_instructions();
        
        // Step 2: Get request token from user
        let request_token = self.get_request_token_from_user()?;
        self.session.request_token = Some(request_token.clone());
        
        // Step 3: Exchange for access token
        info!("🔄 Exchanging request token for access token...");
        let response = exchange_request_token(
            &self.session.api_key,
            &self.session.api_secret,
            &request_token,
        ).await?;
        
        self.session.access_token = Some(response.data.access_token.clone());
        
        let session_token = SessionToken::from(response);
        
        info!("✅ Access token generated successfully!");
        info!("📋 User: {} ({})", session_token.user_name, session_token.user_id);
        info!("🏦 Broker: {}", session_token.broker);
        info!("📧 Email: {}", session_token.email);
        info!("🔑 Access Token: {}***", &session_token.access_token[..8]);
        
        Ok(session_token)
    }

    /// Generate access token with provided request token
    pub async fn generate_token_with_request_token(&mut self, request_token: &str) -> ZerodhaResult<SessionToken> {
        info!("🔄 Exchanging provided request token for access token...");
        
        let response = exchange_request_token(
            &self.session.api_key,
            &self.session.api_secret,
            request_token,
        ).await?;
        
        self.session.request_token = Some(request_token.to_string());
        self.session.access_token = Some(response.data.access_token.clone());
        
        let session_token = SessionToken::from(response);
        
        info!("✅ Access token generated successfully!");
        Ok(session_token)
    }

    /// Display login instructions to user
    fn display_login_instructions(&self) {
        println!("\n🔐 ZERODHA KITE CONNECT AUTHENTICATION");
        println!("=====================================");
        println!();
        println!("To generate your access token, please follow these steps:");
        println!();
        println!("1. 🌐 Open this URL in your browser:");
        println!("   {}", self.session.login_url);
        println!();
        println!("2. 🔑 Log in to your Zerodha account");
        println!();
        println!("3. ✅ Authorize the application");
        println!();
        println!("4. 📋 Copy the 'request_token' from the redirect URL");
        println!("   The URL will look like:");
        println!("   http://127.0.0.1/?request_token=YOUR_REQUEST_TOKEN&action=login&status=success");
        println!();
        println!("5. 📝 Paste the request_token below");
        println!();
    }

    /// Get request token from user input
    fn get_request_token_from_user(&self) -> ZerodhaResult<String> {
        print!("Enter request_token: ");
        io::stdout().flush().map_err(|e| ZerodhaError::io_error(&format!("Failed to flush stdout: {}", e)))?;
        
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| ZerodhaError::io_error(&format!("Failed to read input: {}", e)))?;
        
        let request_token = input.trim().to_string();
        
        if request_token.is_empty() {
            return Err(ZerodhaError::validation_error("Request token cannot be empty"));
        }
        
        // Basic validation - request tokens are typically 32 character alphanumeric
        if request_token.len() != 32 || !request_token.chars().all(|c| c.is_alphanumeric()) {
            warn!("⚠️  Request token format looks unusual (expected 32 alphanumeric characters)");
            warn!("⚠️  Continuing anyway, but double-check if authentication fails...");
        }
        
        Ok(request_token)
    }

    /// Save access token to configuration file
    pub fn save_to_config(&self, config_path: Option<&str>) -> ZerodhaResult<()> {
        let access_token = self.session.access_token()?;
        
        // TODO: Integrate with configuration system to update the TOML file
        // For now, just display instructions
        
        println!("\n💾 ACCESS TOKEN GENERATED");
        println!("========================");
        println!();
        println!("🔑 Your access token: {}", access_token);
        println!();
        println!("📝 To save this token, add it to your configuration file:");
        println!();
        
        if let Some(path) = config_path {
            println!("   File: {}", path);
        } else {
            println!("   File: zerodha_credentials.toml");
        }
        
        println!();
        println!("   [api]");
        println!("   api_key = \"{}\"", self.session.api_key);
        println!("   api_secret = \"{}\"", self.session.api_secret);
        println!("   access_token = \"{}\"", access_token);
        println!();
        println!("⚠️  IMPORTANT: Keep your credentials secure!");
        println!("⚠️  Never commit credential files to version control!");
        
        Ok(())
    }

    /// Get current session
    pub fn session(&self) -> &LoginSession {
        &self.session
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_login_session_creation() {
        let session = LoginSession::new(
            "test_api_key".to_string(),
            "test_api_secret".to_string(),
        );
        
        assert_eq!(session.api_key, "test_api_key");
        assert_eq!(session.api_secret, "test_api_secret");
        assert!(session.login_url.contains("test_api_key"));
        assert!(!session.is_authenticated());
    }

    #[test]
    fn test_access_token_generator_creation() {
        let generator = AccessTokenGenerator::new(
            "test_key".to_string(),
            "test_secret".to_string(),
        );
        
        assert_eq!(generator.session.api_key, "test_key");
        assert_eq!(generator.session.api_secret, "test_secret");
    }
}