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
Instrument providers for ICICI Direct Breeze integration.

This module provides instrument discovery and management for Indian equity
and derivatives markets via ICICI Direct's Breeze API.
"""

from __future__ import annotations

import asyncio
from datetime import date, datetime, timedelta
from decimal import Decimal
from typing import Any

from nautilus_trader.adapters.icici.config import ICICIInstrumentProviderConfig
from nautilus_trader.common.component import Logger
from nautilus_trader.model.enums import AssetClass
from nautilus_trader.model.enums import InstrumentClass
from nautilus_trader.model.enums import OptionKind
from nautilus_trader.model.identifiers import InstrumentId
from nautilus_trader.model.identifiers import Symbol
from nautilus_trader.model.identifiers import Venue
from nautilus_trader.model.instruments import Equity
from nautilus_trader.model.instruments import FuturesContract
from nautilus_trader.model.instruments import Instrument
from nautilus_trader.model.instruments import OptionsContract
from nautilus_trader.model.objects import Currency
from nautilus_trader.model.objects import Money
from nautilus_trader.model.objects import Price
from nautilus_trader.model.objects import Quantity


# Indian Rupee currency
INR = Currency.from_str("INR")

# Venue mappings for ICICI Direct
NSE = Venue("NSE")
BSE = Venue("BSE")
NFO = Venue("NFO")  # NSE F&O
BFO = Venue("BFO")  # BSE F&O


class ICICIInstrumentProvider:
    """
    Provides instrument discovery and management for ICICI Direct markets.

    This provider handles the loading and caching of instruments from NSE, BSE,
    and derivatives segments via the Breeze API.

    Parameters
    ----------
    client : Any
        The ICICI Direct Breeze client.
    config : ICICIInstrumentProviderConfig
        The configuration for the provider.
    logger : Logger
        The logger instance.
    """

    def __init__(
        self,
        client: Any,
        config: ICICIInstrumentProviderConfig,
        logger: Logger,
    ) -> None:
        self._client = client
        self._config = config
        self._log = logger

        # Instrument caches
        self._instruments: dict[InstrumentId, Instrument] = {}
        self._token_to_instrument: dict[str, InstrumentId] = {}
        self._symbol_to_token: dict[str, str] = {}

        # Options chain cache
        self._options_chains: dict[str, dict[date, dict[float, dict[str, InstrumentId]]]] = {}

        self._is_loaded = False

    # =========================================================================
    # Public API
    # =========================================================================

    async def load_instruments(self) -> None:
        """Load instruments according to configuration."""
        if self._is_loaded:
            self._log.warning("Instruments already loaded")
            return

        self._log.info("Loading instruments from ICICI Direct...")
        start_time = datetime.now()

        try:
            # Load instruments based on configuration
            if self._config.load_all_instruments:
                await self._load_all_instruments()
            else:
                if self._config.load_nse_equity:
                    await self._load_nse_equity()

                if self._config.load_nse_fno:
                    await self._load_nse_fno()

                if self._config.load_bse_equity:
                    await self._load_bse_equity()

                if self._config.load_bse_fno:
                    await self._load_bse_fno()

            load_time = datetime.now() - start_time
            self._log.info(
                f"Loaded {len(self._instruments)} instruments in {load_time.total_seconds():.2f}s"
            )
            self._is_loaded = True

        except Exception as e:
            self._log.error(f"Failed to load instruments: {e}")
            raise

    def get_instrument(self, instrument_id: InstrumentId) -> Instrument | None:
        """
        Get an instrument by ID.

        Parameters
        ----------
        instrument_id : InstrumentId
            The instrument ID.

        Returns
        -------
        Instrument | None
            The instrument, or None if not found.
        """
        return self._instruments.get(instrument_id)

    def get_instrument_token(self, instrument_id: InstrumentId) -> str | None:
        """
        Get the ICICI stock token for an instrument.

        Parameters
        ----------
        instrument_id : InstrumentId
            The instrument ID.

        Returns
        -------
        str | None
            The stock token, or None if not found.
        """
        symbol_key = f"{instrument_id.symbol.value}.{instrument_id.venue.value}"
        return self._symbol_to_token.get(symbol_key)

    def get_instrument_id(self, token: str) -> InstrumentId | None:
        """
        Get the instrument ID for an ICICI stock token.

        Parameters
        ----------
        token : str
            The ICICI stock token.

        Returns
        -------
        InstrumentId | None
            The instrument ID, or None if not found.
        """
        return self._token_to_instrument.get(token)

    def get_options_chain(
        self,
        underlying: str,
        expiry: date,
    ) -> dict[float, dict[str, InstrumentId]] | None:
        """
        Get options chain for underlying and expiry.

        Parameters
        ----------
        underlying : str
            The underlying symbol (e.g., "NIFTY", "BANKNIFTY").
        expiry : date
            The expiry date.

        Returns
        -------
        dict[float, dict[str, InstrumentId]] | None
            Options chain mapping strike -> {CE: id, PE: id}, or None.
        """
        if underlying not in self._options_chains:
            return None
        return self._options_chains[underlying].get(expiry)

    def list_instruments(
        self,
        venue: Venue | None = None,
        instrument_class: InstrumentClass | None = None,
    ) -> list[Instrument]:
        """
        List instruments with optional filtering.

        Parameters
        ----------
        venue : Venue | None
            Filter by venue.
        instrument_class : InstrumentClass | None
            Filter by instrument class.

        Returns
        -------
        list[Instrument]
            The filtered instruments.
        """
        instruments = list(self._instruments.values())

        if venue:
            instruments = [i for i in instruments if i.id.venue == venue]

        if instrument_class:
            instruments = [i for i in instruments if i.instrument_class == instrument_class]

        return instruments

    # =========================================================================
    # Private Loading Methods
    # =========================================================================

    async def _load_all_instruments(self) -> None:
        """Load all available instruments."""
        await self._load_nse_equity()
        await self._load_nse_fno()
        await self._load_bse_equity()
        await self._load_bse_fno()

    async def _load_nse_equity(self) -> None:
        """Load NSE equity instruments."""
        self._log.info("Loading NSE equity instruments...")

        try:
            # Use Breeze API to get NSE instruments
            # The actual implementation depends on Breeze SDK capabilities
            response = await self._fetch_instruments("NSE", "cash")

            if not response:
                self._log.warning("No NSE equity instruments returned")
                return

            count = 0
            for item in response:
                try:
                    instrument = self._create_equity(item, NSE)
                    if instrument:
                        self._add_instrument(instrument, item.get("stock_token", ""))
                        count += 1
                except Exception as e:
                    self._log.debug(f"Error creating equity instrument: {e}")

            self._log.info(f"Loaded {count} NSE equity instruments")

        except Exception as e:
            self._log.error(f"Failed to load NSE equity: {e}")

    async def _load_nse_fno(self) -> None:
        """Load NSE F&O instruments (futures and options)."""
        self._log.info("Loading NSE F&O instruments...")

        try:
            # Load futures
            futures_response = await self._fetch_instruments("NFO", "futures")
            if futures_response:
                count = 0
                for item in futures_response:
                    try:
                        instrument = self._create_futures(item, NFO)
                        if instrument:
                            self._add_instrument(instrument, item.get("stock_token", ""))
                            count += 1
                    except Exception as e:
                        self._log.debug(f"Error creating futures instrument: {e}")
                self._log.info(f"Loaded {count} NSE futures instruments")

            # Load options
            options_response = await self._fetch_instruments("NFO", "options")
            if options_response:
                count = 0
                for item in options_response:
                    try:
                        instrument = self._create_option(item, NFO)
                        if instrument:
                            self._add_instrument(instrument, item.get("stock_token", ""))
                            self._add_to_options_chain(instrument, item)
                            count += 1
                    except Exception as e:
                        self._log.debug(f"Error creating option instrument: {e}")
                self._log.info(f"Loaded {count} NSE options instruments")

        except Exception as e:
            self._log.error(f"Failed to load NSE F&O: {e}")

    async def _load_bse_equity(self) -> None:
        """Load BSE equity instruments."""
        self._log.info("Loading BSE equity instruments...")

        try:
            response = await self._fetch_instruments("BSE", "cash")

            if not response:
                self._log.warning("No BSE equity instruments returned")
                return

            count = 0
            for item in response:
                try:
                    instrument = self._create_equity(item, BSE)
                    if instrument:
                        self._add_instrument(instrument, item.get("stock_token", ""))
                        count += 1
                except Exception as e:
                    self._log.debug(f"Error creating BSE equity: {e}")

            self._log.info(f"Loaded {count} BSE equity instruments")

        except Exception as e:
            self._log.error(f"Failed to load BSE equity: {e}")

    async def _load_bse_fno(self) -> None:
        """Load BSE F&O instruments."""
        self._log.info("Loading BSE F&O instruments...")

        try:
            # BSE F&O is less common, implement if needed
            self._log.info("BSE F&O loading not yet implemented")

        except Exception as e:
            self._log.error(f"Failed to load BSE F&O: {e}")

    async def _fetch_instruments(self, exchange: str, product_type: str) -> list[dict] | None:
        """
        Fetch instruments from Breeze API.

        Parameters
        ----------
        exchange : str
            Exchange code (NSE, BSE, NFO, BFO).
        product_type : str
            Product type (cash, futures, options).

        Returns
        -------
        list[dict] | None
            List of instrument data, or None if error.
        """
        try:
            # This will use the Breeze client to fetch instrument list
            # For now, return empty list as placeholder
            # Actual implementation will call self._client.get_names() or similar

            if hasattr(self._client, 'get_names'):
                response = self._client.get_names(
                    exchange_code=exchange,
                    product_type=product_type
                )
                if response and response.get('Success'):
                    return response.get('Success', [])

            return []

        except Exception as e:
            self._log.error(f"Error fetching instruments: {e}")
            return None

    # =========================================================================
    # Instrument Creation
    # =========================================================================

    def _create_equity(self, data: dict, venue: Venue) -> Equity | None:
        """
        Create an Equity instrument from Breeze data.

        Parameters
        ----------
        data : dict
            The instrument data from Breeze API.
        venue : Venue
            The venue (NSE or BSE).

        Returns
        -------
        Equity | None
            The equity instrument, or None if invalid.
        """
        try:
            symbol_str = data.get("stock_code") or data.get("symbol")
            if not symbol_str:
                return None

            instrument_id = InstrumentId(
                symbol=Symbol(symbol_str),
                venue=venue,
            )

            # Default lot size for Indian equities is 1
            lot_size = int(data.get("lot_size", 1))
            tick_size = float(data.get("tick_size", 0.05))

            return Equity(
                instrument_id=instrument_id,
                raw_symbol=Symbol(symbol_str),
                currency=INR,
                price_precision=2,
                price_increment=Price(Decimal(str(tick_size)), 2),
                lot_size=Quantity(Decimal(str(lot_size)), 0),
                ts_event=0,
                ts_init=0,
            )

        except Exception as e:
            self._log.debug(f"Error creating equity: {e}")
            return None

    def _create_futures(self, data: dict, venue: Venue) -> FuturesContract | None:
        """
        Create a FuturesContract from Breeze data.

        Parameters
        ----------
        data : dict
            The instrument data from Breeze API.
        venue : Venue
            The venue (NFO or BFO).

        Returns
        -------
        FuturesContract | None
            The futures contract, or None if invalid.
        """
        try:
            symbol_str = data.get("stock_code") or data.get("symbol")
            expiry_str = data.get("expiry_date")

            if not symbol_str or not expiry_str:
                return None

            # Check expiry filter
            expiry_date = self._parse_expiry(expiry_str)
            if expiry_date and self._config.filter_expiry_days:
                days_to_expiry = (expiry_date - date.today()).days
                if days_to_expiry > self._config.filter_expiry_days:
                    return None

            # Create symbol with expiry
            full_symbol = f"{symbol_str}{expiry_str.replace('-', '')}"

            instrument_id = InstrumentId(
                symbol=Symbol(full_symbol),
                venue=venue,
            )

            lot_size = int(data.get("lot_size", 1))
            tick_size = float(data.get("tick_size", 0.05))

            return FuturesContract(
                instrument_id=instrument_id,
                raw_symbol=Symbol(full_symbol),
                asset_class=AssetClass.INDEX if "NIFTY" in symbol_str else AssetClass.EQUITY,
                currency=INR,
                price_precision=2,
                price_increment=Price(Decimal(str(tick_size)), 2),
                multiplier=Quantity(Decimal(str(lot_size)), 0),
                lot_size=Quantity(Decimal(str(lot_size)), 0),
                underlying=symbol_str,
                activation_ns=0,
                expiration_ns=self._date_to_ns(expiry_date) if expiry_date else 0,
                ts_event=0,
                ts_init=0,
            )

        except Exception as e:
            self._log.debug(f"Error creating futures: {e}")
            return None

    def _create_option(self, data: dict, venue: Venue) -> OptionsContract | None:
        """
        Create an OptionsContract from Breeze data.

        Parameters
        ----------
        data : dict
            The instrument data from Breeze API.
        venue : Venue
            The venue (NFO or BFO).

        Returns
        -------
        OptionsContract | None
            The options contract, or None if invalid.
        """
        try:
            symbol_str = data.get("stock_code") or data.get("symbol")
            expiry_str = data.get("expiry_date")
            strike = data.get("strike_price")
            option_type = data.get("right") or data.get("option_type")

            if not all([symbol_str, expiry_str, strike, option_type]):
                return None

            # Check expiry filter
            expiry_date = self._parse_expiry(expiry_str)
            if expiry_date and self._config.filter_expiry_days:
                days_to_expiry = (expiry_date - date.today()).days
                if days_to_expiry > self._config.filter_expiry_days:
                    return None

            # Determine option kind
            option_kind = OptionKind.CALL if option_type.upper() in ["CE", "CALL", "C"] else OptionKind.PUT

            # Create symbol with strike and type
            strike_val = float(strike)
            option_suffix = "CE" if option_kind == OptionKind.CALL else "PE"
            full_symbol = f"{symbol_str}{expiry_str.replace('-', '')}{int(strike_val)}{option_suffix}"

            instrument_id = InstrumentId(
                symbol=Symbol(full_symbol),
                venue=venue,
            )

            lot_size = int(data.get("lot_size", 1))
            tick_size = float(data.get("tick_size", 0.05))

            return OptionsContract(
                instrument_id=instrument_id,
                raw_symbol=Symbol(full_symbol),
                asset_class=AssetClass.INDEX if "NIFTY" in symbol_str else AssetClass.EQUITY,
                currency=INR,
                price_precision=2,
                price_increment=Price(Decimal(str(tick_size)), 2),
                multiplier=Quantity(Decimal(str(lot_size)), 0),
                lot_size=Quantity(Decimal(str(lot_size)), 0),
                underlying=symbol_str,
                kind=option_kind,
                strike_price=Price(Decimal(str(strike_val)), 2),
                activation_ns=0,
                expiration_ns=self._date_to_ns(expiry_date) if expiry_date else 0,
                ts_event=0,
                ts_init=0,
            )

        except Exception as e:
            self._log.debug(f"Error creating option: {e}")
            return None

    # =========================================================================
    # Helper Methods
    # =========================================================================

    def _add_instrument(self, instrument: Instrument, token: str) -> None:
        """Add instrument to caches."""
        self._instruments[instrument.id] = instrument

        if token:
            self._token_to_instrument[token] = instrument.id
            symbol_key = f"{instrument.id.symbol.value}.{instrument.id.venue.value}"
            self._symbol_to_token[symbol_key] = token

    def _add_to_options_chain(self, instrument: OptionsContract, data: dict) -> None:
        """Add option to options chain cache."""
        try:
            underlying = instrument.underlying
            expiry_str = data.get("expiry_date")
            strike = float(data.get("strike_price", 0))

            if not all([underlying, expiry_str, strike]):
                return

            expiry = self._parse_expiry(expiry_str)
            if not expiry:
                return

            # Initialize chain structure
            if underlying not in self._options_chains:
                self._options_chains[underlying] = {}
            if expiry not in self._options_chains[underlying]:
                self._options_chains[underlying][expiry] = {}
            if strike not in self._options_chains[underlying][expiry]:
                self._options_chains[underlying][expiry][strike] = {}

            # Add to chain
            option_type = "CE" if instrument.kind == OptionKind.CALL else "PE"
            self._options_chains[underlying][expiry][strike][option_type] = instrument.id

        except Exception as e:
            self._log.debug(f"Error adding to options chain: {e}")

    def _parse_expiry(self, expiry_str: str) -> date | None:
        """Parse expiry date string to date object."""
        try:
            # Try common formats
            for fmt in ["%Y-%m-%d", "%d-%m-%Y", "%d%b%Y", "%Y%m%d"]:
                try:
                    return datetime.strptime(expiry_str, fmt).date()
                except ValueError:
                    continue
            return None
        except Exception:
            return None

    def _date_to_ns(self, dt: date) -> int:
        """Convert date to nanoseconds since epoch."""
        return int(datetime.combine(dt, datetime.min.time()).timestamp() * 1e9)
