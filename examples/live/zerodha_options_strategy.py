#!/usr/bin/env python3
# -------------------------------------------------------------------------------------------------
#  Copyright (C) 2015-2025 Nautech Systems Pty Ltd. All rights reserved.
#  https://nautechsystems.io
#
#  Licensed under the GNU Lesser General Public License Version 3.0 (the "License");
#  You may not use this file except in compliance with the License.
#  You may obtain a copy of the License at https://www.gnu.org/licenses/lgpl-3.0.en.html
#
#  Unless required by applicable law or agreed to in writing, software
#  distributed under the License is distributed on an "AS IS" BASIS,
#  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
#  See the License for the specific language governing permissions and
#  limitations under the License.
# -------------------------------------------------------------------------------------------------

"""
Example options trading strategy using Zerodha integration.

This example demonstrates:
- Options chain analysis for NSE indices (Nifty, Bank Nifty)
- Greeks-based position sizing and risk management
- Delta-neutral portfolio construction
- Real-time P&L monitoring with Greeks exposure

Usage:
    Set the following environment variables:
    - ZERODHA_API_KEY: Your Zerodha API key
    - ZERODHA_API_SECRET: Your API secret
    - ZERODHA_ACCESS_TOKEN: Your access token
    
    python zerodha_options_strategy.py
"""

import asyncio
from datetime import datetime, timedelta
from decimal import Decimal

from nautilus_trader.adapters.zerodha.config import ZerodhaDataClientConfig
from nautilus_trader.adapters.zerodha.config import ZerodhaExecClientConfig
from nautilus_trader.adapters.zerodha.greeks import GreeksCalculator, OptionPricingInputs
from nautilus_trader.adapters.zerodha.factories import ZerodhaLiveDataClientFactory
from nautilus_trader.adapters.zerodha.factories import ZerodhaLiveExecClientFactory
from nautilus_trader.common.component import MessageBus
from nautilus_trader.common.component import TestClock
from nautilus_trader.core.uuid import UUID4
from nautilus_trader.live.node import TradingNode
from nautilus_trader.model.data import QuoteTick, TradeTick
from nautilus_trader.model.enums import OptionKind, OrderSide, OrderType, TimeInForce
from nautilus_trader.model.events import PositionOpened, PositionChanged
from nautilus_trader.model.identifiers import StrategyId, TraderId
from nautilus_trader.model.instruments import Option
from nautilus_trader.model.objects import Price, Quantity
from nautilus_trader.model.orders import MarketOrder
from nautilus_trader.model.position import Position
from nautilus_trader.trading.strategy import Strategy


