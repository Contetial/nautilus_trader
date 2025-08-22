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
Instrument providers for Zerodha integration.

This module provides instrument discovery and management for Indian equity
and derivatives markets, with specialized support for options trading.
"""

from __future__ import annotations

import asyncio
from datetime import date, datetime, timedelta
from typing import Any

from nautilus_trader.adapters.zerodha.config import ZerodhaInstrumentProviderConfig
from nautilus_trader.common.component import Logger
from nautilus_trader.model.enums import AssetClass
from nautilus_trader.model.enums import InstrumentClass
from nautilus_trader.model.enums import OptionKind
from nautilus_trader.model.identifiers import InstrumentId
from nautilus_trader.model.identifiers import Symbol
from nautilus_trader.model.identifiers import Venue
from nautilus_trader.model.instruments import CurrencyPair
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

# Venue mappings
NSE = Venue("NSE")
BSE = Venue("BSE") 
NFO = Venue("NFO")  # NSE F&O
BFO = Venue("BFO")  # BSE F&O


class ZerodhaInstrumentProvider:
    """
    Provides instrument discovery and management for Zerodha markets.
    
    This provider handles the loading and caching of instruments from NSE, BSE,
    and derivatives segments, with special handling for options chains.
    
    Parameters
    ----------
    client : Any
        The Zerodha HTTP client.
    config : ZerodhaInstrumentProviderConfig
        The configuration for the provider.
    logger : Logger
        The logger instance.
    """

    def __init__(
        self,
        client: Any,  # TODO: Type with actual Rust client
        config: ZerodhaInstrumentProviderConfig,
        logger: Logger,
    ) -> None:
        self._client = client
        self._config = config
        self._log = logger
        
        # Instrument caches
        self._instruments: dict[InstrumentId, Instrument] = {}
        self._token_to_instrument: dict[int, InstrumentId] = {}
        self._symbol_to_token: dict[str, int] = {}
        
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

        self._log.info("Loading instruments from Zerodha...")
        start_time = datetime.now()
        
        try:
            # Load instruments based on configuration
            if self._config.load_all_instruments:
                await self._load_all_instruments()
            else:
                if self._config.load_nse_equity:
                    await self._load_nse_equity()
                
                if self._config.load_nse_options:
                    await self._load_nse_options()
                
                if self._config.load_bse_equity:
                    await self._load_bse_equity()
                    
                if self._config.load_bse_options:
                    await self._load_bse_options()

            load_time = datetime.now() - start_time
            self._log.info(
                f"✅ Loaded {len(self._instruments)} instruments in {load_time.total_seconds():.2f}s"
            )
            self._is_loaded = True
            
        except Exception as e:
            self._log.error(f"❌ Failed to load instruments: {e}")
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

    def get_instrument_by_token(self, token: int) -> Instrument | None:
        """
        Get an instrument by Zerodha token.
        
        Parameters
        ----------
        token : int
            The Zerodha instrument token.
            
        Returns
        -------
        Instrument | None  
            The instrument, or None if not found.
        """
        instrument_id = self._token_to_instrument.get(token)
        return self._instruments.get(instrument_id) if instrument_id else None

    def get_token_by_instrument_id(self, instrument_id: InstrumentId) -> int | None:
        """
        Get Zerodha token by instrument ID.
        
        Parameters
        ----------
        instrument_id : InstrumentId
            The instrument ID.
            
        Returns
        -------
        int | None
            The Zerodha token, or None if not found.
        """
        # Reverse lookup in token mapping
        for token, id_ in self._token_to_instrument.items():
            if id_ == instrument_id:
                return token
        return None

    def list_instruments(
        self,
        venue: Venue | None = None,
        instrument_class: InstrumentClass | None = None,
        asset_class: AssetClass | None = None,
    ) -> list[Instrument]:
        """
        List instruments with optional filters.
        
        Parameters
        ----------
        venue : Venue | None
            Filter by venue.
        instrument_class : InstrumentClass | None
            Filter by instrument class.
        asset_class : AssetClass | None
            Filter by asset class.
            
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
            
        if asset_class:
            instruments = [i for i in instruments if i.asset_class == asset_class]
            
        return instruments

    def get_options_chain(
        self,
        underlying: str,
        expiry: date | None = None,
    ) -> dict[float, dict[str, InstrumentId]]:
        """
        Get options chain for an underlying.
        
        Parameters
        ----------
        underlying : str
            The underlying symbol (e.g., "NIFTY", "BANKNIFTY").
        expiry : date | None
            The expiry date. If None, returns nearest expiry.
            
        Returns
        -------
        dict[float, dict[str, InstrumentId]]
            Options chain organized by strike -> {call_id, put_id}.
        """
        if underlying not in self._options_chains:
            return {}
            
        underlying_chains = self._options_chains[underlying]
        
        if expiry is None:
            # Get nearest expiry
            available_expiries = sorted(underlying_chains.keys())
            if not available_expiries:
                return {}
            expiry = available_expiries[0]
            
        return underlying_chains.get(expiry, {})

    def get_options_expiries(self, underlying: str) -> list[date]:
        """
        Get available expiry dates for an underlying.
        
        Parameters
        ----------
        underlying : str
            The underlying symbol.
            
        Returns
        -------
        list[date]
            Available expiry dates, sorted.
        """
        if underlying not in self._options_chains:
            return []
            
        return sorted(self._options_chains[underlying].keys())

    # =========================================================================
    # Private Loading Methods
    # =========================================================================

    async def _load_all_instruments(self) -> None:
        """Load all available instruments."""
        self._log.info("Loading all instruments...")
        
        try:
            # Fetch all instruments from Rust client
            instruments = await self._client.get_instruments()
            
            processed = 0
            for instrument_data in instruments:
                try:
                    instrument = self._create_instrument(instrument_data)
                    if instrument:
                        self._add_instrument(instrument, instrument_data.instrument_token)
                        processed += 1
                        
                        # Add to options chain if it's an option
                        if hasattr(instrument_data, 'instrument_type') and instrument_data.instrument_type in ('CE', 'PE'):
                            self._add_to_options_chain(instrument, instrument_data)
                            
                except Exception as e:
                    self._log.warning(f"Failed to process instrument {instrument_data.tradingsymbol}: {e}")
            
            self._log.info(f"Successfully processed {processed} instruments")
            
        except Exception as e:
            self._log.error(f"Failed to load instruments: {e}")
            raise

    async def _load_nse_equity(self) -> None:
        """Load NSE equity instruments."""
        self._log.info("Loading NSE equity instruments...")
        
        # TODO: Implement using Rust client
        # nse_instruments = await self._client.get_instruments_for_exchange("NSE")
        # equity_instruments = [i for i in nse_instruments if i.instrument_type == "EQ"]
        
        # for instrument_data in equity_instruments:
        #     instrument = self._create_equity_instrument(instrument_data)
        #     self._add_instrument(instrument, instrument_data.instrument_token)
        
        self._log.warning("NSE equity loading not yet implemented")

    async def _load_nse_options(self) -> None:
        """Load NSE options instruments (NFO segment)."""
        self._log.info("Loading NSE options instruments...")
        
        # TODO: Implement using Rust client
        # nfo_instruments = await self._client.get_instruments_for_exchange("NFO")
        # options_instruments = [i for i in nfo_instruments 
        #                       if i.instrument_type in ("CE", "PE")]
        
        # Apply expiry filter if configured
        cutoff_date = None
        if self._config.filter_expiry_days:
            cutoff_date = date.today() + timedelta(days=self._config.filter_expiry_days)
        
        # for instrument_data in options_instruments:
        #     if cutoff_date and instrument_data.expiry and instrument_data.expiry > cutoff_date:
        #         continue
        #         
        #     instrument = self._create_options_instrument(instrument_data)
        #     self._add_instrument(instrument, instrument_data.instrument_token)
        #     self._add_to_options_chain(instrument, instrument_data)
        
        self._log.warning("NSE options loading not yet implemented")

    async def _load_bse_equity(self) -> None:
        """Load BSE equity instruments."""
        self._log.info("Loading BSE equity instruments...")
        self._log.warning("BSE equity loading not yet implemented")

    async def _load_bse_options(self) -> None:
        """Load BSE options instruments (BFO segment)."""
        self._log.info("Loading BSE options instruments...")
        self._log.warning("BSE options loading not yet implemented")

    # =========================================================================
    # Instrument Creation
    # =========================================================================

    def _create_instrument(self, instrument_data: Any) -> Instrument | None:
        """
        Create an instrument from Zerodha data based on its type.
        
        Parameters
        ----------
        instrument_data : Any
            The Zerodha instrument data.
            
        Returns
        -------
        Instrument | None
            The created instrument, or None if unsupported type.
        """
        try:
            instrument_type = instrument_data.instrument_type
            
            if instrument_type == 'EQ':
                return self._create_equity_instrument(instrument_data)
            elif instrument_type in ('CE', 'PE'):
                return self._create_options_instrument(instrument_data)
            elif instrument_type == 'FUT':
                return self._create_futures_instrument(instrument_data)
            else:
                self._log.debug(f"Unsupported instrument type: {instrument_type}")
                return None
                
        except Exception as e:
            self._log.warning(f"Failed to create instrument {instrument_data.tradingsymbol}: {e}")
            return None

    def _create_equity_instrument(self, instrument_data: Any) -> Equity:
        """
        Create an equity instrument from Zerodha data.
        
        Parameters
        ----------
        instrument_data : Any
            The Zerodha instrument data.
            
        Returns
        -------
        Equity
            The created equity instrument.
        """
        # TODO: Implement using actual Zerodha instrument data structure
        return Equity(
            instrument_id=InstrumentId(
                Symbol(instrument_data.tradingsymbol),
                NSE if instrument_data.exchange == "NSE" else BSE,
            ),
            raw_symbol=Symbol(instrument_data.tradingsymbol),
            currency=INR,
            price_precision=2,
            price_increment=Price(instrument_data.tick_size, precision=2),
            lot_size=Quantity(instrument_data.lot_size, precision=0),
            isin=None,  # TODO: Get ISIN if available
            ts_event=0,  # TODO: Set proper timestamp
            ts_init=0,   # TODO: Set proper timestamp
        )

    def _create_options_instrument(self, instrument_data: Any) -> OptionsContract:
        """
        Create an options contract from Zerodha data.
        
        Parameters
        ----------
        instrument_data : Any
            The Zerodha instrument data.
            
        Returns
        -------
        OptionsContract
            The created options contract.
        """
        # Determine option kind
        option_kind = OptionKind.CALL if instrument_data.instrument_type == "CE" else OptionKind.PUT
        
        # TODO: Implement using actual Zerodha instrument data structure
        return OptionsContract(
            instrument_id=InstrumentId(
                Symbol(instrument_data.tradingsymbol),
                NFO if instrument_data.exchange == "NFO" else BFO,
            ),
            raw_symbol=Symbol(instrument_data.tradingsymbol),
            asset_class=AssetClass.INDEX if "NIFTY" in instrument_data.tradingsymbol else AssetClass.EQUITY,
            currency=INR,
            price_precision=2,
            price_increment=Price(instrument_data.tick_size, precision=2),
            lot_size=Quantity(instrument_data.lot_size, precision=0),
            underlying=instrument_data.name,  # TODO: Extract proper underlying
            option_kind=option_kind,
            strike_price=Price(instrument_data.strike, precision=2),
            expiry_date=instrument_data.expiry,
            ts_event=0,  # TODO: Set proper timestamp
            ts_init=0,   # TODO: Set proper timestamp
        )

    def _create_futures_instrument(self, instrument_data: Any) -> FuturesContract:
        """
        Create a futures contract from Zerodha data.
        
        Parameters
        ----------
        instrument_data : Any
            The Zerodha instrument data.
            
        Returns
        -------
        FuturesContract
            The created futures contract.
        """
        # TODO: Implement futures contract creation
        return FuturesContract(
            instrument_id=InstrumentId(
                Symbol(instrument_data.tradingsymbol),
                NFO if instrument_data.exchange == "NFO" else BFO,
            ),
            raw_symbol=Symbol(instrument_data.tradingsymbol),
            asset_class=AssetClass.INDEX if "NIFTY" in instrument_data.tradingsymbol else AssetClass.EQUITY,
            currency=INR,
            price_precision=2,
            price_increment=Price(instrument_data.tick_size, precision=2),
            lot_size=Quantity(instrument_data.lot_size, precision=0),
            underlying=instrument_data.name,
            expiry_date=instrument_data.expiry,
            ts_event=0,
            ts_init=0,
        )

    # =========================================================================
    # Helper Methods
    # =========================================================================

    def _add_instrument(self, instrument: Instrument, token: int) -> None:
        """
        Add an instrument to the provider's cache.
        
        Parameters
        ----------
        instrument : Instrument
            The instrument to add.
        token : int
            The Zerodha instrument token.
        """
        self._instruments[instrument.id] = instrument
        self._token_to_instrument[token] = instrument.id
        self._symbol_to_token[str(instrument.id.symbol)] = token

    def _add_to_options_chain(self, instrument: OptionsContract, instrument_data: Any) -> None:
        """
        Add an options contract to the options chain cache.
        
        Parameters
        ----------
        instrument : OptionsContract
            The options contract.
        instrument_data : Any
            The Zerodha instrument data.
        """
        # Extract underlying symbol
        underlying = self._extract_underlying_symbol(instrument_data.tradingsymbol)
        expiry = instrument_data.expiry
        strike = float(instrument_data.strike)
        
        # Initialize nested dictionaries
        if underlying not in self._options_chains:
            self._options_chains[underlying] = {}
        if expiry not in self._options_chains[underlying]:
            self._options_chains[underlying][expiry] = {}
        if strike not in self._options_chains[underlying][expiry]:
            self._options_chains[underlying][expiry][strike] = {}
        
        # Add call or put
        option_type = "call" if instrument.option_kind == OptionKind.CALL else "put"
        self._options_chains[underlying][expiry][strike][option_type] = instrument.id

    def get_instrument_token(self, instrument_id: InstrumentId) -> int:
        """
        Get Zerodha instrument token for an instrument ID.
        
        Parameters
        ----------
        instrument_id : InstrumentId
            The instrument ID.
            
        Returns
        -------
        int
            The Zerodha instrument token.
        """
        token = self.get_token_by_instrument_id(instrument_id)
        if token is None:
            raise ValueError(f"Token not found for instrument {instrument_id}")
        return token

    def get_instrument_id(self, token: int) -> InstrumentId:
        """
        Get instrument ID from Zerodha token.
        
        Parameters
        ----------
        token : int
            The Zerodha instrument token.
            
        Returns
        -------
        InstrumentId
            The instrument ID.
        """
        instrument_id = self._token_to_instrument.get(token)
        if instrument_id is None:
            raise ValueError(f"Instrument ID not found for token {token}")
        return instrument_id

    def _extract_underlying_symbol(self, trading_symbol: str) -> str:
        """
        Extract underlying symbol from trading symbol.
        
        Parameters
        ----------
        trading_symbol : str
            The trading symbol (e.g., "NIFTY24JAN18000CE").
            
        Returns
        -------
        str
            The underlying symbol (e.g., "NIFTY").
        """
        # TODO: Implement proper symbol parsing
        # This is a simplified version
        if trading_symbol.startswith("NIFTY"):
            if "BANK" in trading_symbol:
                return "BANKNIFTY"
            elif "FIN" in trading_symbol:
                return "FINNIFTY"
            else:
                return "NIFTY"
        
        # For individual stocks, extract the base symbol
        # This would need more sophisticated parsing for real implementation
        return trading_symbol.split("24")[0] if "24" in trading_symbol else trading_symbol

    async def close(self) -> None:
        """Close the instrument provider and clean up resources."""
        self._instruments.clear()
        self._token_to_instrument.clear()
        self._symbol_to_token.clear()
        self._options_chains.clear()
        self._is_loaded = False
        self._log.info("✅ ZerodhaInstrumentProvider closed")


