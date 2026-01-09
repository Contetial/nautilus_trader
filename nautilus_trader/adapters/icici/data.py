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
Live data client implementation for ICICI Direct Breeze.

This module provides real-time market data streaming from ICICI Direct's Breeze API,
supporting NSE and BSE equity, futures, and options markets.
"""

from __future__ import annotations

import asyncio
from datetime import datetime, timezone
from decimal import Decimal
from typing import Any

from nautilus_trader.adapters.icici.config import ICICIDataClientConfig
from nautilus_trader.adapters.icici.providers import ICICIInstrumentProvider
from nautilus_trader.cache.cache import Cache
from nautilus_trader.common.component import LiveClock
from nautilus_trader.common.component import MessageBus
from nautilus_trader.core.uuid import UUID4
from nautilus_trader.live.data_client import LiveDataClient
from nautilus_trader.model.data import Bar
from nautilus_trader.model.data import BarType
from nautilus_trader.model.data import QuoteTick
from nautilus_trader.model.data import TradeTick
from nautilus_trader.model.enums import AggressorSide
from nautilus_trader.model.enums import BookType
from nautilus_trader.model.identifiers import ClientId
from nautilus_trader.model.identifiers import InstrumentId
from nautilus_trader.model.identifiers import Symbol
from nautilus_trader.model.identifiers import Venue
from nautilus_trader.model.instruments import Instrument
from nautilus_trader.model.objects import Price
from nautilus_trader.model.objects import Quantity


class ICICIDataClient(LiveDataClient):
    """
    Provides a data client for ICICI Direct Breeze API.

    This client handles real-time market data streaming and historical data
    requests for Indian equity and derivatives markets via Breeze WebSocket.

    Parameters
    ----------
    loop : asyncio.AbstractEventLoop
        The event loop for the client.
    client : Any
        The Breeze API client.
    msgbus : MessageBus
        The message bus for the client.
    cache : Cache
        The cache for the client.
    clock : LiveClock
        The clock for the client.
    config : ICICIDataClientConfig
        The configuration for the client.
    instrument_provider : ICICIInstrumentProvider | None
        The instrument provider for the client.
    """

    def __init__(
        self,
        loop: asyncio.AbstractEventLoop,
        client: Any,
        msgbus: MessageBus,
        cache: Cache,
        clock: LiveClock,
        config: ICICIDataClientConfig,
        instrument_provider: ICICIInstrumentProvider | None = None,
    ) -> None:
        super().__init__(
            loop=loop,
            client_id=ClientId("ICICI"),
            venue=None,  # Multi-venue (NSE, BSE, etc.)
            msgbus=msgbus,
            cache=cache,
            clock=clock,
        )

        self._config = config
        self._client = client
        self._instrument_provider = instrument_provider
        self._subscriptions: dict[InstrumentId, str] = {}

        # WebSocket state
        self._ws_connected = False

        # Data buffers
        self._quote_buffer: dict[InstrumentId, QuoteTick] = {}
        self._trade_buffer: dict[InstrumentId, TradeTick] = {}

        self._log.info(f"Initialized ICICIDataClient with config: {config}")

    @property
    def instrument_provider(self) -> ICICIInstrumentProvider | None:
        """
        Return the instrument provider for the client.

        Returns
        -------
        ICICIInstrumentProvider | None
            The ICICI instrument provider.
        """
        return self._instrument_provider

    # =========================================================================
    # Connection Lifecycle
    # =========================================================================

    async def _connect(self) -> None:
        """Connect to the ICICI Direct Breeze data feed."""
        try:
            self._log.info("Connecting to ICICI Direct Breeze data feed...")

            # Generate session if needed
            if self._config.session_token:
                self._client.generate_session(
                    api_secret=self._config.api_secret,
                    session_token=self._config.session_token,
                )

            # Connect to WebSocket
            self._client.ws_connect()

            # Set up tick callback
            self._client.on_ticks = self._handle_tick_data

            self._ws_connected = True
            self._log.info("Connected to ICICI Direct Breeze data feed")

        except Exception as e:
            self._log.error(f"Failed to connect to ICICI Direct data feed: {e}")
            raise

    async def _disconnect(self) -> None:
        """Disconnect from the ICICI Direct Breeze data feed."""
        try:
            self._log.info("Disconnecting from ICICI Direct Breeze data feed...")

            if self._client:
                try:
                    self._client.ws_disconnect()
                except Exception:
                    pass

            self._ws_connected = False
            self._subscriptions.clear()
            self._log.info("Disconnected from ICICI Direct Breeze data feed")

        except Exception as e:
            self._log.error(f"Error during disconnection: {e}")

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

        instruments = self._instrument_provider.list_instruments(venue=venue)
        for instrument in instruments:
            await self._subscribe_instrument(instrument.id)

        self._log.info(f"Subscribed to {len(instruments)} instruments for {venue}")

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
            # Get stock token for subscription
            token = self._get_stock_token(instrument_id)
            if not token:
                self._log.warning(f"Cannot resolve stock token for {instrument_id}")
                return

            # Determine exchange code from venue
            exchange_code = self._venue_to_exchange(instrument_id.venue)

            # Subscribe via Breeze WebSocket
            self._client.subscribe_feeds(
                stock_token=token,
                exchange_code=exchange_code,
                product_type=self._get_product_type(instrument_id),
            )

            self._subscriptions[instrument_id] = token
            self._log.info(f"Subscribed to {instrument_id}")

        except Exception as e:
            self._log.error(f"Failed to subscribe to {instrument_id}: {e}")

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
            token = self._subscriptions[instrument_id]
            exchange_code = self._venue_to_exchange(instrument_id.venue)

            self._client.unsubscribe_feeds(
                stock_token=token,
                exchange_code=exchange_code,
            )

            del self._subscriptions[instrument_id]
            self._log.info(f"Unsubscribed from {instrument_id}")

        except Exception as e:
            self._log.error(f"Failed to unsubscribe from {instrument_id}: {e}")

    # =========================================================================
    # QuoteTicks
    # =========================================================================

    async def _subscribe_quote_ticks(self, instrument_id: InstrumentId) -> None:
        """Subscribe to quote ticks for the given instrument."""
        await self._subscribe_instrument(instrument_id)
        self._log.info(f"Subscribed to quote ticks for {instrument_id}")

    async def _unsubscribe_quote_ticks(self, instrument_id: InstrumentId) -> None:
        """Unsubscribe from quote ticks for the given instrument."""
        await self._unsubscribe_instrument(instrument_id)
        if instrument_id in self._quote_buffer:
            del self._quote_buffer[instrument_id]

    # =========================================================================
    # TradeTicks
    # =========================================================================

    async def _subscribe_trade_ticks(self, instrument_id: InstrumentId) -> None:
        """Subscribe to trade ticks for the given instrument."""
        await self._subscribe_instrument(instrument_id)
        self._log.info(f"Subscribed to trade ticks for {instrument_id}")

    async def _unsubscribe_trade_ticks(self, instrument_id: InstrumentId) -> None:
        """Unsubscribe from trade ticks for the given instrument."""
        await self._unsubscribe_instrument(instrument_id)
        if instrument_id in self._trade_buffer:
            del self._trade_buffer[instrument_id]

    # =========================================================================
    # Bars
    # =========================================================================

    async def _subscribe_bars(self, bar_type: BarType) -> None:
        """Subscribe to bars for the given bar type."""
        await self._subscribe_instrument(bar_type.instrument_id)
        self._log.info(f"Subscribed to bars for {bar_type}")

    async def _unsubscribe_bars(self, bar_type: BarType) -> None:
        """Unsubscribe from bars for the given bar type."""
        await self._unsubscribe_instrument(bar_type.instrument_id)

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
            self._log.info(f"Requesting {limit} bars for {bar_type}")

            # Set default time range
            if not end:
                end = datetime.now(timezone.utc)
            if not start:
                # Calculate start based on bar type and limit
                start = end  # Simplified - actual implementation would calculate properly

            instrument_id = bar_type.instrument_id
            symbol = str(instrument_id.symbol)
            exchange = self._venue_to_exchange(instrument_id.venue)

            # Map bar aggregation to Breeze interval
            interval = self._bar_spec_to_interval(bar_type)

            # Fetch historical data using Breeze API
            response = self._client.get_historical_data(
                interval=interval,
                from_date=start.strftime("%Y-%m-%dT07:00:00.000Z"),
                to_date=end.strftime("%Y-%m-%dT07:00:00.000Z"),
                stock_code=symbol,
                exchange_code=exchange,
                product_type="cash",
            )

            if response and response.get("Success"):
                bars = self._parse_historical_bars(response["Success"], bar_type)
                for bar in bars:
                    self._handle_data(bar)

            self._log.info(f"Received historical bars for {bar_type}")

        except Exception as e:
            self._log.error(f"Failed to request bars for {bar_type}: {e}")

    # =========================================================================
    # Data Handling
    # =========================================================================

    def _handle_tick_data(self, tick_data: dict) -> None:
        """
        Handle incoming tick data from Breeze WebSocket.

        Parameters
        ----------
        tick_data : dict
            The tick data from Breeze WebSocket.
        """
        try:
            # Extract instrument info
            stock_code = tick_data.get("symbol") or tick_data.get("stock_code")
            exchange = tick_data.get("exchange_code", "NSE")

            if not stock_code:
                return

            # Resolve instrument ID
            instrument_id = self._resolve_instrument_id(stock_code, exchange)
            if not instrument_id:
                return

            # Create and handle quote tick
            quote_tick = self._create_quote_tick(tick_data, instrument_id)
            if quote_tick:
                self._handle_data(quote_tick)

            # Create and handle trade tick
            trade_tick = self._create_trade_tick(tick_data, instrument_id)
            if trade_tick:
                self._handle_data(trade_tick)

        except Exception as e:
            self._log.error(f"Error handling tick data: {e}")

    def _create_quote_tick(self, tick_data: dict, instrument_id: InstrumentId) -> QuoteTick | None:
        """Create a QuoteTick from Breeze tick data."""
        try:
            bid_price = tick_data.get("best_bid_price")
            ask_price = tick_data.get("best_offer_price")
            bid_qty = tick_data.get("best_bid_quantity", 1)
            ask_qty = tick_data.get("best_offer_quantity", 1)

            if not bid_price or not ask_price:
                return None

            instrument = self._cache.instrument(instrument_id)
            if not instrument:
                return None

            return QuoteTick(
                instrument_id=instrument_id,
                bid_price=Price(Decimal(str(bid_price)), instrument.price_precision),
                ask_price=Price(Decimal(str(ask_price)), instrument.price_precision),
                bid_size=Quantity(Decimal(str(bid_qty)), instrument.size_precision),
                ask_size=Quantity(Decimal(str(ask_qty)), instrument.size_precision),
                ts_event=self._clock.timestamp_ns(),
                ts_init=self._clock.timestamp_ns(),
            )

        except Exception as e:
            self._log.debug(f"Error creating QuoteTick: {e}")
            return None

    def _create_trade_tick(self, tick_data: dict, instrument_id: InstrumentId) -> TradeTick | None:
        """Create a TradeTick from Breeze tick data."""
        try:
            ltp = tick_data.get("last") or tick_data.get("ltp")
            volume = tick_data.get("total_quantity_traded", 1)

            if not ltp:
                return None

            instrument = self._cache.instrument(instrument_id)
            if not instrument:
                return None

            return TradeTick(
                instrument_id=instrument_id,
                price=Price(Decimal(str(ltp)), instrument.price_precision),
                size=Quantity(Decimal(str(volume)), instrument.size_precision),
                aggressor_side=AggressorSide.UNKNOWN,
                trade_id=None,
                ts_event=self._clock.timestamp_ns(),
                ts_init=self._clock.timestamp_ns(),
            )

        except Exception as e:
            self._log.debug(f"Error creating TradeTick: {e}")
            return None

    # =========================================================================
    # Helper Methods
    # =========================================================================

    def _get_stock_token(self, instrument_id: InstrumentId) -> str | None:
        """Get Breeze stock token for instrument."""
        if self._instrument_provider:
            return self._instrument_provider.get_instrument_token(instrument_id)

        # Fallback: construct token from symbol
        # Format varies by exchange/segment
        return str(instrument_id.symbol)

    def _venue_to_exchange(self, venue: Venue) -> str:
        """Convert NautilusTrader Venue to Breeze exchange code."""
        mapping = {
            "NSE": "NSE",
            "BSE": "BSE",
            "NFO": "NFO",
            "BFO": "BFO",
        }
        return mapping.get(venue.value, "NSE")

    def _get_product_type(self, instrument_id: InstrumentId) -> str:
        """Determine product type from instrument."""
        venue = instrument_id.venue.value
        if venue in ["NFO", "BFO"]:
            symbol = str(instrument_id.symbol)
            if "CE" in symbol or "PE" in symbol:
                return "options"
            return "futures"
        return "cash"

    def _resolve_instrument_id(self, stock_code: str, exchange: str) -> InstrumentId | None:
        """Resolve stock code to instrument ID."""
        try:
            venue = Venue(exchange)
            return InstrumentId(Symbol(stock_code), venue)
        except Exception:
            return None

    def _bar_spec_to_interval(self, bar_type: BarType) -> str:
        """Convert bar type to Breeze interval string."""
        # Map bar aggregation to Breeze intervals
        # Available: 1minute, 5minute, 30minute, 1day
        step = bar_type.spec.step

        if bar_type.spec.aggregation.value == 1:  # SECOND
            return "1minute"  # Minimum supported
        elif bar_type.spec.aggregation.value == 2:  # MINUTE
            if step <= 1:
                return "1minute"
            elif step <= 5:
                return "5minute"
            else:
                return "30minute"
        elif bar_type.spec.aggregation.value == 3:  # HOUR
            return "30minute"
        else:  # DAY or higher
            return "1day"

    def _parse_historical_bars(self, data: list, bar_type: BarType) -> list[Bar]:
        """Parse historical data into Bar objects."""
        bars = []

        instrument = self._cache.instrument(bar_type.instrument_id)
        if not instrument:
            return bars

        for item in data:
            try:
                bar = Bar(
                    bar_type=bar_type,
                    open=Price(Decimal(str(item.get("open", 0))), instrument.price_precision),
                    high=Price(Decimal(str(item.get("high", 0))), instrument.price_precision),
                    low=Price(Decimal(str(item.get("low", 0))), instrument.price_precision),
                    close=Price(Decimal(str(item.get("close", 0))), instrument.price_precision),
                    volume=Quantity(Decimal(str(item.get("volume", 0))), 0),
                    ts_event=self._clock.timestamp_ns(),
                    ts_init=self._clock.timestamp_ns(),
                )
                bars.append(bar)
            except Exception as e:
                self._log.debug(f"Error parsing bar: {e}")

        return bars
