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
Historical data client for ICICI Direct Breeze.

This module provides historical OHLCV data fetching from ICICI Direct's Breeze API
for backtesting purposes. Supports multiple timeframes and instrument types.
"""

from __future__ import annotations

import asyncio
from datetime import datetime, date, timedelta
from decimal import Decimal
from typing import Any

from nautilus_trader.adapters.icici.config import ICICIConfig
from nautilus_trader.adapters.icici.providers import ICICIInstrumentProvider
from nautilus_trader.common.component import Logger
from nautilus_trader.model.data import Bar
from nautilus_trader.model.data import BarType
from nautilus_trader.model.data import BarSpecification
from nautilus_trader.model.enums import BarAggregation
from nautilus_trader.model.enums import PriceType
from nautilus_trader.model.identifiers import InstrumentId
from nautilus_trader.model.identifiers import Symbol
from nautilus_trader.model.identifiers import Venue
from nautilus_trader.model.objects import Price
from nautilus_trader.model.objects import Quantity


class ICICIHistoricalClient:
    """
    Provides historical data fetching from ICICI Direct Breeze API.

    This client is designed for backtesting, fetching OHLCV data for specified
    instruments and date ranges.

    Parameters
    ----------
    client : Any
        The Breeze API client.
    config : ICICIConfig
        The ICICI configuration.
    logger : Logger
        The logger instance.
    instrument_provider : ICICIInstrumentProvider | None
        The instrument provider for instrument metadata.
    """

    def __init__(
        self,
        client: Any,
        config: ICICIConfig,
        logger: Logger,
        instrument_provider: ICICIInstrumentProvider | None = None,
    ) -> None:
        self._client = client
        self._config = config
        self._log = logger
        self._instrument_provider = instrument_provider

        # Rate limiting
        self._last_request_time: datetime | None = None
        self._min_request_interval = 1.0 / config.rate_limit_per_second

    async def connect(self) -> None:
        """Connect to ICICI Direct API."""
        try:
            self._log.info("Connecting to ICICI Direct for historical data...")

            if self._config.session_token:
                self._client.generate_session(
                    api_secret=self._config.api_secret,
                    session_token=self._config.session_token,
                )

            self._log.info("Connected to ICICI Direct historical data service")

        except Exception as e:
            self._log.error(f"Failed to connect: {e}")
            raise

    async def disconnect(self) -> None:
        """Disconnect from ICICI Direct API."""
        self._log.info("Disconnected from ICICI Direct historical data service")

    async def request_bars(
        self,
        instrument_id: InstrumentId,
        bar_spec: BarSpecification,
        start: datetime,
        end: datetime,
    ) -> list[Bar]:
        """
        Request historical bars for an instrument.

        Parameters
        ----------
        instrument_id : InstrumentId
            The instrument to fetch data for.
        bar_spec : BarSpecification
            The bar specification (timeframe).
        start : datetime
            The start date/time.
        end : datetime
            The end date/time.

        Returns
        -------
        list[Bar]
            List of historical bars.
        """
        try:
            self._log.info(
                f"Requesting bars for {instrument_id} from {start} to {end}"
            )

            # Rate limiting
            await self._rate_limit()

            # Map bar spec to Breeze interval
            interval = self._bar_spec_to_interval(bar_spec)

            # Determine exchange and product type
            symbol = str(instrument_id.symbol)
            exchange = self._venue_to_exchange(instrument_id.venue)
            product_type = self._get_product_type(instrument_id)

            # Fetch data from Breeze API
            response = self._client.get_historical_data_v2(
                interval=interval,
                from_date=start.strftime("%Y-%m-%dT%H:%M:%S.000Z"),
                to_date=end.strftime("%Y-%m-%dT%H:%M:%S.000Z"),
                stock_code=symbol,
                exchange_code=exchange,
                product_type=product_type,
            )

            if not response:
                self._log.warning(f"No response for {instrument_id}")
                return []

            if response.get("Error"):
                self._log.error(f"API error: {response.get('Error')}")
                return []

            data = response.get("Success", [])
            if not data:
                self._log.warning(f"No data returned for {instrument_id}")
                return []

            # Parse bars
            bar_type = BarType(
                instrument_id=instrument_id,
                bar_spec=bar_spec,
            )

            bars = self._parse_bars(data, bar_type)
            self._log.info(f"Received {len(bars)} bars for {instrument_id}")

            return bars

        except Exception as e:
            self._log.error(f"Failed to request bars: {e}")
            return []

    async def request_bars_batch(
        self,
        instrument_ids: list[InstrumentId],
        bar_spec: BarSpecification,
        start: datetime,
        end: datetime,
    ) -> dict[InstrumentId, list[Bar]]:
        """
        Request historical bars for multiple instruments.

        Parameters
        ----------
        instrument_ids : list[InstrumentId]
            The instruments to fetch data for.
        bar_spec : BarSpecification
            The bar specification.
        start : datetime
            The start date/time.
        end : datetime
            The end date/time.

        Returns
        -------
        dict[InstrumentId, list[Bar]]
            Dictionary mapping instrument IDs to their bars.
        """
        results: dict[InstrumentId, list[Bar]] = {}

        for instrument_id in instrument_ids:
            bars = await self.request_bars(instrument_id, bar_spec, start, end)
            results[instrument_id] = bars

        return results

    async def request_daily_bars(
        self,
        instrument_id: InstrumentId,
        start_date: date,
        end_date: date,
    ) -> list[Bar]:
        """
        Request daily OHLCV bars for an instrument.

        Parameters
        ----------
        instrument_id : InstrumentId
            The instrument to fetch data for.
        start_date : date
            The start date.
        end_date : date
            The end date.

        Returns
        -------
        list[Bar]
            List of daily bars.
        """
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
        """
        Request intraday OHLCV bars for an instrument.

        Parameters
        ----------
        instrument_id : InstrumentId
            The instrument to fetch data for.
        interval_minutes : int
            The bar interval in minutes (1, 5, 15, 30).
        start : datetime
            The start date/time.
        end : datetime
            The end date/time.

        Returns
        -------
        list[Bar]
            List of intraday bars.
        """
        bar_spec = BarSpecification(
            step=interval_minutes,
            aggregation=BarAggregation.MINUTE,
            price_type=PriceType.LAST,
        )

        return await self.request_bars(instrument_id, bar_spec, start, end)

    def get_available_intervals(self) -> list[str]:
        """
        Get available data intervals from Breeze API.

        Returns
        -------
        list[str]
            Available interval strings.
        """
        return [
            "1minute",
            "5minute",
            "30minute",
            "1day",
        ]

    # =========================================================================
    # Private Methods
    # =========================================================================

    def _parse_bars(self, data: list[dict], bar_type: BarType) -> list[Bar]:
        """Parse raw API data into Bar objects."""
        bars = []

        # Get instrument for precision
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
                # Parse OHLCV data
                open_price = float(item.get("open", 0))
                high_price = float(item.get("high", 0))
                low_price = float(item.get("low", 0))
                close_price = float(item.get("close", 0))
                volume = int(item.get("volume", 0))

                # Skip invalid bars
                if open_price <= 0 or high_price <= 0 or low_price <= 0 or close_price <= 0:
                    continue

                # Parse timestamp
                timestamp_str = item.get("datetime") or item.get("date")
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
                continue

        # Sort by timestamp
        bars.sort(key=lambda b: b.ts_event)

        return bars

    def _parse_timestamp(self, timestamp_str: str) -> int:
        """Parse timestamp string to nanoseconds since epoch."""
        try:
            # Try different formats
            formats = [
                "%Y-%m-%dT%H:%M:%S.%fZ",
                "%Y-%m-%dT%H:%M:%SZ",
                "%Y-%m-%d %H:%M:%S",
                "%Y-%m-%d",
            ]

            for fmt in formats:
                try:
                    dt = datetime.strptime(timestamp_str, fmt)
                    return int(dt.timestamp() * 1e9)
                except ValueError:
                    continue

            return 0

        except Exception:
            return 0

    def _bar_spec_to_interval(self, bar_spec: BarSpecification) -> str:
        """Convert bar specification to Breeze interval string."""
        if bar_spec.aggregation == BarAggregation.DAY:
            return "1day"
        elif bar_spec.aggregation == BarAggregation.MINUTE:
            if bar_spec.step <= 1:
                return "1minute"
            elif bar_spec.step <= 5:
                return "5minute"
            else:
                return "30minute"
        elif bar_spec.aggregation == BarAggregation.HOUR:
            return "30minute"
        else:
            return "1day"

    def _venue_to_exchange(self, venue: Venue) -> str:
        """Convert venue to Breeze exchange code."""
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

    async def _rate_limit(self) -> None:
        """Apply rate limiting between requests."""
        if self._last_request_time:
            elapsed = (datetime.now() - self._last_request_time).total_seconds()
            if elapsed < self._min_request_interval:
                await asyncio.sleep(self._min_request_interval - elapsed)

        self._last_request_time = datetime.now()


async def fetch_historical_data(
    api_key: str,
    api_secret: str,
    session_token: str,
    symbol: str,
    exchange: str,
    start_date: date,
    end_date: date,
    interval: str = "1day",
) -> list[dict]:
    """
    Convenience function to fetch historical data.

    Parameters
    ----------
    api_key : str
        ICICI Direct API key.
    api_secret : str
        ICICI Direct API secret.
    session_token : str
        Session token from login.
    symbol : str
        Stock symbol (e.g., "RELIANCE", "NIFTY").
    exchange : str
        Exchange code (NSE, BSE, NFO).
    start_date : date
        Start date for data.
    end_date : date
        End date for data.
    interval : str
        Data interval (1minute, 5minute, 30minute, 1day).

    Returns
    -------
    list[dict]
        List of OHLCV data dictionaries.
    """
    try:
        from breeze_connect import BreezeConnect

        # Initialize client
        client = BreezeConnect(api_key=api_key)
        client.generate_session(api_secret=api_secret, session_token=session_token)

        # Fetch data
        response = client.get_historical_data_v2(
            interval=interval,
            from_date=start_date.strftime("%Y-%m-%dT07:00:00.000Z"),
            to_date=end_date.strftime("%Y-%m-%dT07:00:00.000Z"),
            stock_code=symbol,
            exchange_code=exchange,
            product_type="cash" if exchange in ["NSE", "BSE"] else "futures",
        )

        if response and response.get("Success"):
            return response["Success"]

        return []

    except Exception as e:
        print(f"Error fetching historical data: {e}")
        return []
