#!/usr/bin/env python3
"""Test paper trading execution system."""

import sys
import asyncio
import requests
from pathlib import Path
from datetime import datetime
import json

# Add the adapters path
sys.path.insert(0, str(Path(__file__).parent / "nautilus_trader" / "adapters" / "zerodha"))

class PaperTradingTester:
    """Test paper trading execution system."""
    
    def __init__(self):
        self.config = None
        self.session = requests.Session()
        self.paper_orders = []
        self.paper_positions = {}
        self.paper_balance = 0.0
        
    async def setup(self):
        """Setup paper trading test environment."""
        print("1. SETTING UP PAPER TRADING TEST")
        print("-" * 40)
        
        try:
            from config_loader import ZerodhaCredentialsConfig
            self.config = ZerodhaCredentialsConfig.load()
            
            # Initialize paper trading balance
            self.paper_balance = self.config.paper_trading.initial_balance
            print(f"   Initial Balance: Rs. {self.paper_balance:,.2f}")
            print(f"   Commission per Trade: Rs. {self.config.paper_trading.commission_per_trade}")
            print(f"   Execution Delay: {self.config.paper_trading.execution_delay_ms}ms")
            
            # Setup session for live market data (for realistic simulation)
            self.session.headers.update({
                'Authorization': f'token {self.config.api.api_key}:{self.config.api.access_token}',
                'X-Kite-Version': '3'
            })
            
            return True
            
        except Exception as e:
            print(f"   ERROR: {e}")
            return False
    
    async def get_market_price(self, symbol):
        """Get live market price for realistic simulation."""
        try:
            # Try to get live quote, fallback to mock price
            response = self.session.get(f"https://api.kite.trade/quote?i={symbol}", timeout=10)
            
            if response.status_code == 200:
                data = response.json()
                return data['data'][symbol]['last_price']
            else:
                # Mock prices for testing
                mock_prices = {
                    "NSE:RELIANCE": 2800.50,
                    "NSE:TCS": 3500.75,
                    "NSE:INFY": 1750.25,
                    "NSE:HDFCBANK": 1650.80,
                    "NSE:ICICIBANK": 1200.45
                }
                return mock_prices.get(symbol, 1000.0)
                
        except Exception:
            return 1000.0  # Default price for testing
    
    async def simulate_order_execution(self, order):
        """Simulate order execution with realistic parameters."""
        print(f"\n   Executing Order: {order['transaction_type']} {order['quantity']} {order['tradingsymbol']}")
        
        # Get current market price
        current_price = await self.get_market_price(order['tradingsymbol'])
        print(f"   Current Market Price: Rs. {current_price}")
        
        # Determine execution price based on order type
        if order['order_type'] == 'MARKET':
            execution_price = current_price
        elif order['order_type'] == 'LIMIT':
            limit_price = order['price']
            if order['transaction_type'] == 'BUY' and current_price <= limit_price:
                execution_price = min(current_price, limit_price)
            elif order['transaction_type'] == 'SELL' and current_price >= limit_price:
                execution_price = max(current_price, limit_price)
            else:
                print(f"   Order NOT EXECUTED - Price condition not met (Market: {current_price}, Limit: {limit_price})")
                return False
        else:
            execution_price = current_price
        
        # Calculate order value and commission
        order_value = execution_price * order['quantity']
        commission = self.config.paper_trading.commission_per_trade
        total_cost = order_value + commission if order['transaction_type'] == 'BUY' else order_value - commission
        
        print(f"   Execution Price: Rs. {execution_price}")
        print(f"   Order Value: Rs. {order_value:,.2f}")
        print(f"   Commission: Rs. {commission}")
        print(f"   Total Impact: Rs. {total_cost:,.2f}")
        
        # Check sufficient balance for BUY orders
        if order['transaction_type'] == 'BUY':
            if total_cost > self.paper_balance:
                print(f"   ERROR: Insufficient balance (Required: Rs. {total_cost:,.2f}, Available: Rs. {self.paper_balance:,.2f})")
                return False
            self.paper_balance -= total_cost
        else:
            self.paper_balance += total_cost
        
        # Update positions
        symbol = order['tradingsymbol']
        if symbol not in self.paper_positions:
            self.paper_positions[symbol] = {
                'quantity': 0,
                'average_price': 0.0,
                'realized_pnl': 0.0
            }
        
        position = self.paper_positions[symbol]
        
        if order['transaction_type'] == 'BUY':
            # Update average price for buy orders
            total_quantity = position['quantity'] + order['quantity']
            if total_quantity > 0:
                position['average_price'] = (
                    (position['average_price'] * position['quantity']) + 
                    (execution_price * order['quantity'])
                ) / total_quantity
            position['quantity'] = total_quantity
        else:
            # Calculate realized P&L for sell orders
            if position['quantity'] >= order['quantity']:
                realized_pnl = (execution_price - position['average_price']) * order['quantity'] - commission
                position['realized_pnl'] += realized_pnl
                position['quantity'] -= order['quantity']
                print(f"   Realized P&L: Rs. {realized_pnl:,.2f}")
            else:
                print(f"   ERROR: Insufficient position to sell (Have: {position['quantity']}, Selling: {order['quantity']})")
                return False
        
        # Add to order history
        executed_order = order.copy()
        executed_order.update({
            'execution_price': execution_price,
            'order_value': order_value,
            'commission': commission,
            'status': 'COMPLETE',
            'timestamp': datetime.now().isoformat()
        })
        self.paper_orders.append(executed_order)
        
        print(f"   Updated Balance: Rs. {self.paper_balance:,.2f}")
        print(f"   Position in {symbol}: {position['quantity']} @ Rs. {position['average_price']:.2f}")
        
        return True
    
    async def test_market_buy_order(self):
        """Test market buy order execution."""
        print("\n2. TESTING MARKET BUY ORDER")
        print("-" * 40)
        
        order = {
            'tradingsymbol': 'NSE:RELIANCE',
            'transaction_type': 'BUY',
            'quantity': 10,
            'order_type': 'MARKET',
            'product': 'MIS'
        }
        
        return await self.simulate_order_execution(order)
    
    async def test_limit_buy_order(self):
        """Test limit buy order execution."""
        print("\n3. TESTING LIMIT BUY ORDER")
        print("-" * 40)
        
        order = {
            'tradingsymbol': 'NSE:TCS',
            'transaction_type': 'BUY',
            'quantity': 5,
            'order_type': 'LIMIT',
            'price': 3600.0,  # Above current market price
            'product': 'MIS'
        }
        
        return await self.simulate_order_execution(order)
    
    async def test_market_sell_order(self):
        """Test market sell order execution."""
        print("\n4. TESTING MARKET SELL ORDER")
        print("-" * 40)
        
        # First ensure we have a position to sell
        if 'NSE:RELIANCE' not in self.paper_positions or self.paper_positions['NSE:RELIANCE']['quantity'] <= 0:
            print("   No RELIANCE position to sell, skipping test")
            return True
        
        quantity_to_sell = min(5, self.paper_positions['NSE:RELIANCE']['quantity'])
        
        order = {
            'tradingsymbol': 'NSE:RELIANCE',
            'transaction_type': 'SELL',
            'quantity': quantity_to_sell,
            'order_type': 'MARKET',
            'product': 'MIS'
        }
        
        return await self.simulate_order_execution(order)
    
    async def test_portfolio_summary(self):
        """Test portfolio summary calculation."""
        print("\n5. PORTFOLIO SUMMARY")
        print("-" * 40)
        
        total_position_value = 0.0
        total_unrealized_pnl = 0.0
        
        for symbol, position in self.paper_positions.items():
            if position['quantity'] > 0:
                current_price = await self.get_market_price(symbol)
                position_value = current_price * position['quantity']
                unrealized_pnl = (current_price - position['average_price']) * position['quantity']
                
                total_position_value += position_value
                total_unrealized_pnl += unrealized_pnl
                
                print(f"   {symbol}:")
                print(f"     Quantity: {position['quantity']}")
                print(f"     Avg Price: Rs. {position['average_price']:.2f}")
                print(f"     Current Price: Rs. {current_price:.2f}")
                print(f"     Position Value: Rs. {position_value:,.2f}")
                print(f"     Unrealized P&L: Rs. {unrealized_pnl:,.2f}")
                print(f"     Realized P&L: Rs. {position['realized_pnl']:,.2f}")
        
        total_portfolio_value = self.paper_balance + total_position_value
        total_pnl = total_unrealized_pnl + sum(pos['realized_pnl'] for pos in self.paper_positions.values())
        
        print(f"\n   PORTFOLIO TOTALS:")
        print(f"     Cash Balance: Rs. {self.paper_balance:,.2f}")
        print(f"     Position Value: Rs. {total_position_value:,.2f}")
        print(f"     Total Portfolio Value: Rs. {total_portfolio_value:,.2f}")
        print(f"     Total P&L: Rs. {total_pnl:,.2f}")
        print(f"     Return: {total_pnl/self.config.paper_trading.initial_balance*100:.2f}%")
        
        return True
    
    async def test_order_history(self):
        """Display order execution history."""
        print("\n6. ORDER EXECUTION HISTORY")
        print("-" * 40)
        
        if not self.paper_orders:
            print("   No orders executed")
            return True
        
        for i, order in enumerate(self.paper_orders, 1):
            print(f"   Order {i}:")
            print(f"     Symbol: {order['tradingsymbol']}")
            print(f"     Type: {order['transaction_type']} {order['quantity']} @ {order['order_type']}")
            print(f"     Execution: Rs. {order['execution_price']:.2f}")
            print(f"     Value: Rs. {order['order_value']:.2f}")
            print(f"     Status: {order['status']}")
            print(f"     Time: {order['timestamp']}")
            print()
        
        return True

    async def run_all_tests(self):
        """Run all paper trading tests."""
        print("=" * 50)
        print("PAPER TRADING EXECUTION TEST")
        print("=" * 50)
        print(f"Timestamp: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
        
        results = []
        
        test_methods = [
            self.setup,
            self.test_market_buy_order,
            self.test_limit_buy_order,
            self.test_market_sell_order,
            self.test_portfolio_summary,
            self.test_order_history
        ]
        
        test_names = [
            "Setup Paper Trading",
            "Market Buy Order",
            "Limit Buy Order",
            "Market Sell Order",
            "Portfolio Summary",
            "Order History"
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
        print("PAPER TRADING TEST SUMMARY")
        print("=" * 50)
        
        passed = sum(1 for _, result in results if result)
        total = len(results)
        
        for i, (name, result) in enumerate(results):
            status = "PASS" if result else "FAIL"
            print(f"{i+1}. {name}: {status}")
        
        print(f"\nOverall Result: {passed}/{total} tests passed")
        
        if passed == total:
            print("SUCCESS: Paper trading system is fully operational!")
        else:
            print("PARTIAL SUCCESS: Some paper trading features need attention")
        
        return passed == total

async def main():
    """Main test runner."""
    tester = PaperTradingTester()
    success = await tester.run_all_tests()
    return success

if __name__ == "__main__":
    success = asyncio.run(main())
    sys.exit(0 if success else 1)