#!/usr/bin/env python3
"""Exchange the request token for access token."""

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

def main():
    print("=" * 50)
    print("EXCHANGING REQUEST TOKEN FOR ACCESS TOKEN")
    print("=" * 50)
    
    # Load configuration
    from config_loader import ZerodhaCredentialsConfig
    config = ZerodhaCredentialsConfig.load()
    
    api_key = config.api.api_key
    api_secret = config.api.api_secret
    request_token = "gU4GypuI5Tm6uSOuorzlVKAXXjFamZgn"
    
    print("1. Credentials:")
    print(f"   API Key: {api_key[:8]}***")
    print(f"   API Secret: {api_secret[:8]}***")
    print(f"   Request Token: {request_token[:8]}***")
    print()
    
    print("2. Generating checksum...")
    checksum = generate_checksum(api_key, request_token, api_secret)
    print(f"   Checksum: {checksum[:16]}***")
    print()
    
    print("3. Exchanging token with Kite Connect API...")
    
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
        
        print(f"   Response Status: {response.status_code}")
        
        if response.status_code == 200:
            data = response.json()
            print("   SUCCESS! Token exchange completed.")
            print()
            
            print("4. User Information:")
            user_data = data['data']
            print(f"   User ID: {user_data['user_id']}")
            print(f"   User Name: {user_data['user_name']}")
            print(f"   Email: {user_data['email']}")
            print(f"   Broker: {user_data['broker']}")
            print(f"   Exchanges: {', '.join(user_data['exchanges'])}")
            print(f"   Products: {', '.join(user_data['products'])}")
            print()
            
            access_token = user_data['access_token']
            print(f"5. Access Token Generated: {access_token[:8]}***")
            print()
            
            # Update configuration file
            print("6. Updating configuration file...")
            config_path = Path("config/zerodha_credentials.toml")
            
            # Read current config
            with open(config_path, 'r') as f:
                content = f.read()
            
            # Replace access token line
            lines = content.split('\n')
            for i, line in enumerate(lines):
                if line.strip().startswith('access_token = '):
                    lines[i] = f'access_token = "{access_token}"'
                    break
            
            # Write updated config
            with open(config_path, 'w') as f:
                f.write('\n'.join(lines))
            
            print(f"   Configuration updated: {config_path}")
            print()
            
            print("7. AUTHENTICATION COMPLETE!")
            print("   - Access token has been saved to your configuration")
            print("   - Token expires daily around 7:30 AM IST")
            print("   - You can now run live tests and trading operations")
            print("   - Paper trading mode is still enabled for safety")
            
            return True
            
        else:
            print(f"   ERROR: HTTP {response.status_code}")
            print(f"   Response: {response.text}")
            return False
            
    except Exception as e:
        print(f"   ERROR: {e}")
        return False

if __name__ == "__main__":
    success = main()
    if success:
        print("\nSUCCESS: Authentication system fully operational!")
    else:
        print("\nFAILED: Token exchange unsuccessful")
        sys.exit(1)