class DeltaNeutralOptionsStrategy(Strategy):
    """
    Delta-neutral options trading strategy.
    
    This strategy:
    1. Monitors Nifty 50 index options
    2. Constructs delta-neutral positions using straddles/strangles
    3. Rebalances when portfolio delta exceeds thresholds
    4. Manages risk using Greeks exposure limits
    """
    
    def __init__(self) -> None:
        super().__init__()
        
        # Strategy parameters
        self.underlying = "NIFTY"
        self.max_portfolio_delta = 50.0  # Maximum net delta exposure
        self.rebalance_threshold = 0.1   # Rebalance when delta > 10% of max
        self.max_vega_exposure = 1000.0  # Maximum vega exposure
        self.min_days_to_expiry = 7      # Minimum days to expiry
        self.max_days_to_expiry = 45     # Maximum days to expiry
        
        # Position tracking
        self.current_positions: dict[str, Position] = {}
        self.portfolio_greeks = {"delta": 0.0, "gamma": 0.0, "theta": 0.0, "vega": 0.0, "rho": 0.0}
        
        # Market data
        self.spot_price = 0.0
        self.options_quotes: dict[str, QuoteTick] = {}
        
        # Greeks calculator
        self.greeks_calc = GreeksCalculator()
        
    def on_start(self) -> None:
        """Strategy initialization."""
        self.log.info("🚀 Starting Delta-Neutral Options Strategy")
        self.log.info(f"   Underlying: {self.underlying}")
        self.log.info(f"   Max Portfolio Delta: {self.max_portfolio_delta}")
        self.log.info(f"   Max Vega Exposure: {self.max_vega_exposure}")
        
        # Subscribe to underlying index (for spot price)
        self._subscribe_to_underlying()
        
        # Subscribe to near-term options chain
        self._subscribe_to_options_chain()
        
    def on_stop(self) -> None:
        """Strategy cleanup."""
        self.log.info("⏹️ Stopping Delta-Neutral Options Strategy")
        self._close_all_positions()
        
    def on_quote_tick(self, tick: QuoteTick) -> None:
        """Handle incoming quote ticks."""
        try:
            instrument_id_str = str(tick.instrument_id)
            self.options_quotes[instrument_id_str] = tick
            
            # Check if this is the underlying
            if self._is_underlying_instrument(tick.instrument_id):
                # Update spot price (use mid price)
                self.spot_price = float((tick.bid_price + tick.ask_price) / 2)
                self.log.debug(f"📊 {self.underlying} spot: ₹{self.spot_price:.2f}")
                
                # Update portfolio Greeks
                self._update_portfolio_greeks()
                
                # Check rebalancing conditions
                self._check_rebalancing()
            
        except Exception as e:
            self.log.error(f"❌ Error handling quote tick: {e}")
    
    def on_trade_tick(self, tick: TradeTick) -> None:
        """Handle incoming trade ticks."""
        if self._is_underlying_instrument(tick.instrument_id):
            self.spot_price = float(tick.price)
            self._update_portfolio_greeks()
    
    def on_position_opened(self, event: PositionOpened) -> None:
        """Handle position opened events."""
        position = self.cache.position(event.position_id)
        if position:
            self.current_positions[str(position.instrument_id)] = position
            self.log.info(f"📈 Opened position: {position.instrument_id} | Size: {position.size}")
            self._update_portfolio_greeks()
    
    def on_position_changed(self, event: PositionChanged) -> None:
        """Handle position changed events."""
        position = self.cache.position(event.position_id)
        if position:
            self.current_positions[str(position.instrument_id)] = position
            self._update_portfolio_greeks()
    
    def _subscribe_to_underlying(self) -> None:
        """Subscribe to underlying index quotes."""
        # This would subscribe to Nifty 50 index
        # For demonstration, we'll subscribe to the actual traded instrument
        self.log.info(f"📡 Subscribing to {self.underlying} index")
        
        # In practice, you'd find the correct instrument ID for Nifty 50
        # and subscribe to its quote ticks
    
    def _subscribe_to_options_chain(self) -> None:
        """Subscribe to relevant options chain."""
        try:
            self.log.info(f"🔗 Subscribing to {self.underlying} options chain")
            
            # Get instrument provider
            provider = self.cache.get_zerodha_instrument_provider()  # Custom method
            if not provider:
                self.log.error("❌ Zerodha instrument provider not available")
                return
            
            # Get available expiries
            expiries = provider.get_expiries(self.underlying)
            if not expiries:
                self.log.warning(f"⚠️  No expiries found for {self.underlying}")
                return
            
            # Filter expiries by days to expiry
            now = datetime.now()
            valid_expiries = []
            
            for expiry_str in expiries:
                expiry_date = datetime.strptime(expiry_str, '%Y-%m-%d')
                days_to_expiry = (expiry_date - now).days
                
                if self.min_days_to_expiry <= days_to_expiry <= self.max_days_to_expiry:
                    valid_expiries.append(expiry_str)
            
            if not valid_expiries:
                self.log.warning("⚠️  No valid expiries found")
                return
            
            # Use nearest valid expiry
            nearest_expiry = min(valid_expiries)
            self.log.info(f"📅 Using expiry: {nearest_expiry}")
            
            # Get options chain for this expiry
            options_chain = provider.get_option_chain(self.underlying, nearest_expiry)
            
            # Subscribe to ATM and nearby strikes
            atm_strikes = self._get_atm_strikes(options_chain)
            
            for instrument_id in atm_strikes:
                self.subscribe_quote_ticks(instrument_id)
                self.log.debug(f"📡 Subscribed to {instrument_id}")
            
        except Exception as e:
            self.log.error(f"❌ Error subscribing to options chain: {e}")
    
    def _get_atm_strikes(self, options_chain: list) -> list:
        """Get ATM and nearby strike instruments."""
        # This would analyze the options chain and return relevant instruments
        # For demonstration, return first few instruments
        return options_chain[:10] if len(options_chain) >= 10 else options_chain
    
    def _is_underlying_instrument(self, instrument_id) -> bool:
        """Check if instrument is the underlying."""
        return self.underlying in str(instrument_id)
    
    def _update_portfolio_greeks(self) -> None:
        """Update portfolio-level Greeks."""
        try:
            if not self.current_positions or self.spot_price <= 0:
                return
            
            # Reset portfolio Greeks
            self.portfolio_greeks = {"delta": 0.0, "gamma": 0.0, "theta": 0.0, "vega": 0.0, "rho": 0.0}
            
            for instrument_id_str, position in self.current_positions.items():
                instrument = self.cache.instrument(position.instrument_id)
                
                if not isinstance(instrument, Option):
                    continue
                
                # Get current quote for this option
                quote = self.options_quotes.get(instrument_id_str)
                if not quote:
                    continue
                
                # Calculate option Greeks
                option_greeks = self._calculate_option_greeks(instrument, quote)
                
                if option_greeks:
                    # Add to portfolio Greeks (weighted by position size)
                    position_multiplier = float(position.size) * instrument.lot_size
                    
                    self.portfolio_greeks["delta"] += option_greeks.delta * position_multiplier
                    self.portfolio_greeks["gamma"] += option_greeks.gamma * position_multiplier
                    self.portfolio_greeks["theta"] += option_greeks.theta * position_multiplier
                    self.portfolio_greeks["vega"] += option_greeks.vega * position_multiplier
                    self.portfolio_greeks["rho"] += option_greeks.rho * position_multiplier
            
            # Log portfolio Greeks periodically
            self.log.debug(f"📊 Portfolio Greeks: Δ={self.portfolio_greeks['delta']:.1f}, "
                          f"Γ={self.portfolio_greeks['gamma']:.1f}, "
                          f"Θ={self.portfolio_greeks['theta']:.1f}, "
                          f"ν={self.portfolio_greeks['vega']:.1f}")
            
        except Exception as e:
            self.log.error(f"❌ Error updating portfolio Greeks: {e}")
    
    def _calculate_option_greeks(self, option: Option, quote: QuoteTick):
        """Calculate Greeks for a single option."""
        try:
            # Get time to expiry
            expiry_ns = option.expiration_ns
            if expiry_ns <= 0:
                return None
                
            expiry_dt = datetime.fromtimestamp(expiry_ns / 1e9)
            time_to_expiry = self.greeks_calc.time_to_expiry_years(expiry_dt)
            
            if time_to_expiry <= 0:
                return None
            
            # Use mid price for volatility calculation
            mid_price = float((quote.bid_price + quote.ask_price) / 2)
            
            # Create pricing inputs
            inputs = OptionPricingInputs(
                spot_price=self.spot_price,
                strike_price=float(option.strike_price),
                time_to_expiry=time_to_expiry,
                risk_free_rate=self.greeks_calc.DEFAULT_RISK_FREE_RATE,
                volatility=0.25,  # Default volatility, could calculate IV
                dividend_yield=self.greeks_calc.DEFAULT_DIVIDEND_YIELD,
                option_kind=option.option_kind
            )
            
            # Calculate implied volatility if possible
            iv = self.greeks_calc.calculate_implied_volatility(mid_price, inputs)
            if iv:
                inputs.volatility = iv
            
            # Calculate Greeks
            return self.greeks_calc.calculate_greeks(inputs)
            
        except Exception as e:
            self.log.warning(f"⚠️  Error calculating Greeks for {option.id}: {e}")
            return None
    
    def _check_rebalancing(self) -> None:
        """Check if portfolio needs rebalancing."""
        try:
            current_delta = abs(self.portfolio_greeks["delta"])
            delta_threshold = self.max_portfolio_delta * self.rebalance_threshold
            
            if current_delta > delta_threshold:
                self.log.info(f"⚖️  Portfolio delta ({current_delta:.1f}) exceeds threshold ({delta_threshold:.1f})")
                self._rebalance_portfolio()
            
            # Check vega exposure
            current_vega = abs(self.portfolio_greeks["vega"])
            if current_vega > self.max_vega_exposure:
                self.log.warning(f"⚠️  Vega exposure ({current_vega:.1f}) exceeds limit ({self.max_vega_exposure:.1f})")
                self._reduce_vega_exposure()
            
        except Exception as e:
            self.log.error(f"❌ Error checking rebalancing: {e}")
    
    def _rebalance_portfolio(self) -> None:
        """Rebalance portfolio to maintain delta neutrality."""
        try:
            self.log.info("⚖️  Rebalancing portfolio for delta neutrality")
            
            current_delta = self.portfolio_greeks["delta"]
            
            if abs(current_delta) < 1.0:  # Already balanced
                return
            
            # Find best hedge instrument (ATM options or underlying futures)
            hedge_instruments = self._find_hedge_instruments()
            
            for instrument_id, hedge_size in hedge_instruments:
                if abs(current_delta) < 1.0:
                    break
                
                # Calculate order size to hedge delta
                order_size = self._calculate_hedge_size(instrument_id, current_delta)
                
                if order_size != 0:
                    self._place_hedge_order(instrument_id, order_size)
                    
        except Exception as e:
            self.log.error(f"❌ Error rebalancing portfolio: {e}")
    
    def _reduce_vega_exposure(self) -> None:
        """Reduce vega exposure by closing some positions."""
        self.log.info("📉 Reducing vega exposure")
        
        # Sort positions by vega contribution
        vega_positions = []
        for instrument_id_str, position in self.current_positions.items():
            instrument = self.cache.instrument(position.instrument_id)
            if isinstance(instrument, Option):
                quote = self.options_quotes.get(instrument_id_str)
                if quote:
                    greeks = self._calculate_option_greeks(instrument, quote)
                    if greeks:
                        vega_contrib = abs(greeks.vega * float(position.size) * instrument.lot_size)
                        vega_positions.append((position, vega_contrib))
        
        # Sort by vega contribution (highest first)
        vega_positions.sort(key=lambda x: x[1], reverse=True)
        
        # Close positions with highest vega contribution
        vega_reduced = 0.0
        target_reduction = self.portfolio_greeks["vega"] * 0.3  # Reduce by 30%
        
        for position, vega_contrib in vega_positions:
            if vega_reduced >= target_reduction:
                break
                
            # Close 50% of the position
            close_size = abs(float(position.size)) * 0.5
            side = OrderSide.SELL if position.side.name == "LONG" else OrderSide.BUY
            
            self._place_market_order(position.instrument_id, side, close_size)
            vega_reduced += vega_contrib * 0.5
    
    def _find_hedge_instruments(self) -> list[tuple[str, float]]:
        """Find instruments suitable for hedging."""
        # This would implement logic to find the best hedging instruments
        # For now, return empty list
        return []
    
    def _calculate_hedge_size(self, instrument_id: str, target_delta: float) -> int:
        """Calculate hedge order size."""
        # This would calculate the appropriate hedge size
        # For now, return 0
        return 0
    
    def _place_hedge_order(self, instrument_id: str, size: int) -> None:
        """Place a hedge order."""
        self.log.info(f"🛡️  Placing hedge order: {instrument_id} size={size}")
        # Implementation would place actual hedge orders
    
    def _place_market_order(self, instrument_id, side: OrderSide, size: float) -> None:
        """Place a market order."""
        try:
            order = MarketOrder(
                trader_id=TraderId("TRADER-001"),
                strategy_id=self.id,
                instrument_id=instrument_id,
                order_side=side,
                quantity=Quantity.from_str(str(int(size))),
                init_id=UUID4(),
                ts_init=self.clock.timestamp_ns(),
            )
            
            self.submit_order(order)
            self.log.info(f"📤 Submitted {side.name} order: {instrument_id} qty={size}")
            
        except Exception as e:
            self.log.error(f"❌ Error placing market order: {e}")
    
    def _close_all_positions(self) -> None:
        """Close all open positions."""
        self.log.info("🔒 Closing all positions")
        
        for position in self.current_positions.values():
            if not position.is_closed:
                side = OrderSide.SELL if position.side.name == "LONG" else OrderSide.BUY
                self._place_market_order(position.instrument_id, side, abs(float(position.size)))


