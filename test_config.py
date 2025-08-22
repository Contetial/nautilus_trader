#!/usr/bin/env python3
"""Simple configuration test script for Zerodha without Unicode issues."""

import sys
import os
from pathlib import Path

# Add the adapters path
sys.path.insert(0, str(Path(__file__).parent / "nautilus_trader" / "adapters" / "zerodha"))

def test_config():
    """Test the Zerodha configuration system."""
    print("=" * 50)
    print("ZERODHA CONFIGURATION TEST")
    print("=" * 50)
    
    try:
        # Import the config loader
        from config_loader import ZerodhaCredentialsConfig
        
        print("1. Loading configuration...")
        config = ZerodhaCredentialsConfig.load()
        
        print("2. Configuration loaded successfully!")
        print("   Trading Mode:", config.settings.trading_mode)
        print("   Sandbox Mode:", config.settings.sandbox)
        print("   API Key:", config.api.api_key[:8] + "***")
        print("   API Secret:", config.api.api_secret[:8] + "***")
        
        # Check access token
        if config.api.access_token and config.api.access_token != "your_access_token_here":
            print("   Access Token: PRESENT (" + config.api.access_token[:8] + "***)")
            print("   Status: READY FOR TESTING")
        else:
            print("   Access Token: MISSING")
            print("   Status: NEED TO GENERATE ACCESS TOKEN")
            print("\n3. Next Step: Generate Access Token")
            print("   You need to generate an access token to complete the setup.")
            print("   This requires the interactive Kite Connect login flow.")
            print("\n   Steps to generate access token:")
            print("   a) Go to: https://kite.zerodha.com/connect/login?v=3&api_key=" + config.api.api_key)
            print("   b) Log in with your Zerodha credentials")
            print("   c) Authorize the application")
            print("   d) Copy the 'request_token' from the redirect URL")
            print("   e) Use our token exchange system")
            
        print("\n4. Configuration Validation:")
        try:
            config.validate()
            print("   Validation: PASSED")
        except Exception as e:
            print("   Validation: FAILED -", str(e))
            
        print("\n5. Paper Trading Settings:")
        print("   Initial Balance: Rs.", config.paper_trading.initial_balance)
        print("   Commission per Trade: Rs.", config.paper_trading.commission_per_trade)
        print("   Execution Delay: {}ms".format(config.paper_trading.execution_delay_ms))
        
        return True
        
    except ImportError as e:
        print("ERROR: Failed to import config loader:", str(e))
        return False
    except Exception as e:
        print("ERROR: Configuration test failed:", str(e))
        return False

def generate_access_token():
    """Guide user through access token generation."""
    print("\n" + "=" * 50)
    print("ACCESS TOKEN GENERATION GUIDE")
    print("=" * 50)
    
    try:
        from config_loader import ZerodhaCredentialsConfig
        config = ZerodhaCredentialsConfig.load()
        
        api_key = config.api.api_key
        api_secret = config.api.api_secret
        
        print("1. API Credentials Found:")
        print("   API Key:", api_key[:8] + "***")
        print("   API Secret:", api_secret[:8] + "***")
        
        print("\n2. Login URL:")
        login_url = f"https://kite.zerodha.com/connect/login?v=3&api_key={api_key}"
        print("   " + login_url)
        
        print("\n3. Manual Steps:")
        print("   a) Open the URL above in your browser")
        print("   b) Log in with your Zerodha credentials") 
        print("   c) Authorize the application")
        print("   d) You'll be redirected to a URL like:")
        print("      http://127.0.0.1/?request_token=XXXXXX&action=login&status=success")
        print("   e) Copy the 'request_token' value from the URL")
        
        print("\n4. Token Exchange:")
        print("   Once you have the request_token, we can exchange it for an access_token.")
        print("   This would normally be done programmatically, but since we can't run")
        print("   the Rust binary in this environment, you'll need to:")
        print("   - Use an external tool or")
        print("   - Manually make the API call to exchange the token")
        
        print("\n5. API Call Details for Manual Exchange:")
        print("   POST https://api.kite.trade/session/token")
        print("   Headers: X-Kite-Version: 3")
        print("   Form Data:")
        print("     api_key =", api_key)
        print("     request_token = [YOUR_REQUEST_TOKEN]")
        print("     checksum = [SHA256 of: api_key + request_token + api_secret]")
        
        return True
        
    except Exception as e:
        print("ERROR: Failed to generate access token guide:", str(e))
        return False

if __name__ == "__main__":
    print("Starting Zerodha configuration test...")
    
    # Test configuration
    config_success = test_config()
    
    if config_success:
        print("\nConfiguration test completed successfully!")
        
        # Show token generation guide
        generate_access_token()
    else:
        print("\nConfiguration test failed!")
        sys.exit(1)