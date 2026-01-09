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
Live data client for Kotak Neo integration.
"""

from __future__ import annotations

import asyncio
from datetime import datetime, timezone
from decimal import Decimal
from typing import Any

from nautilus_trader.adapters.kotak.config import KotakDataClientConfig
from nautilus_trader.adapters.kotak.providers import KotakInstrumentProvider
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
from nautilus_trader.model.objects import Price
from nautilus_trader.model.objects import Quantity


class KotakDataClient(LiveDataClient):
    """
    Provides a data client for Kotak Neo API.

    Parameters
    ----------
    loop : asyncio.AbstractEventLoop
        The event loop for the client.
    client : Any
        The Neo API client.
    msgbus : MessageBus
        The message bus for the client.
    cache : Cache
        The cache for the client.
    clock : LiveClock
        The clock for the client.
    config : KotakDataClientConfig
        The configuration for the client.
    instrument_provider : KotakInstrumentProvider | None
        The instrument provider.
    """

    def __init__(
        self,
        loop: asyncio.AbstractEventLoop,
        client: Any,
        msgbus: MessageBus,
        cache: Cache,
        clock: LiveClock,
        config: KotakDataClientConfig,
        instrument_provider: KotakInstrumentProvider | None = None,
    ) -> None:
        super().__init__(
            loop=loop,
            client_id=ClientId("KOTAK"),
            venue=None,
            msgbus=msgbus,
            cache=cache,
            clock=clock,
        )

        self._config = config
        self._client = client
        self._instrument_provider = instrument_provider
        self._subscriptions: dict[InstrumentId, str] = {}
        self._ws_connected = False

        self._log.info(f"Initialized KotakDataClient")

    @property
    def instrument_provider(self) -> KotakInstrumentProvider | None:
        return self._instrument_provider

    async def _connect(self) -> None:
        """Connect to Kotak Neo data feed."""
        try:
            self._log.info("Connecting to Kotak Neo data feed...")

            if self._config.access_token and hasattr(self._client, 'session_init'):
                self._client.session_init(access_token=self._config.access_token)

            # Set up WebSocket callback
            if hasattr(self._client, 'on_message'):
                self._client.on_message = self._handle_tick_data

            self._ws_connected = True
            self._log.info("Connected to Kotak Neo data feed")

        except Exception as e:
            self._log.error(f"Failed to connect: {e}")
            raise

    async def _disconnect(self) -> None:
        """Disconnect from Kotak Neo data feed."""
        try:
            self._log.info("Disconnecting from Kotak Neo...")
            self._ws_connected = False
            self._subscriptions.clear()
            self._log.info("Disconnected from Kotak Neo")
        except Exception as e:
            self._log.error(f"Error during disconnection: {e}")

    async def _subscribe_instrument(self, instrument_id: InstrumentId) -> None:
        """Subscribe to an instrument."""
        if instrument_id in self._subscriptions:
            return

        try:
            token = self._get_token(instrument_id)
            if not token:
                self._log.warning(f"Cannot resolve token for {instrument_id}")
                return

            if hasattr(self._client, 'subscribe'):
                self._client.subscribe(
                    instrument_tokens=[token],
                    isIndex=False,
                    isDepth=True,
                )

            self._subscriptions[instrument_id] = token
            self._log.info(f"Subscribed to {instrument_id}")

        except Exception as e:
            self._log.error(f"Failed to subscribe to {instrument_id}: {e}")

    async def _unsubscribe_instrument(self, instrument_id: InstrumentId) -> None:
        """Unsubscribe from an instrument."""
        if instrument_id not in self._subscriptions:
            return

        try:
            token = self._subscriptions[instrument_id]
            if hasattr(self._client, 'un_subscribe'):
                self._client.un_subscribe(instrument_tokens=[token])
            del self._subscriptions[instrument_id]
            self._log.info(f"Unsubscribed from {instrument_id}")
        except Exception as e:
            self._log.error(f"Failed to unsubscribe: {e}")

    async def _subscribe_quote_ticks(self, instrument_id: InstrumentId) -> None:
        await self._subscribe_instrument(instrument_id)

    async def _unsubscribe_quote_ticks(self, instrument_id: InstrumentId) -> None:
        await self._unsubscribe_instrument(instrument_id)

    async def _subscribe_trade_ticks(self, instrument_id: InstrumentId) -> None:
        await self._subscribe_instrument(instrument_id)

    async def _unsubscribe_trade_ticks(self, instrument_id: InstrumentId) -> None:
        await self._unsubscribe_instrument(instrument_id)

    async def _subscribe_bars(self, bar_type: BarType) -> None:
        await self._subscribe_instrument(bar_type.instrument_id)

    async def _unsubscribe_bars(self, bar_type: BarType) -> None:
        await self._unsubscribe_instrument(bar_type.instrument_id)

    async def _request_bars(
        self,
        bar_type: BarType,
        limit: int,
        correlation_id: UUID4,
        start: datetime | None = None,
        end: datetime | None = None,
    ) -> None:
        """Request historical bars."""
        self._log.info(f"Requesting bars for {bar_type}")
        # Implementation would use Neo historical data API

    def _handle_tick_data(self, tick_data: dict) -> None:
        """Handle incoming tick data from WebSocket."""
        try:
            token = tick_data.get("tk") or tick_data.get("token")
            if not token:
                return

            instrument_id = self._resolve_instrument_id(token)
            if not instrument_id:
                return

            quote_tick = self._create_quote_tick(tick_data, instrument_id)
            if quote_tick:
                self._handle_data(quote_tick)

            trade_tick = self._create_trade_tick(tick_data, instrument_id)
            if trade_tick:
                self._handle_data(trade_tick)

        except Exception as e:
            self._log.error(f"Error handling tick data: {e}")

    def _create_quote_tick(self, tick_data: dict, instrument_id: InstrumentId) -> QuoteTick | None:
        """Create a QuoteTick from Neo tick data."""
        try:
            bid = tick_data.get("bp") or tick_data.get("best_bid")
            ask = tick_data.get("sp") or tick_data.get("best_ask")
            bid_qty = tick_data.get("bq", 1)
            ask_qty = tick_data.get("sq", 1)

            if not bid or not ask:
                return None

            instrument = self._cache.instrument(instrument_id)
            if not instrument:
                return None

            return QuoteTick(
                instrument_id=instrument_id,
                bid_price=Price(Decimal(str(bid)), instrument.price_precision),
                ask_price=Price(Decimal(str(ask)), instrument.price_precision),
                bid_size=Quantity(Decimal(str(bid_qty)), instrument.size_precision),
                ask_size=Quantity(Decimal(str(ask_qty)), instrument.size_precision),
                ts_event=self._clock.timestamp_ns(),
                ts_init=self._clock.timestamp_ns(),
            )
        except Exception:
            return None

    def _create_trade_tick(self, tick_data: dict, instrument_id: InstrumentId) -> TradeTick | None:
        """Create a TradeTick from Neo tick data."""
        try:
            ltp = tick_data.get("ltp") or tick_data.get("last_price")
            volume = tick_data.get("v", 1)

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
        except Exception:
            return None

    def _get_token(self, instrument_id: InstrumentId) -> str | None:
        """Get Neo token for instrument."""
        if self._instrument_provider:
            return self._instrument_provider.get_instrument_token(instrument_id)
        return str(instrument_id.symbol)

    def _resolve_instrument_id(self, token: str) -> InstrumentId | None:
        """Resolve token to instrument ID."""
        if self._instrument_provider:
            return self._instrument_provider.get_instrument_id(token)
        return None
