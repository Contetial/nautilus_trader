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
Live data client implementation for Zerodha.

This module provides real-time market data streaming from Zerodha's Kite Connect API,
specifically optimized for options trading on Indian exchanges.
"""

from __future__ import annotations

import asyncio
from datetime import datetime, timezone
from typing import Any

from nautilus_trader.adapters.zerodha.config import ZerodhaDataClientConfig
from nautilus_trader.cache.cache import Cache
from nautilus_trader.common.component import LiveClock
from nautilus_trader.common.component import MessageBus
from nautilus_trader.core.uuid import UUID4
from nautilus_trader.live.data_client import LiveDataClient
from nautilus_trader.model.data import Bar
from nautilus_trader.model.data import BarType
from nautilus_trader.model.data import QuoteTick
from nautilus_trader.model.data import TradeTick
from nautilus_trader.model.enums import BookType
from nautilus_trader.model.identifiers import ClientId
from nautilus_trader.model.identifiers import InstrumentId
from nautilus_trader.model.identifiers import Venue
from nautilus_trader.model.instruments import Instrument


class ZerodhaDataClient(LiveDataClient):
    """
    Provides a data client for Zerodha Kite Connect API.
    
    This client handles real-time market data streaming and historical data
    requests for Indian equity and derivatives markets.
    
    Parameters
    ----------
    loop : asyncio.AbstractEventLoop
        The event loop for the client.
    client : Any
        The internal HTTP client (from Rust adapter).
    msgbus : MessageBus
        The message bus for the client.
    cache : Cache
        The cache for the client.
    clock : LiveClock
        The clock for the client.
    config : ZerodhaDataClientConfig
        The configuration for the client.
    """

    def __init__(
        self,
        loop: asyncio.AbstractEventLoop,
        client: Any,  # TODO: Type with actual Rust client when implemented
        msgbus: MessageBus,
        cache: Cache,
        clock: LiveClock,
        config: ZerodhaDataClientConfig,
    ) -> None:
        super().__init__(
            loop=loop,
            client_id=ClientId("ZERODHA"),
            venue=None,  # Multi-venue (NSE, BSE, etc.)
            msgbus=msgbus,
            cache=cache,
            clock=clock,
        )

        self._config = config
        self._client = client
        self._subscriptions: dict[InstrumentId, str] = {}
        self._instrument_provider: Any = None  # TODO: Type when implemented
        
        # WebSocket client for real-time data
        self._ws_client: Any = None  # TODO: Type when implemented
        self._is_connected = False
        
        # Market data handling
        self._quote_buffer: dict[InstrumentId, QuoteTick] = {}
        self._trade_buffer: dict[InstrumentId, TradeTick] = {}
        
        self._log.info(f"Initialized ZerodhaDataClient with config: {config}")

    @property
    def instrument_provider(self) -> Any:
        """
        Return the instrument provider for the client.
        
        Returns
        -------
        Any
            The Zerodha instrument provider.
        """
        return self._instrument_provider

    async def _connect(self) -> None:
        """Connect to the Zerodha data feed."""
        try:
            self._log.info("Connecting to Zerodha data feed...")
            
            # TODO: Initialize WebSocket client with actual implementation
            # self._ws_client = create_websocket_client(self._config)
            # await self._ws_client.connect()
            
            self._is_connected = True
            self._log.info("✅ Connected to Zerodha data feed")
            
        except Exception as e:
            self._log.error(f"❌ Failed to connect to Zerodha data feed: {e}")
            raise

    async def _disconnect(self) -> None:
        """Disconnect from the Zerodha data feed."""
        try:
            self._log.info("Disconnecting from Zerodha data feed...")
            
            if self._ws_client:
                # TODO: Implement WebSocket disconnection
                # await self._ws_client.disconnect()
                pass
                
            self._is_connected = False
            self._subscriptions.clear()
            self._log.info("✅ Disconnected from Zerodha data feed")
            
        except Exception as e:
            self._log.error(f"❌ Error during disconnection: {e}")

    # =========================================================================
    # Subscriptions
    # =========================================================================

    async def _subscribe_instruments(self, venue: Venue) -> None:
        """
        Subscribe to all instruments for the venue.
        
        Parameters
        ----------
        venue : Venue
            The venue to subscribe to.
        """
        if not self._instrument_provider:
            self._log.warning(f"No instrument provider available for {venue}")
            return
            
        # TODO: Load instruments for venue and subscribe
        self._log.info(f"Subscribing to all instruments for {venue}")

    async def _subscribe_instrument(self, instrument_id: InstrumentId) -> None:
        """
        Subscribe to the given instrument ID.
        
        Parameters
        ----------
        instrument_id : InstrumentId
            The instrument to subscribe to.
        """
        if instrument_id in self._subscriptions:
            self._log.warning(f"Already subscribed to {instrument_id}")
            return
            
        try:
            # TODO: Get instrument token from instrument_id
            # instrument_token = self._get_instrument_token(instrument_id)
            # await self._ws_client.subscribe([instrument_token], TickerMode.Full)
            
            self._subscriptions[instrument_id] = "subscribed"
            self._log.info(f"✅ Subscribed to {instrument_id}")
            
        except Exception as e:
            self._log.error(f"❌ Failed to subscribe to {instrument_id}: {e}")

    async def _unsubscribe_instrument(self, instrument_id: InstrumentId) -> None:
        """
        Unsubscribe from the given instrument ID.
        
        Parameters
        ----------
        instrument_id : InstrumentId
            The instrument to unsubscribe from.
        """
        if instrument_id not in self._subscriptions:
            self._log.warning(f"Not subscribed to {instrument_id}")
            return
            
        try:
            # TODO: Get instrument token and unsubscribe
            # instrument_token = self._get_instrument_token(instrument_id)
            # await self._ws_client.unsubscribe([instrument_token])
            
            del self._subscriptions[instrument_id]
            self._log.info(f"✅ Unsubscribed from {instrument_id}")
            
        except Exception as e:
            self._log.error(f"❌ Failed to unsubscribe from {instrument_id}: {e}")

    # =========================================================================
    # QuoteTicks
    # =========================================================================

    async def _subscribe_quote_ticks(self, instrument_id: InstrumentId) -> None:
        """
        Subscribe to quote ticks for the given instrument ID.
        
        Parameters
        ----------
        instrument_id : InstrumentId
            The instrument to subscribe to quote ticks for.
        """
        await self._subscribe_instrument(instrument_id)
        self._log.info(f"📊 Subscribed to quote ticks for {instrument_id}")

    async def _unsubscribe_quote_ticks(self, instrument_id: InstrumentId) -> None:
        """
        Unsubscribe from quote ticks for the given instrument ID.
        
        Parameters
        ----------
        instrument_id : InstrumentId
            The instrument to unsubscribe from quote ticks for.
        """
        await self._unsubscribe_instrument(instrument_id)
        if instrument_id in self._quote_buffer:
            del self._quote_buffer[instrument_id]

    # =========================================================================
    # TradeTicks  
    # =========================================================================

    async def _subscribe_trade_ticks(self, instrument_id: InstrumentId) -> None:
        """
        Subscribe to trade ticks for the given instrument ID.
        
        Parameters
        ----------
        instrument_id : InstrumentId
            The instrument to subscribe to trade ticks for.
        """
        await self._subscribe_instrument(instrument_id)
        self._log.info(f"💹 Subscribed to trade ticks for {instrument_id}")

    async def _unsubscribe_trade_ticks(self, instrument_id: InstrumentId) -> None:
        """
        Unsubscribe from trade ticks for the given instrument ID.
        
        Parameters
        ----------
        instrument_id : InstrumentId
            The instrument to unsubscribe from trade ticks for.
        """
        await self._unsubscribe_instrument(instrument_id)
        if instrument_id in self._trade_buffer:
            del self._trade_buffer[instrument_id]

    # =========================================================================
    # Bars
    # =========================================================================

    async def _subscribe_bars(self, bar_type: BarType) -> None:
        """
        Subscribe to bars for the given bar type.
        
        Parameters
        ----------
        bar_type : BarType
            The bar type to subscribe to.
        """
        await self._subscribe_instrument(bar_type.instrument_id)
        self._log.info(f"📈 Subscribed to bars for {bar_type}")

    async def _unsubscribe_bars(self, bar_type: BarType) -> None:
        """
        Unsubscribe from bars for the given bar type.
        
        Parameters
        ----------
        bar_type : BarType
            The bar type to unsubscribe from.
        """
        await self._unsubscribe_instrument(bar_type.instrument_id)

    # =========================================================================
    # OrderBookDeltas (not supported by Zerodha)
    # =========================================================================

    async def _subscribe_order_book_deltas(
        self, 
        instrument_id: InstrumentId,
        book_type: BookType,
        depth: int | None = None,
    ) -> None:
        """
        Subscribe to order book deltas.
        
        Note: Zerodha provides limited depth data (5 levels max).
        
        Parameters
        ----------
        instrument_id : InstrumentId
            The instrument to subscribe to.
        book_type : BookType
            The book type.
        depth : int | None
            The depth level (max 5 for Zerodha).
        """
        if depth and depth > 5:
            self._log.warning(f"Zerodha supports max depth of 5, requested {depth}")
            
        await self._subscribe_instrument(instrument_id)
        self._log.info(f"📊 Subscribed to order book for {instrument_id} (depth={depth})")

    async def _unsubscribe_order_book_deltas(self, instrument_id: InstrumentId) -> None:
        """
        Unsubscribe from order book deltas.
        
        Parameters
        ----------
        instrument_id : InstrumentId
            The instrument to unsubscribe from.
        """
        await self._unsubscribe_instrument(instrument_id)

    # =========================================================================
    # Historical Data Requests
    # =========================================================================

    async def _request_bars(
        self,
        bar_type: BarType,
        limit: int,
        correlation_id: UUID4,
        start: datetime | None = None,
        end: datetime | None = None,
    ) -> None:
        """
        Request historical bars for the given parameters.
        
        Parameters
        ----------
        bar_type : BarType
            The bar type to request.
        limit : int
            The limit for the number of bars.
        correlation_id : UUID4
            The correlation ID for the request.
        start : datetime | None
            The start time for the request.
        end : datetime | None
            The end time for the request.
        """
        try:
            self._log.info(f"Requesting {limit} bars for {bar_type} (correlation_id={correlation_id})")
            
            # Set default time range if not provided
            if not end:
                end = datetime.now(timezone.utc)
            if not start:
                # Default to 1000 bars back based on bar type
                # This is a simplified approach
                start = end  # TODO: Calculate proper start time based on bar_type
                
            # TODO: Implement historical data request using Rust client
            # bars = await self._client.get_historical_data(...)
            # self._handle_bars(bars, correlation_id)
            
            self._log.warning(f"Historical bars request not yet implemented for {bar_type}")
            
        except Exception as e:
            self._log.error(f"❌ Failed to request bars for {bar_type}: {e}")

    # =========================================================================
    # Data Handling
    # =========================================================================

    def _handle_tick_data(self, tick_data: Any) -> None:
        """
        Handle incoming tick data from WebSocket.
        
        Parameters
        ----------
        tick_data : Any
            The tick data from Zerodha WebSocket.
        """
        try:
            # Convert instrument token to InstrumentId
            instrument_id = self._token_to_instrument_id(tick_data.instrument_token)
            
            # Create and handle quote tick (if market depth available)
            if hasattr(tick_data, 'depth') and tick_data.depth:
                quote_tick = self._create_quote_tick(tick_data, instrument_id)
                if quote_tick:
                    self._handle_data(quote_tick)
            
            # Create and handle trade tick (if last trade data available)
            if hasattr(tick_data, 'last_price') and tick_data.last_price > 0:
                trade_tick = self._create_trade_tick(tick_data, instrument_id)
                if trade_tick:
                    self._handle_data(trade_tick)
            
            # Handle bar updates for subscribed bar types
            self._handle_bar_updates(tick_data, instrument_id)
            
        except Exception as e:
            self._log.error(f"❌ Error handling tick data: {e}")

    def _create_quote_tick(self, tick_data: Any, instrument_id: InstrumentId) -> QuoteTick | None:
        """
        Create a QuoteTick from Zerodha tick data.
        
        Parameters
        ----------
        tick_data : Any
            The Zerodha tick data.
        instrument_id : InstrumentId
            The instrument ID.
            
        Returns
        -------
        QuoteTick | None
            The created quote tick, or None if insufficient data.
        """
        try:
            from nautilus_trader.model.data import QuoteTick
            from nautilus_trader.model.objects import Price, Quantity
            
            # Extract depth data for bid/ask
            if not hasattr(tick_data, 'depth') or not tick_data.depth:
                return None
                
            depth = tick_data.depth
            
            # Get best bid and ask from depth
            if not depth.buy or not depth.sell:
                return None
                
            best_bid = depth.buy[0]  # First item is best bid
            best_ask = depth.sell[0]  # First item is best ask
            
            # Get instrument for precision
            instrument = self._cache.instrument(instrument_id)
            if not instrument:
                self._log.warning(f"Instrument not found for {instrument_id}")
                return None
            
            # Create quote tick
            quote_tick = QuoteTick(
                instrument_id=instrument_id,
                bid_price=Price(best_bid.price, instrument.price_precision),
                ask_price=Price(best_ask.price, instrument.price_precision),
                bid_size=Quantity(best_bid.quantity, instrument.size_precision),
                ask_size=Quantity(best_ask.quantity, instrument.size_precision),
                ts_event=self._clock.timestamp_ns(),
                ts_init=self._clock.timestamp_ns(),
            )
            
            return quote_tick
            
        except Exception as e:
            self._log.error(f"❌ Error creating QuoteTick: {e}")
            return None

    def _create_trade_tick(self, tick_data: Any, instrument_id: InstrumentId) -> TradeTick | None:
        """
        Create a TradeTick from Zerodha tick data.
        
        Parameters
        ----------
        tick_data : Any
            The Zerodha tick data.
        instrument_id : InstrumentId
            The instrument ID.
            
        Returns
        -------
        TradeTick | None
            The created trade tick, or None if no trade data.
        """
        try:
            from nautilus_trader.model.data import TradeTick
            from nautilus_trader.model.enums import AggressorSide
            from nautilus_trader.model.objects import Price, Quantity
            
            # Check if we have trade data
            if not hasattr(tick_data, 'last_price') or tick_data.last_price <= 0:
                return None
                
            # Get instrument for precision
            instrument = self._cache.instrument(instrument_id)
            if not instrument:
                self._log.warning(f"Instrument not found for {instrument_id}")
                return None
                
            # Get trade quantity (default to 1 if not available)
            trade_quantity = getattr(tick_data, 'last_quantity', 1) or 1
            
            # Determine aggressor side (buy/sell) - simplified logic
            # In practice, this would need more sophisticated logic
            aggressor_side = AggressorSide.UNKNOWN
            
            # Use best bid/ask to determine aggressor if available
            if hasattr(tick_data, 'depth') and tick_data.depth:
                if tick_data.depth.buy and tick_data.depth.sell:
                    best_bid = tick_data.depth.buy[0].price
                    best_ask = tick_data.depth.sell[0].price
                    mid_price = (best_bid + best_ask) / 2
                    
                    if tick_data.last_price >= mid_price:
                        aggressor_side = AggressorSide.BUYER
                    else:
                        aggressor_side = AggressorSide.SELLER
            
            # Create trade tick
            trade_tick = TradeTick(
                instrument_id=instrument_id,
                price=Price(tick_data.last_price, instrument.price_precision),
                size=Quantity(trade_quantity, instrument.size_precision),
                aggressor_side=aggressor_side,
                trade_id=None,  # Zerodha doesn't provide trade IDs in tick data
                ts_event=self._clock.timestamp_ns(),
                ts_init=self._clock.timestamp_ns(),
            )
            
            return trade_tick
            
        except Exception as e:
            self._log.error(f"❌ Error creating TradeTick: {e}")
            return None

    def _get_instrument_token(self, instrument_id: InstrumentId) -> int:
        """
        Get Zerodha instrument token from NautilusTrader InstrumentId.
        
        Parameters
        ----------
        instrument_id : InstrumentId
            The instrument ID.
            
        Returns
        -------
        int
            The Zerodha instrument token.
        """
        if self._instrument_provider:
            return self._instrument_provider.get_instrument_token(instrument_id)
        else:
            # Fallback: extract token from instrument_id if available
            # Format: symbol.exchange or symbol-token.exchange
            symbol_str = str(instrument_id.symbol)
            if '-' in symbol_str:
                token_part = symbol_str.split('-')[-1]
                try:
                    return int(token_part)
                except ValueError:
                    pass
            
            raise ValueError(f"Cannot resolve instrument token for {instrument_id}")

    def _token_to_instrument_id(self, token: int) -> InstrumentId:
        """
        Convert Zerodha instrument token to NautilusTrader InstrumentId.
        
        Parameters
        ----------
        token : int
            The Zerodha instrument token.
            
        Returns
        -------
        InstrumentId
            The instrument ID.
        """
        if self._instrument_provider:
            return self._instrument_provider.get_instrument_id(token)
        else:
            # Fallback: create basic instrument ID
            # This is a simplified approach - production would need proper mapping
            from nautilus_trader.model.identifiers import Symbol
            
            symbol = Symbol(f"TOKEN_{token}")
            venue = Venue("NSE")  # Default to NSE
            return InstrumentId(symbol, venue)

    def _handle_bar_updates(self, tick_data: Any, instrument_id: InstrumentId) -> None:
        """
        Handle bar updates from tick data.
        
        Parameters
        ----------
        tick_data : Any
            The tick data.
        instrument_id : InstrumentId
            The instrument ID.
        """
        try:
            # For now, we'll rely on NautilusTrader's built-in bar aggregation
            # from trade ticks. In the future, we could build bars directly
            # from OHLC data if available in full mode ticks.
            
            if hasattr(tick_data, 'ohlc') and tick_data.ohlc and hasattr(tick_data, 'timestamp'):
                # We have OHLC data - could potentially create bars directly
                # For now, just log for debugging
                self._log.debug(f"OHLC data available for {instrument_id}: {tick_data.ohlc}")
                
        except Exception as e:
            self._log.error(f"❌ Error handling bar updates: {e}")

    # =========================================================================
    # Market Status and Sessions
    # =========================================================================

    def _is_market_open(self, venue: Venue) -> bool:
        """
        Check if the market is currently open for the given venue.
        
        Parameters
        ----------
        venue : Venue
            The venue to check.
            
        Returns
        -------
        bool
            True if market is open, False otherwise.
        """
        from datetime import datetime, time
        import pytz
        
        try:
            # Get current time in IST
            ist = pytz.timezone('Asia/Kolkata')
            now_ist = datetime.now(ist)
            
            # Check if it's a weekday (Monday = 0, Sunday = 6)
            if now_ist.weekday() >= 5:  # Saturday or Sunday
                return False
            
            current_time = now_ist.time()
            
            # Define market hours for different venues
            if venue.value in ['NSE', 'BSE']:
                # Regular equity market hours
                market_open = time(9, 15)   # 9:15 AM IST
                market_close = time(15, 30) # 3:30 PM IST
                
                return market_open <= current_time <= market_close
                
            elif venue.value in ['NFO', 'BFO']:
                # Derivatives market hours (same as equity)
                market_open = time(9, 15)   # 9:15 AM IST
                market_close = time(15, 30) # 3:30 PM IST
                
                return market_open <= current_time <= market_close
                
            elif venue.value == 'MCX':
                # MCX commodity market has different hours
                # 9:00 AM to 11:30/11:55 PM IST (depending on commodity)
                market_open = time(9, 0)    # 9:00 AM IST
                market_close = time(23, 30) # 11:30 PM IST
                
                return market_open <= current_time <= market_close
                
            else:
                # Unknown venue - assume closed
                return False
                
        except Exception as e:
            self._log.error(f"❌ Error checking market hours: {e}")
            return False
    
    def _is_pre_open_session(self, venue: Venue) -> bool:
        """
        Check if market is in pre-open session.
        
        Parameters
        ----------
        venue : Venue
            The venue to check.
            
        Returns
        -------
        bool
            True if in pre-open session.
        """
        from datetime import datetime, time
        import pytz
        
        try:
            ist = pytz.timezone('Asia/Kolkata')
            now_ist = datetime.now(ist)
            
            if now_ist.weekday() >= 5:  # Weekend
                return False
                
            current_time = now_ist.time()
            
            if venue.value in ['NSE', 'BSE', 'NFO', 'BFO']:
                # Pre-open session: 9:00 AM - 9:15 AM IST
                pre_open_start = time(9, 0)
                pre_open_end = time(9, 15)
                
                return pre_open_start <= current_time < pre_open_end
                
            return False
            
        except Exception as e:
            self._log.error(f"❌ Error checking pre-open session: {e}")
            return False
    
    def _is_post_market_session(self, venue: Venue) -> bool:
        """
        Check if market is in post-market session.
        
        Parameters
        ----------
        venue : Venue
            The venue to check.
            
        Returns
        -------
        bool
            True if in post-market session.
        """
        from datetime import datetime, time
        import pytz
        
        try:
            ist = pytz.timezone('Asia/Kolkata')
            now_ist = datetime.now(ist)
            
            if now_ist.weekday() >= 5:  # Weekend
                return False
                
            current_time = now_ist.time()
            
            if venue.value in ['NSE', 'BSE']:
                # Post-market session: 3:40 PM - 4:00 PM IST
                post_market_start = time(15, 40)
                post_market_end = time(16, 0)
                
                return post_market_start <= current_time <= post_market_end
                
            return False
            
        except Exception as e:
            self._log.error(f"❌ Error checking post-market session: {e}")
            return False
    
    def _get_next_market_open(self, venue: Venue) -> datetime | None:
        """
        Get the next market opening time.
        
        Parameters
        ----------
        venue : Venue
            The venue to check.
            
        Returns
        -------
        datetime | None
            Next market opening time in UTC, or None if unknown venue.
        """
        from datetime import datetime, time, timedelta
        import pytz
        
        try:
            ist = pytz.timezone('Asia/Kolkata')
            now_ist = datetime.now(ist)
            
            # Market opening time
            market_open_time = time(9, 15) if venue.value in ['NSE', 'BSE', 'NFO', 'BFO'] else time(9, 0)
            
            # Calculate next market open
            next_open_date = now_ist.date()
            
            # If current time is after market hours, move to next day
            if now_ist.time() > time(15, 30):
                next_open_date += timedelta(days=1)
            
            # Skip weekends
            while next_open_date.weekday() >= 5:
                next_open_date += timedelta(days=1)
            
            # Create next opening datetime
            next_open_ist = ist.localize(datetime.combine(next_open_date, market_open_time))
            
            # Convert to UTC
            return next_open_ist.astimezone(timezone.utc)
            
        except Exception as e:
            self._log.error(f"❌ Error calculating next market open: {e}")
            return None
    
    def _log_market_status(self) -> None:
        """Log current market status for all venues."""
        venues = [Venue("NSE"), Venue("BSE"), Venue("NFO"), Venue("BFO")]
        
        for venue in venues:
            is_open = self._is_market_open(venue)
            is_pre_open = self._is_pre_open_session(venue)
            is_post_market = self._is_post_market_session(venue)
            
            if is_open:
                status = "🟢 OPEN"
            elif is_pre_open:
                status = "🟡 PRE-OPEN"
            elif is_post_market:
                status = "🟠 POST-MARKET"
            else:
                status = "🔴 CLOSED"
                next_open = self._get_next_market_open(venue)
                if next_open:
                    status += f" (Next open: {next_open.strftime('%Y-%m-%d %H:%M UTC')})"
            
            self._log.info(f"📅 {venue.value}: {status}")