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
Historical data client for Kotak Neo.
"""

from __future__ import annotations

import asyncio
from datetime import datetime, date
from decimal import Decimal
from typing import Any

from nautilus_trader.adapters.kotak.config import KotakConfig
from nautilus_trader.adapters.kotak.providers import KotakInstrumentProvider
from nautilus_trader.common.component import Logger
from nautilus_trader.model.data import Bar
from nautilus_trader.model.data import BarType
from nautilus_trader.model.data import BarSpecification
from nautilus_trader.model.enums import BarAggregation
from nautilus_trader.model.enums import PriceType
from nautilus_trader.model.identifiers import InstrumentId
from nautilus_trader.model.identifiers import Venue
from nautilus_trader.model.objects import Price
from nautilus_trader.model.objects import Quantity


class KotakHistoricalClient:
    """
    Historical data fetching from Kotak Neo API.

    Parameters
    ----------
    client : Any
        The Neo API client.
    config : KotakConfig
        The configuration.
    logger : Logger
        The logger.
    instrument_provider : KotakInstrumentProvider | None
        The instrument provider.
    """

    def __init__(
        self,
        client: Any,
        config: KotakConfig,
        logger: Logger,
        instrument_provider: KotakInstrumentProvider | None = None,
    ) -> None:
        self._client = client
        self._config = config
        self._log = logger
        self._instrument_provider = instrument_provider
        self._last_request_time: datetime | None = None
        self._min_request_interval = 1.0 / config.rate_limit_per_second

    async def connect(self) -> None:
        """Connect to Kotak Neo API."""
        self._log.info("Connecting to Kotak Neo for historical data...")
        if self._config.access_token and hasattr(self._client, 'session_init'):
            self._client.session_init(access_token=self._config.access_token)
        self._log.info("Connected to Kotak Neo historical data service")

    async def disconnect(self) -> None:
        """Disconnect from Kotak Neo API."""
        self._log.info("Disconnected from Kotak Neo historical data service")

    async def request_bars(
        self,
        instrument_id: InstrumentId,
        bar_spec: BarSpecification,
        start: datetime,
        end: datetime,
    ) -> list[Bar]:
        """
        Request historical bars.

        Parameters
        ----------
        instrument_id : InstrumentId
            The instrument.
        bar_spec : BarSpecification
            The bar specification.
        start : datetime
            Start time.
        end : datetime
            End time.

        Returns
        -------
        list[Bar]
            List of bars.
        """
        try:
            self._log.info(f"Requesting bars for {instrument_id} from {start} to {end}")

            await self._rate_limit()

            interval = self._bar_spec_to_interval(bar_spec)
            exchange = self._venue_to_exchange(instrument_id.venue)

            response = self._client.history(
                exchange_segment=exchange,
                trading_symbol=str(instrument_id.symbol),
                from_date=start.strftime("%d-%m-%Y"),
                to_date=end.strftime("%d-%m-%Y"),
                type=interval,
            )

            if not response or not response.get("data"):
                self._log.warning(f"No data for {instrument_id}")
                return []

            bar_type = BarType(instrument_id=instrument_id, bar_spec=bar_spec)
            bars = self._parse_bars(response["data"], bar_type)

            self._log.info(f"Received {len(bars)} bars for {instrument_id}")
            return bars

        except Exception as e:
            self._log.error(f"Failed to request bars: {e}")
            return []

    async def request_daily_bars(
        self,
        instrument_id: InstrumentId,
        start_date: date,
        end_date: date,
    ) -> list[Bar]:
        """Request daily bars."""
        bar_spec = BarSpecification(
            step=1,
            aggregation=BarAggregation.DAY,
            price_type=PriceType.LAST,
        )
        start_dt = datetime.combine(start_date, datetime.min.time())
        end_dt = datetime.combine(end_date, datetime.max.time())
        return await self.request_bars(instrument_id, bar_spec, start_dt, end_dt)

    async def request_intraday_bars(
        self,
        instrument_id: InstrumentId,
        interval_minutes: int,
        start: datetime,
        end: datetime,
    ) -> list[Bar]:
        """Request intraday bars."""
        bar_spec = BarSpecification(
            step=interval_minutes,
            aggregation=BarAggregation.MINUTE,
            price_type=PriceType.LAST,
        )
        return await self.request_bars(instrument_id, bar_spec, start, end)

    def _parse_bars(self, data: list[dict], bar_type: BarType) -> list[Bar]:
        """Parse raw data into Bar objects."""
        bars = []

        instrument = None
        if self._instrument_provider:
            instrument = self._instrument_provider.get_instrument(bar_type.instrument_id)

        price_precision = 2
        size_precision = 0

        if instrument:
            price_precision = instrument.price_precision
            size_precision = instrument.size_precision

        for item in data:
            try:
                open_price = float(item.get("no", 0))
                high_price = float(item.get("nh", 0))
                low_price = float(item.get("nl", 0))
                close_price = float(item.get("nc", 0))
                volume = int(item.get("v", 0))

                if any(p <= 0 for p in [open_price, high_price, low_price, close_price]):
                    continue

                timestamp_str = item.get("time") or item.get("t")
                if not timestamp_str:
                    continue

                ts_event = self._parse_timestamp(timestamp_str)
                if ts_event == 0:
                    continue

                bar = Bar(
                    bar_type=bar_type,
                    open=Price(Decimal(str(open_price)), price_precision),
                    high=Price(Decimal(str(high_price)), price_precision),
                    low=Price(Decimal(str(low_price)), price_precision),
                    close=Price(Decimal(str(close_price)), price_precision),
                    volume=Quantity(Decimal(str(volume)), size_precision),
                    ts_event=ts_event,
                    ts_init=ts_event,
                )
                bars.append(bar)

            except Exception as e:
                self._log.debug(f"Error parsing bar: {e}")

        bars.sort(key=lambda b: b.ts_event)
        return bars

    def _parse_timestamp(self, timestamp_str: str) -> int:
        """Parse timestamp to nanoseconds."""
        try:
            for fmt in ["%Y-%m-%d %H:%M:%S", "%d-%m-%Y %H:%M:%S", "%Y-%m-%d", "%d-%m-%Y"]:
                try:
                    dt = datetime.strptime(timestamp_str, fmt)
                    return int(dt.timestamp() * 1e9)
                except ValueError:
                    continue
            return 0
        except Exception:
            return 0

    def _bar_spec_to_interval(self, bar_spec: BarSpecification) -> str:
        """Convert bar spec to Neo interval."""
        if bar_spec.aggregation == BarAggregation.DAY:
            return "D"
        elif bar_spec.aggregation == BarAggregation.MINUTE:
            if bar_spec.step <= 1:
                return "1"
            elif bar_spec.step <= 5:
                return "5"
            elif bar_spec.step <= 15:
                return "15"
            else:
                return "30"
        return "D"

    def _venue_to_exchange(self, venue: Venue) -> str:
        """Convert venue to Neo segment."""
        mapping = {"NSE": "nse_cm", "BSE": "bse_cm", "NFO": "nse_fo", "BFO": "bse_fo"}
        return mapping.get(venue.value, "nse_cm")

    async def _rate_limit(self) -> None:
        """Apply rate limiting."""
        if self._last_request_time:
            elapsed = (datetime.now() - self._last_request_time).total_seconds()
            if elapsed < self._min_request_interval:
                await asyncio.sleep(self._min_request_interval - elapsed)
        self._last_request_time = datetime.now()