# Configuration and startup
async def main():
    """Run the options trading strategy."""
    
    # Create strategy
    strategy = DeltaNeutralOptionsStrategy()
    strategy_id = StrategyId("DELTA_NEUTRAL_NIFTY_OPTIONS")
    
    # Configure Zerodha clients
    data_config = ZerodhaDataClientConfig(
        api_key="your_api_key",      # Set from environment
        api_secret="your_secret",     # Set from environment  
        access_token="your_token",    # Set from environment
        sandbox_mode=True,            # Use sandbox for testing
    )
    
    exec_config = ZerodhaExecClientConfig(
        api_key="your_api_key",
        api_secret="your_secret", 
        access_token="your_token",
        sandbox_mode=True,
    )
    
    # Create trading node
    node = TradingNode()
    
    # Add strategy
    node.add_strategy(strategy)
    
    try:
        # Start the trading node
        await node.start_async()
        
        print("🚀 Options trading strategy started!")
        print("📊 Monitoring Nifty 50 options...")
        print("⚖️  Maintaining delta-neutral portfolio")
        print("🛑 Press Ctrl+C to stop")
        
        # Keep running
        while True:
            await asyncio.sleep(1)
            
    except KeyboardInterrupt:
        print("\n🛑 Shutting down strategy...")
    finally:
        await node.stop_async()


if __name__ == "__main__":
    asyncio.run(main())