# =========================================================================
# Helper Functions
# =========================================================================

def get_popular_nse_stocks() -> list[str]:
    """
    Get list of popular NSE stocks for instrument loading.
    
    Returns
    -------
    list[str]
        List of popular stock symbols.
    """
    return [
        "RELIANCE", "TCS", "HDFCBANK", "ICICIBANK", "HINDUNILVR",
        "INFY", "HDFC", "ITC", "SBIN", "BHARTIARTL", "KOTAKBANK",
        "LT", "ASIANPAINT", "MARUTI", "AXISBANK", "BAJFINANCE",
        "HCLTECH", "ULTRACEMCO", "WIPRO", "SUNPHARMA", "NESTLEIND",
        "TITAN", "POWERGRID", "M&M", "TECHM", "TATAMOTORS", "BAJAJFINSV",
        "ONGC", "NTPC", "DRREDDY", "JSWSTEEL", "CIPLA", "GRASIM",
        "COALINDIA", "BRITANNIA", "EICHERMOT", "ADANIPORTS", "HINDALCO",
        "SHREECEM", "DIVISLAB", "INDUSINDBK", "TATASTEEL", "APOLLOHOSP",
        "BPCL", "HEROMOTOCO", "BAJAJ-AUTO", "UPL", "SBILIFE"
    ]


def get_nifty_indices() -> list[str]:
    """
    Get list of major Nifty indices.
    
    Returns
    -------
    list[str]
        List of Nifty index symbols.
    """
    return [
        "NIFTY 50", "NIFTY BANK", "NIFTY FINANCIAL SERVICES",
        "NIFTY IT", "NIFTY FMCG", "NIFTY PHARMA", "NIFTY AUTO",
        "NIFTY METAL", "NIFTY REALTY", "NIFTY ENERGY", "NIFTY MIDCAP 50",
        "NIFTY SMALLCAP 50", "NIFTY NEXT 50", "NIFTY 100", "NIFTY 200"
    ]