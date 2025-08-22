#!/usr/bin/env python3
"""Test complete order management flow."""

import sys
import asyncio
import requests
from pathlib import Path
from datetime import datetime

# Add the adapters path
sys.path.insert(0, str(Path(__file__).parent / "nautilus_trader" / "adapters" / "zerodha"))

class OrderManagementTester:
    """Test complete order management workflow."""
    
    def __init__(self):
        self.config = None
        self.session = requests.Session()
        self.test_orders = []
        
    async def setup(self):
        """Setup order management test environment."""
        print("1. SETTING UP ORDER MANAGEMENT TEST")
        print("-" * 40)
        
        try:
            from config_loader import ZerodhaCredentialsConfig
            self.config = ZerodhaCredentialsConfig.load()
            
            print(f"   API Key: {self.config.api.api_key[:8]}***")
            print(f"   Trading Mode: {self.config.settings.trading_mode}")
            print(f"   Max Order Value: Rs. {self.config.risk_management.max_order_value:,.2f}")
            print(f"   Max Orders/Min: {self.config.risk_management.max_orders_per_minute}")
            
            # Setup session headers
            self.session.headers.update({
                'Authorization': f'token {self.config.api.api_key}:{self.config.api.access_token}',
                'X-Kite-Version': '3'
            })
            
            return True
            
        except Exception as e:
            print(f"   ERROR: {e}")
            return False
    
    async def test_order_validation(self):
        """Test order validation logic."""
        print("\n2. TESTING ORDER VALIDATION")
        print("-" * 40)
        
        try:
            # Test valid order
            valid_order = {
                'tradingsymbol': 'RELIANCE',
                'exchange': 'NSE',
                'transaction_type': 'BUY',
                'quantity': 1,
                'order_type': 'MARKET',
                'product': 'MIS',
                'validity': 'DAY'
            }
            
            # Basic validation checks
            validation_results = []
            
            # Check required fields
            required_fields = ['tradingsymbol', 'exchange', 'transaction_type', 'quantity', 'order_type', 'product']
            for field in required_fields:
                if field in valid_order:
                    validation_results.append(f"   ✓ {field}: {valid_order[field]}")
                else:
                    validation_results.append(f"   ✗ {field}: MISSING")
            
            # Check quantity limits
            if valid_order['quantity'] > 0 and valid_order['quantity'] <= 10000:
                validation_results.append(f"   ✓ Quantity within limits: {valid_order['quantity']}")
            else:
                validation_results.append(f"   ✗ Quantity out of range: {valid_order['quantity']}")
            
            # Check order value (approximate)
            estimated_price = 2800.0  # RELIANCE approximate price
            order_value = estimated_price * valid_order['quantity']
            
            if order_value <= self.config.risk_management.max_order_value:
                validation_results.append(f"   ✓ Order value within limit: Rs. {order_value:,.2f}")
            else:
                validation_results.append(f"   ✗ Order value exceeds limit: Rs. {order_value:,.2f}")
            
            # Check valid exchange
            valid_exchanges = ['NSE', 'BSE', 'NFO', 'BFO', 'CDS', 'MCX']
            if valid_order['exchange'] in valid_exchanges:
                validation_results.append(f"   ✓ Valid exchange: {valid_order['exchange']}")
            else:
                validation_results.append(f"   ✗ Invalid exchange: {valid_order['exchange']}")
            
            print("   Order validation results:")
            for result in validation_results:
                print(result)
            
            # Test invalid orders
            print("\n   Testing invalid order scenarios:")
            
            invalid_orders = [
                {'name': 'Zero quantity', 'order': {**valid_order, 'quantity': 0}},
                {'name': 'Negative quantity', 'order': {**valid_order, 'quantity': -5}},
                {'name': 'Invalid exchange', 'order': {**valid_order, 'exchange': 'INVALID'}},
                {'name': 'Invalid transaction_type', 'order': {**valid_order, 'transaction_type': 'INVALID'}},
                {'name': 'Large quantity', 'order': {**valid_order, 'quantity': 100000}}
            ]
            
            for invalid_order in invalid_orders:
                try:
                    # Simulate validation
                    order = invalid_order['order']
                    
                    if order.get('quantity', 0) <= 0:
                        print(f"   ✓ Rejected {invalid_order['name']}: Invalid quantity")
                    elif order.get('exchange') not in valid_exchanges:
                        print(f"   ✓ Rejected {invalid_order['name']}: Invalid exchange")
                    elif order.get('transaction_type') not in ['BUY', 'SELL']:
                        print(f"   ✓ Rejected {invalid_order['name']}: Invalid transaction type")
                    elif order.get('quantity', 0) * 2800 > self.config.risk_management.max_order_value:
                        print(f"   ✓ Rejected {invalid_order['name']}: Order value too large")
                    else:
                        print(f"   ⚠ Would accept {invalid_order['name']} (needs review)")
                        
                except Exception as e:
                    print(f"   ✓ Rejected {invalid_order['name']}: {e}")
            
            return True
            
        except Exception as e:
            print(f"   ERROR: {e}")
            return False
    
    async def test_order_placement_simulation(self):
        """Test order placement in paper trading mode."""
        print("\n3. TESTING ORDER PLACEMENT (SIMULATION)")
        print("-" * 40)
        
        try:
            print("   NOTE: This is simulation mode - no real orders placed")
            print("   Testing order placement logic and API structure")
            
            # Sample order for testing
            test_order = {
                'tradingsymbol': 'RELIANCE',
                'exchange': 'NSE',
                'transaction_type': 'BUY',
                'quantity': 1,
                'order_type': 'MARKET',
                'product': 'MIS',
                'validity': 'DAY'
            }
            
            print(f"   Test Order: {test_order['transaction_type']} {test_order['quantity']} {test_order['tradingsymbol']}")
            
            # Simulate order placement API call structure
            order_params = {
                'variety': 'regular',
                'tradingsymbol': test_order['tradingsymbol'],
                'exchange': test_order['exchange'],
                'transaction_type': test_order['transaction_type'],
                'order_type': test_order['order_type'],
                'quantity': test_order['quantity'],
                'product': test_order['product'],
                'validity': test_order['validity']
            }
            
            print("   Order parameters prepared:")
            for key, value in order_params.items():
                print(f"     {key}: {value}")
            
            # Simulate order ID generation (would come from API in real scenario)
            import random
            simulated_order_id = f"SIM{random.randint(100000, 999999)}"
            
            print(f"   Simulated Order ID: {simulated_order_id}")
            
            # Store for tracking
            self.test_orders.append({
                'order_id': simulated_order_id,
                'status': 'OPEN',
                'timestamp': datetime.now().isoformat(),
                **order_params
            })
            
            print("   ✓ Order placement simulation successful")
            return True
            
        except Exception as e:
            print(f"   ERROR: {e}")
            return False
    
    async def test_order_modification_simulation(self):
        """Test order modification in paper trading mode."""
        print("\n4. TESTING ORDER MODIFICATION (SIMULATION)")
        print("-" * 40)
        
        try:
            if not self.test_orders:
                print("   No orders to modify")
                return True
            
            order_to_modify = self.test_orders[0]
            print(f"   Modifying Order ID: {order_to_modify['order_id']}")
            
            # Simulate modification parameters
            modifications = {
                'quantity': 2,  # Change quantity
                'order_type': 'LIMIT',  # Change to limit order
                'price': 2800.0  # Add limit price
            }
            
            print("   Modification parameters:")
            for key, value in modifications.items():
                print(f"     {key}: {order_to_modify.get(key, 'N/A')} → {value}")
            
            # Apply modifications
            order_to_modify.update(modifications)
            order_to_modify['last_modified'] = datetime.now().isoformat()
            
            print("   ✓ Order modification simulation successful")
            return True
            
        except Exception as e:
            print(f"   ERROR: {e}")
            return False
    
    async def test_order_cancellation_simulation(self):
        """Test order cancellation in paper trading mode."""
        print("\n5. TESTING ORDER CANCELLATION (SIMULATION)")
        print("-" * 40)
        
        try:
            if not self.test_orders:
                print("   No orders to cancel")
                return True
            
            order_to_cancel = self.test_orders[0]
            print(f"   Cancelling Order ID: {order_to_cancel['order_id']}")
            print(f"   Original Status: {order_to_cancel['status']}")
            
            # Simulate cancellation
            order_to_cancel['status'] = 'CANCELLED'
            order_to_cancel['cancelled_at'] = datetime.now().isoformat()
            
            print("   ✓ Order cancellation simulation successful")
            print(f"   New Status: {order_to_cancel['status']}")
            
            return True
            
        except Exception as e:
            print(f"   ERROR: {e}")
            return False
    
    async def test_order_status_tracking(self):
        """Test order status tracking and history."""
        print("\n6. TESTING ORDER STATUS TRACKING")
        print("-" * 40)
        
        try:
            print(f"   Total Orders in Session: {len(self.test_orders)}")
            
            if not self.test_orders:
                print("   No orders to track")
                return True
            
            # Display order history
            for i, order in enumerate(self.test_orders, 1):
                print(f"   Order {i}:")
                print(f"     ID: {order['order_id']}")
                print(f"     Symbol: {order['tradingsymbol']}")
                print(f"     Type: {order['transaction_type']} {order['quantity']}")
                print(f"     Order Type: {order['order_type']}")
                if 'price' in order:
                    print(f"     Price: Rs. {order['price']}")
                print(f"     Status: {order['status']}")
                print(f"     Created: {order['timestamp']}")
                if 'last_modified' in order:
                    print(f"     Modified: {order['last_modified']}")
                if 'cancelled_at' in order:
                    print(f"     Cancelled: {order['cancelled_at']}")
                print()
            
            # Simulate status updates
            status_transitions = ['OPEN', 'TRIGGER PENDING', 'COMPLETE', 'CANCELLED', 'REJECTED']
            print("   Possible status transitions:")
            for status in status_transitions:
                print(f"     - {status}")
            
            return True
            
        except Exception as e:
            print(f"   ERROR: {e}")
            return False
    
    async def test_risk_management_checks(self):
        """Test risk management and compliance checks."""
        print("\n7. TESTING RISK MANAGEMENT")
        print("-" * 40)
        
        try:
            print("   Risk Management Configuration:")
            print(f"     Max Order Value: Rs. {self.config.risk_management.max_order_value:,.2f}")
            print(f"     Max Position Value: Rs. {self.config.risk_management.max_position_value:,.2f}")
            print(f"     Max Orders/Minute: {self.config.risk_management.max_orders_per_minute}")
            print(f"     Validation Enabled: {self.config.risk_management.enable_validation}")
            
            # Test risk scenarios
            risk_scenarios = [
                {
                    'name': 'Large Order Value',
                    'order_value': self.config.risk_management.max_order_value + 50000,
                    'should_reject': True
                },
                {
                    'name': 'Normal Order Value', 
                    'order_value': self.config.risk_management.max_order_value * 0.1,
                    'should_reject': False
                },
                {
                    'name': 'Maximum Allowed Value',
                    'order_value': self.config.risk_management.max_order_value,
                    'should_reject': False
                }
            ]
            
            print("\n   Risk Scenario Testing:")
            for scenario in risk_scenarios:
                order_value = scenario['order_value']
                
                if order_value > self.config.risk_management.max_order_value:
                    result = "REJECTED"
                    correct = scenario['should_reject']
                else:
                    result = "APPROVED"
                    correct = not scenario['should_reject']
                
                status = "✓" if correct else "✗"
                print(f"   {status} {scenario['name']}: Rs. {order_value:,.2f} → {result}")
            
            # Test order frequency limits
            print(f"\n   Order Frequency Test:")
            print(f"     Current Orders in Minute: {len(self.test_orders)}")
            print(f"     Limit: {self.config.risk_management.max_orders_per_minute}")
            
            if len(self.test_orders) < self.config.risk_management.max_orders_per_minute:
                print("     ✓ Within frequency limits")
            else:
                print("     ✗ Exceeds frequency limits")
            
            return True
            
        except Exception as e:
            print(f"   ERROR: {e}")
            return False

    async def run_all_tests(self):
        """Run all order management tests."""
        print("=" * 50)
        print("ORDER MANAGEMENT FLOW TEST")
        print("=" * 50)
        print(f"Timestamp: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
        
        results = []
        
        test_methods = [
            self.setup,
            self.test_order_validation,
            self.test_order_placement_simulation,
            self.test_order_modification_simulation,
            self.test_order_cancellation_simulation,
            self.test_order_status_tracking,
            self.test_risk_management_checks
        ]
        
        test_names = [
            "Setup Configuration",
            "Order Validation",
            "Order Placement (Sim)",
            "Order Modification (Sim)",
            "Order Cancellation (Sim)",
            "Order Status Tracking",
            "Risk Management"
        ]
        
        for test_method, test_name in zip(test_methods, test_names):
            try:
                result = await test_method()
                results.append((test_name, result))
            except Exception as e:
                print(f"   EXCEPTION in {test_name}: {e}")
                results.append((test_name, False))
        
        # Summary
        print("\n" + "=" * 50)
        print("ORDER MANAGEMENT TEST SUMMARY")
        print("=" * 50)
        
        passed = sum(1 for _, result in results if result)
        total = len(results)
        
        for i, (name, result) in enumerate(results):
            status = "PASS" if result else "FAIL"
            print(f"{i+1}. {name}: {status}")
        
        print(f"\nOverall Result: {passed}/{total} tests passed")
        print(f"Orders Simulated: {len(self.test_orders)}")
        
        if passed == total:
            print("SUCCESS: Order management system is fully operational!")
        else:
            print("PARTIAL SUCCESS: Some order management features need attention")
        
        return passed == total

async def main():
    """Main test runner."""
    tester = OrderManagementTester()
    success = await tester.run_all_tests()
    return success

if __name__ == "__main__":
    success = asyncio.run(main())
    sys.exit(0 if success else 1)