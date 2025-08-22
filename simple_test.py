#!/usr/bin/env python3
"""Minimal test for configuration loading."""

import sys
import os
from pathlib import Path

# Add the adapters path
sys.path.insert(0, str(Path(__file__).parent / "nautilus_trader" / "adapters" / "zerodha"))

def main():
    print("Starting basic configuration test...")
    
    try:
        from config_loader import ZerodhaCredentialsConfig
        print("Config loader imported successfully")
        
        config = ZerodhaCredentialsConfig.load()
        print("Configuration loaded successfully")
        
        print("API Key: " + config.api.api_key[:8] + "***")
        print("API Secret: " + config.api.api_secret[:8] + "***")
        print("Trading Mode: " + config.settings.trading_mode)
        
        if config.api.access_token == "your_access_token_here":
            print("Access Token: NOT SET - needs generation")
        else:
            print("Access Token: " + config.api.access_token[:8] + "***")
            
        print("Configuration test completed successfully!")
        
    except Exception as e:
        print("ERROR: " + str(e))
        
if __name__ == "__main__":
    main()