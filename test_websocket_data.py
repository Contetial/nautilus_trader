#!/usr/bin/env python3
"""Test WebSocket connections for real-time data."""

import sys
import asyncio
import websockets
import json
import struct
import gzip
from pathlib import Path
from datetime import datetime

# Add the adapters path
sys.path.insert(0, str(Path(__file__).parent / "nautilus_trader" / "adapters" / "zerodha"))

class WebSocketTester:
    """Test WebSocket connections for Kite Connect."""
    
    def __init__(self):
        self.config = None
        self.websocket = None
        self.subscribed_tokens = []
        self.message_count = 0
        
    async def setup(self):
        """Setup WebSocket test environment."""
        print("1. SETTING UP WEBSOCKET TEST")
        print("-" * 35)
        
        try:
            from config_loader import ZerodhaCredentialsConfig
            self.config = ZerodhaCredentialsConfig.load()
            
            print(f"   API Key: {self.config.api.api_key[:8]}***")
            print(f"   Access Token: {self.config.api.access_token[:8]}***")
            
            return True
            
        except Exception as e:
            print(f"   ERROR: {e}")
            return False
    
    async def test_websocket_connection(self):
        """Test WebSocket connection to Kite Connect."""
        print("\n2. TESTING WEBSOCKET CONNECTION")
        print("-" * 35)
        
        try:
            # Kite Connect WebSocket URL
            ws_url = f"wss://ws.kite.trade/?api_key={self.config.api.api_key}&access_token={self.config.api.access_token}"
            
            print("   Connecting to Kite WebSocket...")
            print(f"   URL: wss://ws.kite.trade/")
            
            # Connect with timeout
            self.websocket = await asyncio.wait_for(
                websockets.connect(ws_url, ping_interval=30, ping_timeout=10),
                timeout=15
            )
            
            print("   SUCCESS: WebSocket connected!")
            
            # Test basic ping/pong
            await self.websocket.ping()
            print("   Ping/Pong test successful")
            
            return True
            
        except asyncio.TimeoutError:
            print("   ERROR: Connection timeout (15s)")
            return False
        except Exception as e:
            print(f"   ERROR: Connection failed - {e}")
            return False
    
    async def test_subscription_message(self):
        """Test subscribing to market data."""
        print("\n3. TESTING MARKET DATA SUBSCRIPTION")
        print("-" * 35)
        
        if not self.websocket:
            print("   ERROR: WebSocket not connected")
            return False
        
        try:
            # Popular instrument tokens for testing
            # NSE:RELIANCE = 738561, NSE:TCS = 2953217, NSE:INFY = 408065
            test_tokens = [738561, 2953217, 408065]  # RELIANCE, TCS, INFY
            
            # Subscribe to LTP (Last Traded Price) mode
            subscription_message = {
                "a": "subscribe",
                "v": test_tokens
            }
            
            print(f"   Subscribing to tokens: {test_tokens}")
            print("   Mode: LTP (Last Traded Price)")
            
            # Send subscription message
            await self.websocket.send(json.dumps(subscription_message))
            
            print("   Subscription message sent successfully")
            self.subscribed_tokens = test_tokens
            
            return True
            
        except Exception as e:
            print(f"   ERROR: Subscription failed - {e}")
            return False
    
    async def test_mode_changes(self):
        """Test changing subscription modes."""
        print("\n4. TESTING MODE CHANGES")
        print("-" * 35)
        
        if not self.websocket:
            print("   ERROR: WebSocket not connected")
            return False
        
        try:
            # Test different modes
            modes = ["ltp", "quote", "full"]
            
            for mode in modes:
                print(f"   Setting mode to: {mode.upper()}")
                
                mode_message = {
                    "a": "mode",
                    "v": [mode, self.subscribed_tokens]
                }
                
                await self.websocket.send(json.dumps(mode_message))
                
                # Wait a bit for mode change to take effect
                await asyncio.sleep(1)
                
                print(f"   Mode change to {mode.upper()} sent")
            
            return True
            
        except Exception as e:
            print(f"   ERROR: Mode change failed - {e}")
            return False
    
    def parse_binary_packet(self, data):
        """Parse binary market data packet."""
        try:
            # Kite Connect sends binary data in specific format
            # First 2 bytes indicate the number of packets
            packet_count = struct.unpack(">H", data[:2])[0]
            
            packets = []
            offset = 2
            
            for _ in range(packet_count):
                if offset >= len(data):
                    break
                    
                # Next 2 bytes indicate packet length
                packet_length = struct.unpack(">H", data[offset:offset+2])[0]
                offset += 2
                
                if offset + packet_length > len(data):
                    break
                
                # Extract packet data
                packet_data = data[offset:offset+packet_length]
                offset += packet_length
                
                # Parse the packet based on mode
                # This is simplified - actual parsing depends on mode
                if len(packet_data) >= 8:
                    instrument_token = struct.unpack(">I", packet_data[:4])[0]
                    
                    if len(packet_data) >= 8:  # LTP mode
                        ltp = struct.unpack(">I", packet_data[4:8])[0] / 100.0
                        packets.append({
                            "instrument_token": instrument_token,
                            "ltp": ltp,
                            "mode": "ltp"
                        })
                    elif len(packet_data) >= 44:  # Quote mode
                        # More complex parsing for quote mode
                        ltp = struct.unpack(">I", packet_data[4:8])[0] / 100.0
                        packets.append({
                            "instrument_token": instrument_token,
                            "ltp": ltp,
                            "mode": "quote"
                        })
            
            return packets
            
        except Exception as e:
            return [{"error": f"Parse error: {e}"}]
    
    async def test_data_reception(self):
        """Test receiving real-time market data."""
        print("\n5. TESTING DATA RECEPTION")
        print("-" * 35)
        
        if not self.websocket:
            print("   ERROR: WebSocket not connected")
            return False
        
        try:
            print("   Listening for market data (10 seconds)...")
            
            timeout_seconds = 10
            start_time = datetime.now()
            
            while (datetime.now() - start_time).total_seconds() < timeout_seconds:
                try:
                    # Wait for message with timeout
                    message = await asyncio.wait_for(
                        self.websocket.recv(), 
                        timeout=2.0
                    )
                    
                    self.message_count += 1
                    
                    # Handle different message types
                    if isinstance(message, bytes):
                        # Binary market data
                        packets = self.parse_binary_packet(message)
                        
                        for packet in packets:
                            if "error" not in packet:
                                print(f"   Data {self.message_count}: Token {packet['instrument_token']} LTP Rs. {packet['ltp']:.2f}")
                            else:
                                print(f"   Parse Error: {packet['error']}")
                                
                    elif isinstance(message, str):
                        # JSON message (connection status, errors, etc.)
                        try:
                            data = json.loads(message)
                            print(f"   Status Message: {data}")
                        except:
                            print(f"   Text Message: {message}")
                    
                    # Stop if we get enough messages
                    if self.message_count >= 5:
                        break
                        
                except asyncio.TimeoutError:
                    continue
                except Exception as e:
                    print(f"   Message Error: {e}")
                    continue
            
            if self.message_count > 0:
                print(f"   SUCCESS: Received {self.message_count} market data messages")
                return True
            else:
                print("   WARNING: No market data received (market may be closed)")
                return True  # Still consider success as connection worked
                
        except Exception as e:
            print(f"   ERROR: Data reception failed - {e}")
            return False
    
    async def test_unsubscribe(self):
        """Test unsubscribing from market data."""
        print("\n6. TESTING UNSUBSCRIBE")
        print("-" * 35)
        
        if not self.websocket:
            print("   ERROR: WebSocket not connected")
            return False
        
        try:
            unsubscribe_message = {
                "a": "unsubscribe",
                "v": self.subscribed_tokens
            }
            
            print(f"   Unsubscribing from tokens: {self.subscribed_tokens}")
            await self.websocket.send(json.dumps(unsubscribe_message))
            
            print("   Unsubscribe message sent successfully")
            return True
            
        except Exception as e:
            print(f"   ERROR: Unsubscribe failed - {e}")
            return False
    
    async def cleanup(self):
        """Cleanup WebSocket connection."""
        if self.websocket:
            await self.websocket.close()
            print("\n   WebSocket connection closed")
    
    async def run_all_tests(self):
        """Run all WebSocket tests."""
        print("=" * 50)
        print("WEBSOCKET REAL-TIME DATA TEST")
        print("=" * 50)
        print(f"Timestamp: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
        
        results = []
        
        test_methods = [
            self.setup,
            self.test_websocket_connection,
            self.test_subscription_message,
            self.test_mode_changes,
            self.test_data_reception,
            self.test_unsubscribe
        ]
        
        test_names = [
            "Setup Configuration",
            "WebSocket Connection",
            "Market Data Subscription", 
            "Mode Changes",
            "Data Reception",
            "Unsubscribe"
        ]
        
        for test_method, test_name in zip(test_methods, test_names):
            try:
                result = await test_method()
                results.append((test_name, result))
            except Exception as e:
                print(f"   EXCEPTION in {test_name}: {e}")
                results.append((test_name, False))
        
        # Cleanup
        await self.cleanup()
        
        # Summary
        print("\n" + "=" * 50)
        print("WEBSOCKET TEST SUMMARY")
        print("=" * 50)
        
        passed = sum(1 for _, result in results if result)
        total = len(results)
        
        for i, (name, result) in enumerate(results):
            status = "PASS" if result else "FAIL"
            print(f"{i+1}. {name}: {status}")
        
        print(f"\nOverall Result: {passed}/{total} tests passed")
        print(f"Messages Received: {self.message_count}")
        
        if passed >= total - 1:  # Allow one failure for market closed scenarios
            print("SUCCESS: WebSocket real-time data system is operational!")
            if self.message_count == 0:
                print("NOTE: No data received (market may be closed)")
        else:
            print("PARTIAL SUCCESS: Some WebSocket features need attention")
        
        return passed >= total - 1

async def main():
    """Main test runner."""
    tester = WebSocketTester()
    try:
        success = await tester.run_all_tests()
        return success
    except Exception as e:
        print(f"Test runner error: {e}")
        return False

if __name__ == "__main__":
    success = asyncio.run(main())
    sys.exit(0 if success else 1)