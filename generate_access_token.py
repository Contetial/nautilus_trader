#!/usr/bin/env python3
"""Python implementation of Zerodha access token generator."""

import sys
import hashlib
import requests
from pathlib import Path

# Add the adapters path
sys.path.insert(0, str(Path(__file__).parent / "nautilus_trader" / "adapters" / "zerodha"))

def generate_checksum(api_key, request_token, api_secret):
    """Generate SHA-256 checksum for token exchange."""
    data = api_key + request_token + api_secret
    return hashlib.sha256(data.encode()).hexdigest()

def exchange_request_token(api_key, api_secret, request_token):
    """Exchange request token for access token."""
    print("Exchanging request token for access token...")
    
    checksum = generate_checksum(api_key, request_token, api_secret)
    
    params = {
        "api_key": api_key,
        "request_token": request_token,
        "checksum": checksum
    }
    
    headers = {
        "X-Kite-Version": "3"
    }
    
    try:
        response = requests.post(
            "https://api.kite.trade/session/token",
            data=params,
            headers=headers,
            timeout=30
        )
        
        if response.status_code == 200:
            data = response.json()
            print("SUCCESS! Access token generated successfully!")
            print()
            print("User Information:")
            print(f"  User ID: {data['data']['user_id']}")
            print(f"  User Name: {data['data']['user_name']}")
            print(f"  Email: {data['data']['email']}")
            print(f"  Broker: {data['data']['broker']}")
            print()
            print(f"Access Token: {data['data']['access_token']}")
            return data['data']['access_token']
        else:
            print(f"ERROR: HTTP {response.status_code} - {response.text}")
            return None
            
    except Exception as e:
        print(f"ERROR: Failed to exchange token: {e}")
        return None

def update_config_with_token(access_token):
    """Update the configuration file with the new access token."""
    config_path = Path("config/zerodha_credentials.toml")
    
    if not config_path.exists():
        print(f"ERROR: Configuration file not found: {config_path}")
        return False
    
    try:
        # Read the current config
        with open(config_path, 'r') as f:
            content = f.read()
        
        # Replace the access token line
        lines = content.split('\n')
        for i, line in enumerate(lines):
            if line.startswith('access_token = '):
                lines[i] = f'access_token = "{access_token}"'
                break
        
        # Write back the updated config
        with open(config_path, 'w') as f:
            f.write('\n'.join(lines))
        
        print(f"Configuration updated: {config_path}")
        return True
        
    except Exception as e:
        print(f"ERROR: Failed to update config: {e}")
        return False

def main():
    print("=" * 50)
    print("ZERODHA ACCESS TOKEN GENERATOR")
    print("=" * 50)
    
    try:
        # Load configuration
        from config_loader import ZerodhaCredentialsConfig
        config = ZerodhaCredentialsConfig.load()
        
        api_key = config.api.api_key
        api_secret = config.api.api_secret
        
        print("1. API Credentials Found:")
        print(f"   API Key: {api_key[:8]}***")
        print(f"   API Secret: {api_secret[:8]}***")
        print()
        
        # Generate login URL
        login_url = f"https://kite.zerodha.com/connect/login?v=3&api_key={api_key}"
        print("2. MANUAL STEPS REQUIRED:")
        print("   a) Open this URL in your browser:")
        print(f"      {login_url}")
        print()
        print("   b) Log in with your Zerodha credentials")
        print("   c) Authorize the application")
        print("   d) You will be redirected to a URL like:")
        print("      http://127.0.0.1/?request_token=XXXXXXXX&action=login&status=success")
        print("   e) Copy the 'request_token' value from that URL")
        print()
        
        # Get request token from user
        request_token = input("3. Enter the request_token from the redirect URL: ").strip()
        
        if not request_token:
            print("ERROR: Request token cannot be empty")
            return False
        
        # Validate request token format (typically 32 characters)
        if len(request_token) != 32 or not request_token.isalnum():
            print("WARNING: Request token format looks unusual")
            print("Expected: 32 alphanumeric characters")
            print(f"Got: {len(request_token)} characters")
            
            proceed = input("Continue anyway? (y/N): ").strip().lower()
            if not proceed.startswith('y'):
                print("Aborted by user")
                return False
        
        print()
        print("4. TOKEN EXCHANGE:")
        
        # Exchange for access token
        access_token = exchange_request_token(api_key, api_secret, request_token)
        
        if access_token:
            print()
            print("5. SAVING TO CONFIGURATION:")
            if update_config_with_token(access_token):
                print("SUCCESS! Your configuration has been updated with the new access token.")
                print()
                print("6. NEXT STEPS:")
                print("   - Your access token expires daily around 7:30 AM IST")
                print("   - You can now test the full system integration")
                print("   - Run configuration tests to verify everything works")
                return True
            else:
                print("WARNING: Failed to update configuration file")
                print("Please manually add this access token to your config:")
                print(f'access_token = "{access_token}"')
        else:
            print("FAILED to generate access token")
            return False
            
    except Exception as e:
        print(f"ERROR: {e}")
        return False

if __name__ == "__main__":
    success = main()
    if not success:
        sys.exit(1)