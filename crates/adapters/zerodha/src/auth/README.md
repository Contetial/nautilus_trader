# Zerodha Authentication Module

This module provides comprehensive authentication support for the Zerodha Kite Connect API, including programmatic access token generation.

## 🔐 Authentication Flow

Zerodha Kite Connect uses a 3-step OAuth-like authentication flow:

```
1. Login URL → 2. Request Token → 3. Access Token
```

### Step 1: Login URL Generation
```rust
use nautilus_zerodha::auth::generate_login_url;

let login_url = generate_login_url("your_api_key", None);
// Opens: https://kite.zerodha.com/connect/login?v=3&api_key=your_api_key
```

### Step 2: Manual Login & Request Token
1. User opens the login URL in browser
2. Logs in to Zerodha account
3. Authorizes the application
4. Gets redirected with `request_token` in URL parameters

### Step 3: Access Token Exchange
```rust
use nautilus_zerodha::auth::exchange_request_token;

let response = exchange_request_token(
    "your_api_key",
    "your_api_secret", 
    "request_token_from_step_2"
).await?;

let access_token = response.data.access_token;
```

## 🚀 Quick Start

### Interactive Token Generation

Use the token generator binary for the easiest experience:

```bash
# Interactive mode (guides you through the process)
cargo run --bin zerodha-token-generator

# Direct mode (if you already have a request token)
cargo run --bin zerodha-token-generator -- --request-token YOUR_REQUEST_TOKEN
```

### Programmatic Usage

```rust
use nautilus_zerodha::auth::AccessTokenGenerator;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut generator = AccessTokenGenerator::new(
        "your_api_key".to_string(),
        "your_api_secret".to_string()
    );
    
    // Interactive flow
    let session_token = generator.generate_token_interactive().await?;
    println!("Access token: {}", session_token.access_token);
    
    Ok(())
}
```

## 📋 Components

### `AccessTokenGenerator`
Main class for generating access tokens with interactive user guidance.

**Methods:**
- `new(api_key, api_secret)` - Create generator instance
- `generate_token_interactive()` - Complete interactive flow
- `generate_token_with_request_token(token)` - Direct exchange
- `save_to_config(path)` - Save token to configuration

### `LoginSession`
Tracks authentication state throughout the flow.

**Properties:**
- `api_key` - Kite Connect API key
- `api_secret` - Kite Connect API secret  
- `login_url` - Generated login URL
- `request_token` - Token from login redirect
- `access_token` - Final access token

### `SessionToken`
Contains complete session information after successful authentication.

**Properties:**
- `access_token` - The main token for API calls
- `user_id` - Zerodha user ID
- `user_name` - User's display name
- `broker` - Broker identifier
- `email` - User's email address
- `exchanges` - Enabled exchanges (NSE, BSE, etc.)
- `products` - Enabled products (MIS, CNC, NRML)
- `order_types` - Enabled order types

## 🔧 Utility Functions

### `generate_login_url(api_key, redirect_url)`
Creates the login URL for manual authentication.

### `exchange_request_token(api_key, api_secret, request_token)`
Exchanges request token for access token.

### `generate_checksum(api_key, request_token, api_secret)`
Generates SHA-256 checksum for token exchange validation.

### `logout(api_key, access_token)`
Invalidates the access token (logout).

## 🧪 Testing

```bash
# Test authentication module
cargo test auth

# Test token generation (requires real credentials)
cargo run --bin zerodha-token-generator
```

## 🔒 Security Notes

1. **Never commit API secrets or access tokens to version control**
2. **Access tokens expire daily** - regenerate as needed
3. **Request tokens are single-use** - can't be reused
4. **Use HTTPS only** for all API communication
5. **Store credentials securely** using the configuration system

## 📖 API Reference

### Kite Connect Documentation
- [Authentication Flow](https://kite.trade/docs/connect/v3/user/)
- [Session API](https://kite.trade/docs/connect/v3/session/)
- [API Endpoints](https://kite.trade/docs/connect/v3/)

### Error Codes
- `403` - Invalid API key/secret
- `400` - Invalid request token
- `429` - Rate limit exceeded
- `500` - Server error

## 🛠️ Configuration Integration

The authentication module integrates with the configuration system:

```toml
[api]
api_key = "generated_by_kite_connect"
api_secret = "generated_by_kite_connect"  
access_token = "generated_by_this_module"
```

Use `cargo run --bin zerodha-token-generator` to populate the access token automatically.

## 📝 Example Output

```
🚀 Zerodha Access Token Generator
================================

🔐 ZERODHA KITE CONNECT AUTHENTICATION
=====================================

To generate your access token, please follow these steps:

1. 🌐 Open this URL in your browser:
   https://kite.zerodha.com/connect/login?v=3&api_key=your_key

2. 🔑 Log in to your Zerodha account

3. ✅ Authorize the application

4. 📋 Copy the 'request_token' from the redirect URL

5. 📝 Paste the request_token below

Enter request_token: abc123def456...

🔄 Exchanging request token for access token...
✅ Access token generated successfully!
📋 User: John Doe (ZD1234)
🏦 Broker: ZERODHA
📧 Email: john@example.com
🔑 Access Token: xyz789ab***

💾 ACCESS TOKEN GENERATED
========================

🔑 Your access token: xyz789abc123def456...

📝 To save this token, add it to your configuration file:

   File: zerodha_credentials.toml

   [api]
   api_key = "your_api_key"
   api_secret = "your_api_secret"
   access_token = "xyz789abc123def456..."

⚠️  IMPORTANT: Keep your credentials secure!
⚠️  Never commit credential files to version control!
```

## 🚨 Common Issues

### "Invalid request token"
- Make sure you copied the complete token from the redirect URL
- Request tokens are single-use - generate a new one if needed
- Check that there are no extra spaces or characters

### "Invalid API key/secret"
- Verify credentials from your Kite Connect app dashboard
- Ensure the app is active and not suspended
- Check for typos in the key/secret

### "Authentication failed"
- Request tokens expire quickly - don't delay the exchange
- Make sure you're using the correct API version (v3)
- Check your network connection

### "Access token expired"
- Access tokens expire daily around 7:30 AM IST
- Regenerate tokens regularly or implement auto-refresh
- Use the token generator to get a fresh token

## 📅 Token Lifecycle

- **API Key/Secret**: Permanent (until app is deleted)
- **Request Token**: Single-use, expires in ~10 minutes
- **Access Token**: Expires daily around 7:30 AM IST

Plan your token refresh strategy accordingly!