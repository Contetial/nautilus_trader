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
Instrument providers for Kotak Neo integration.

This module provides instrument discovery and management for Indian equity
and derivatives markets via Kotak Securities' Neo API.
"""

from __future__ import annotations

from datetime import date, datetime
from decimal import Decimal
from typing import Any

from nautilus_trader.adapters.kotak.config import KotakInstrumentProviderConfig
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
from nautilus_trader.model.objects import Price
from nautilus_trader.model.objects import Quantity


# Indian Rupee currency
INR = Currency.from_str("INR")

# Venue mappings
NSE = Venue("NSE")
BSE = Venue("BSE")
NFO = Venue("NFO")
BFO = Venue("BFO")


class KotakInstrumentProvider:
    """
    Provides instrument discovery and management for Kotak Neo markets.

    Parameters
    ----------
    client : Any
        The Kotak Neo API client.
    config : KotakInstrumentProviderConfig
        The configuration for the provider.
    logger : Logger
        The logger instance.
    """

    def __init__(
        self,
        client: Any,
        config: KotakInstrumentProviderConfig,
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

    async def load_instruments(self) -> None:
        """Load instruments according to configuration."""
        if self._is_loaded:
            self._log.warning("Instruments already loaded")
            return

        self._log.info("Loading instruments from Kotak Neo...")
        start_time = datetime.now()

        try:
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
        """Get an instrument by ID."""
        return self._instruments.get(instrument_id)

    def get_instrument_token(self, instrument_id: InstrumentId) -> str | None:
        """Get the Kotak token for an instrument."""
        symbol_key = f"{instrument_id.symbol.value}.{instrument_id.venue.value}"
        return self._symbol_to_token.get(symbol_key)

    def get_instrument_id(self, token: str) -> InstrumentId | None:
        """Get the instrument ID for a Kotak token."""
        return self._token_to_instrument.get(token)

    def list_instruments(
        self,
        venue: Venue | None = None,
        instrument_class: InstrumentClass | None = None,
    ) -> list[Instrument]:
        """List instruments with optional filtering."""
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
            response = await self._fetch_instruments("nse_cm")
            if not response:
                return

            count = 0
            for item in response:
                try:
                    instrument = self._create_equity(item, NSE)
                    if instrument:
                        self._add_instrument(instrument, item.get("pSymbol", ""))
                        count += 1
                except Exception as e:
                    self._log.debug(f"Error creating equity: {e}")

            self._log.info(f"Loaded {count} NSE equity instruments")

        except Exception as e:
            self._log.error(f"Failed to load NSE equity: {e}")

    async def _load_nse_fno(self) -> None:
        """Load NSE F&O instruments."""
        self._log.info("Loading NSE F&O instruments...")

        try:
            # Load futures
            futures_response = await self._fetch_instruments("nse_fo", "FUTIDX")
            if futures_response:
                count = 0
                for item in futures_response:
                    try:
                        instrument = self._create_futures(item, NFO)
                        if instrument:
                            self._add_instrument(instrument, item.get("pSymbol", ""))
                            count += 1
                    except Exception as e:
                        self._log.debug(f"Error creating futures: {e}")
                self._log.info(f"Loaded {count} NSE futures")

            # Load options
            options_response = await self._fetch_instruments("nse_fo", "OPTIDX")
            if options_response:
                count = 0
                for item in options_response:
                    try:
                        instrument = self._create_option(item, NFO)
                        if instrument:
                            self._add_instrument(instrument, item.get("pSymbol", ""))
                            self._add_to_options_chain(instrument, item)
                            count += 1
                    except Exception as e:
                        self._log.debug(f"Error creating option: {e}")
                self._log.info(f"Loaded {count} NSE options")

        except Exception as e:
            self._log.error(f"Failed to load NSE F&O: {e}")

    async def _load_bse_equity(self) -> None:
        """Load BSE equity instruments."""
        self._log.info("Loading BSE equity instruments...")

        try:
            response = await self._fetch_instruments("bse_cm")
            if not response:
                return

            count = 0
            for item in response:
                try:
                    instrument = self._create_equity(item, BSE)
                    if instrument:
                        self._add_instrument(instrument, item.get("pSymbol", ""))
                        count += 1
                except Exception as e:
                    self._log.debug(f"Error creating BSE equity: {e}")

            self._log.info(f"Loaded {count} BSE equity instruments")

        except Exception as e:
            self._log.error(f"Failed to load BSE equity: {e}")

    async def _load_bse_fno(self) -> None:
        """Load BSE F&O instruments."""
        self._log.info("BSE F&O loading not yet implemented")

    async def _fetch_instruments(self, segment: str, inst_type: str | None = None) -> list[dict] | None:
        """Fetch instruments from Neo API."""
        try:
            if hasattr(self._client, 'scrip_master'):
                response = self._client.scrip_master(exchange_segment=segment)
                if response and isinstance(response, list):
                    if inst_type:
                        return [i for i in response if i.get("pInstType") == inst_type]
                    return response
            return []
        except Exception as e:
            self._log.error(f"Error fetching instruments: {e}")
            return None

    def _create_equity(self, data: dict, venue: Venue) -> Equity | None:
        """Create an Equity instrument from Neo data."""
        try:
            symbol_str = data.get("pTrdSymbol") or data.get("pSymbol")
            if not symbol_str:
                return None

            instrument_id = InstrumentId(symbol=Symbol(symbol_str), venue=venue)
            lot_size = int(data.get("pLotSize", 1))
            tick_size = float(data.get("pTickSize", 0.05))

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
        """Create a FuturesContract from Neo data."""
        try:
            symbol_str = data.get("pTrdSymbol") or data.get("pSymbol")
            expiry_str = data.get("pExpiryDate")

            if not symbol_str or not expiry_str:
                return None

            expiry_date = self._parse_expiry(expiry_str)
            if expiry_date and self._config.filter_expiry_days:
                days_to_expiry = (expiry_date - date.today()).days
                if days_to_expiry > self._config.filter_expiry_days:
                    return None

            instrument_id = InstrumentId(symbol=Symbol(symbol_str), venue=venue)
            lot_size = int(data.get("pLotSize", 1))
            tick_size = float(data.get("pTickSize", 0.05))

            return FuturesContract(
                instrument_id=instrument_id,
                raw_symbol=Symbol(symbol_str),
                asset_class=AssetClass.INDEX if "NIFTY" in symbol_str else AssetClass.EQUITY,
                currency=INR,
                price_precision=2,
                price_increment=Price(Decimal(str(tick_size)), 2),
                multiplier=Quantity(Decimal(str(lot_size)), 0),
                lot_size=Quantity(Decimal(str(lot_size)), 0),
                underlying=data.get("pUnderlying", symbol_str),
                activation_ns=0,
                expiration_ns=self._date_to_ns(expiry_date) if expiry_date else 0,
                ts_event=0,
                ts_init=0,
            )
        except Exception as e:
            self._log.debug(f"Error creating futures: {e}")
            return None

    def _create_option(self, data: dict, venue: Venue) -> OptionsContract | None:
        """Create an OptionsContract from Neo data."""
        try:
            symbol_str = data.get("pTrdSymbol") or data.get("pSymbol")
            expiry_str = data.get("pExpiryDate")
            strike = data.get("pStrikePrice")
            option_type = data.get("pOptionType")

            if not all([symbol_str, expiry_str, strike, option_type]):
                return None

            expiry_date = self._parse_expiry(expiry_str)
            if expiry_date and self._config.filter_expiry_days:
                days_to_expiry = (expiry_date - date.today()).days
                if days_to_expiry > self._config.filter_expiry_days:
                    return None

            option_kind = OptionKind.CALL if option_type.upper() in ["CE", "CALL", "C"] else OptionKind.PUT

            instrument_id = InstrumentId(symbol=Symbol(symbol_str), venue=venue)
            lot_size = int(data.get("pLotSize", 1))
            tick_size = float(data.get("pTickSize", 0.05))

            return OptionsContract(
                instrument_id=instrument_id,
                raw_symbol=Symbol(symbol_str),
                asset_class=AssetClass.INDEX if "NIFTY" in symbol_str else AssetClass.EQUITY,
                currency=INR,
                price_precision=2,
                price_increment=Price(Decimal(str(tick_size)), 2),
                multiplier=Quantity(Decimal(str(lot_size)), 0),
                lot_size=Quantity(Decimal(str(lot_size)), 0),
                underlying=data.get("pUnderlying", symbol_str),
                kind=option_kind,
                strike_price=Price(Decimal(str(float(strike))), 2),
                activation_ns=0,
                expiration_ns=self._date_to_ns(expiry_date) if expiry_date else 0,
                ts_event=0,
                ts_init=0,
            )
        except Exception as e:
            self._log.debug(f"Error creating option: {e}")
            return None

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
            expiry_str = data.get("pExpiryDate")
            strike = float(data.get("pStrikePrice", 0))

            if not all([underlying, expiry_str, strike]):
                return

            expiry = self._parse_expiry(expiry_str)
            if not expiry:
                return

            if underlying not in self._options_chains:
                self._options_chains[underlying] = {}
            if expiry not in self._options_chains[underlying]:
                self._options_chains[underlying][expiry] = {}
            if strike not in self._options_chains[underlying][expiry]:
                self._options_chains[underlying][expiry][strike] = {}

            option_type = "CE" if instrument.kind == OptionKind.CALL else "PE"
            self._options_chains[underlying][expiry][strike][option_type] = instrument.id
        except Exception as e:
            self._log.debug(f"Error adding to options chain: {e}")

    def _parse_expiry(self, expiry_str: str) -> date | None:
        """Parse expiry date string."""
        try:
            for fmt in ["%Y-%m-%d", "%d-%m-%Y", "%d%b%Y", "%Y%m%d", "%d-%b-%Y"]:
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
