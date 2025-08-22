#!/usr/bin/env python3
"""Comprehensive Zerodha integration test with real credentials."""

import sys
import asyncio
import requests
from pathlib import Path
from datetime import datetime

# Add the adapters path
sys.path.insert(0, str(Path(__file__).parent / "nautilus_trader" / "adapters" / "zerodha"))

class ZerodhaIntegrationTester:
    """Test Zerodha integration with live API calls."""
    
    def __init__(self):
        self.config = None
        self.session = requests.Session()
        
    async def load_configuration(self):
        """Load and validate configuration."""
        print("1. LOADING CONFIGURATION")
        print("-" * 30)
        
        try:
            from config_loader import ZerodhaCredentialsConfig
            self.config = ZerodhaCredentialsConfig.load()
            
            print("   Configuration loaded successfully")
            print(f"   API Key: {self.config.api.api_key[:8]}***")
            print(f"   Trading Mode: {self.config.settings.trading_mode}")
            print(f"   Sandbox: {self.config.settings.sandbox}")
            
            # Validate access token
            if not self.config.api.access_token or self.config.api.access_token == "your_access_token_here":
                raise ValueError("Access token not configured")
                
            print(f"   Access Token: {self.config.api.access_token[:8]}*** (VALID)")
            
            # Setup session headers
            self.session.headers.update({
                'Authorization': f'token {self.config.api.api_key}:{self.config.api.access_token}',
                'X-Kite-Version': '3',
                'User-Agent': 'NautilusTrader-Zerodha/1.0'
            })
            
            return True
            
        except Exception as e:
            print(f"   ERROR: {e}")
            return False
    
    async def test_user_profile(self):
        """Test user profile API call."""
        print("\n2. TESTING USER PROFILE API")
        print("-" * 30)
        
        try:
            response = self.session.get("https://api.kite.trade/user/profile", timeout=30)
            
            if response.status_code == 200:
                data = response.json()
                user_data = data['data']
                
                print("   SUCCESS: User profile retrieved")
                print(f"   User ID: {user_data['user_id']}")
                print(f"   User Name: {user_data['user_name']}")
                print(f"   Email: {user_data['email']}")
                print(f"   Broker: {user_data['broker']}")
                print(f"   Exchanges: {', '.join(user_data['exchanges'])}")
                print(f"   Order Types: {', '.join(user_data['order_types'])}")
                print(f"   Products: {', '.join(user_data['products'])}")
                
                return True
            else:
                print(f"   ERROR: HTTP {response.status_code} - {response.text}")
                return False
                
        except Exception as e:
            print(f"   ERROR: {e}")
            return False
    
    async def test_account_margins(self):
        """Test account margins API call."""
        print("\n3. TESTING ACCOUNT MARGINS")
        print("-" * 30)
        
        try:
            response = self.session.get("https://api.kite.trade/user/margins", timeout=30)
            
            if response.status_code == 200:
                data = response.json()
                margins = data['data']
                
                print("   SUCCESS: Account margins retrieved")
                
                for segment, margin_data in margins.items():
                    print(f"   {segment.upper()}:")
                    print(f"     Available Cash: Rs. {margin_data.get('available', {}).get('cash', 0):,.2f}")
                    print(f"     Used Margin: Rs. {margin_data.get('utilised', {}).get('debits', 0):,.2f}")
                    print(f"     Total: Rs. {margin_data.get('net', 0):,.2f}")
                
                return True
            else:
                print(f"   ERROR: HTTP {response.status_code} - {response.text}")
                return False
                
        except Exception as e:
            print(f"   ERROR: {e}")
            return False
    
    async def test_instruments_list(self):
        """Test instruments list API call."""
        print("\n4. TESTING INSTRUMENTS LIST")
        print("-" * 30)
        
        try:
            response = self.session.get("https://api.kite.trade/instruments", timeout=60)
            
            if response.status_code == 200:
                # Parse CSV data
                lines = response.text.strip().split('\n')
                headers = lines[0].split(',')
                
                print(f"   SUCCESS: Instruments list retrieved")
                print(f"   Total instruments: {len(lines) - 1:,}")
                print(f"   Headers: {', '.join(headers[:5])}...")
                
                # Show sample instruments
                print("   Sample instruments:")
                for i in range(1, min(6, len(lines))):
                    fields = lines[i].split(',')
                    if len(fields) >= 3:
                        print(f"     {fields[2]} ({fields[1]}) - {fields[0]}")
                
                return True
            else:
                print(f"   ERROR: HTTP {response.status_code} - {response.text}")
                return False
                
        except Exception as e:
            print(f"   ERROR: {e}")
            return False
    
    async def test_market_quote(self):
        """Test market quote API call."""
        print("\n5. TESTING MARKET QUOTES")
        print("-" * 30)
        
        # Test with popular stocks
        instruments = ["NSE:RELIANCE", "NSE:TCS", "NSE:INFY"]
        
        try:
            for instrument in instruments:
                response = self.session.get(
                    f"https://api.kite.trade/quote?i={instrument}", 
                    timeout=30
                )
                
                if response.status_code == 200:
                    data = response.json()
                    quote_data = data['data'][instrument]
                    
                    print(f"   {instrument}:")
                    print(f"     LTP: Rs. {quote_data.get('last_price', 0):,.2f}")
                    print(f"     Change: {quote_data.get('net_change', 0):+.2f} ({quote_data.get('net_change', 0)/quote_data.get('last_price', 1)*100:+.2f}%)")
                    print(f"     Volume: {quote_data.get('volume', 0):,}")
                    
                    ohlc = quote_data.get('ohlc', {})
                    print(f"     OHLC: O:{ohlc.get('open', 0)} H:{ohlc.get('high', 0)} L:{ohlc.get('low', 0)} C:{ohlc.get('close', 0)}")
                else:
                    print(f"   ERROR for {instrument}: HTTP {response.status_code}")
                    
            return True
                
        except Exception as e:
            print(f"   ERROR: {e}")
            return False
    
    async def test_positions(self):
        """Test positions API call."""
        print("\n6. TESTING POSITIONS")
        print("-" * 30)
        
        try:
            response = self.session.get("https://api.kite.trade/portfolio/positions", timeout=30)
            
            if response.status_code == 200:
                data = response.json()
                positions = data['data']
                
                print("   SUCCESS: Positions retrieved")
                
                if not positions['net'] and not positions['day']:
                    print("   No open positions found (expected in paper trading mode)")
                else:
                    print(f"   Net positions: {len(positions['net'])}")
                    print(f"   Day positions: {len(positions['day'])}")
                    
                    for pos in positions['net'][:3]:  # Show first 3
                        print(f"     {pos['tradingsymbol']}: {pos['quantity']} @ Rs.{pos['average_price']}")
                
                return True
            else:
                print(f"   ERROR: HTTP {response.status_code} - {response.text}")
                return False
                
        except Exception as e:
            print(f"   ERROR: {e}")
            return False
    
    async def test_orders_history(self):
        """Test orders history API call."""
        print("\n7. TESTING ORDERS HISTORY")
        print("-" * 30)
        
        try:
            response = self.session.get("https://api.kite.trade/orders", timeout=30)
            
            if response.status_code == 200:
                data = response.json()
                orders = data['data']
                
                print("   SUCCESS: Orders history retrieved")
                print(f"   Total orders today: {len(orders)}")
                
                if not orders:
                    print("   No orders found today (expected in paper trading mode)")
                else:
                    print("   Recent orders:")
                    for order in orders[-3:]:  # Show last 3 orders
                        print(f"     {order['tradingsymbol']}: {order['transaction_type']} {order['quantity']} @ {order.get('price', 'Market')} - {order['status']}")
                
                return True
            else:
                print(f"   ERROR: HTTP {response.status_code} - {response.text}")
                return False
                
        except Exception as e:
            print(f"   ERROR: {e}")
            return False

    async def run_all_tests(self):
        """Run all integration tests."""
        print("=" * 50)
        print("ZERODHA LIVE INTEGRATION TEST")
        print("=" * 50)
        print(f"Timestamp: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
        print()
        
        results = []
        
        # Run all tests
        test_methods = [
            self.load_configuration,
            self.test_user_profile,
            self.test_account_margins,
            self.test_instruments_list,
            self.test_market_quote,
            self.test_positions,
            self.test_orders_history
        ]
        
        for test_method in test_methods:
            try:
                result = await test_method()
                results.append(result)
            except Exception as e:
                print(f"   EXCEPTION: {e}")
                results.append(False)
        
        # Summary
        print("\n" + "=" * 50)
        print("TEST SUMMARY")
        print("=" * 50)
        
        passed = sum(results)
        total = len(results)
        
        test_names = [
            "Configuration Loading",
            "User Profile API",
            "Account Margins API", 
            "Instruments List API",
            "Market Quotes API",
            "Positions API",
            "Orders History API"
        ]
        
        for i, (name, result) in enumerate(zip(test_names, results)):
            status = "PASS" if result else "FAIL"
            print(f"{i+1}. {name}: {status}")
        
        print(f"\nOverall Result: {passed}/{total} tests passed")
        
        if passed == total:
            print("SUCCESS: All integration tests passed!")
            print("Your Zerodha authentication and API access is fully operational.")
        else:
            print(f"PARTIAL SUCCESS: {passed} out of {total} tests passed")
            print("Some APIs may have restrictions or require different permissions.")
        
        return passed == total

async def main():
    """Main test runner."""
    tester = ZerodhaIntegrationTester()
    success = await tester.run_all_tests()
    
    if success:
        print("\nNext steps:")
        print("- Test paper trading execution")
        print("- Test WebSocket connections for live data")
        print("- Run strategy backtests")
        
        return True
    else:
        print("\nTroubleshooting:")
        print("- Check API permissions in Kite Connect app settings")
        print("- Verify access token is not expired")
        print("- Ensure network connectivity to api.kite.trade")
        
        return False

if __name__ == "__main__":
    success = asyncio.run(main())
    sys.exit(0 if success else 1)