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
Configuration for the Zerodha adapter.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from nautilus_trader.config import InstrumentProviderConfig
from nautilus_trader.config import LiveDataClientConfig
from nautilus_trader.config import LiveExecClientConfig
from nautilus_trader.config import NautilusConfig


@dataclass
class ZerodhaConfig(NautilusConfig, frozen=True):
    """
    Configuration for Zerodha Kite Connect API integration.
    
    Parameters
    ----------
    api_key : str
        The Zerodha API key from Kite Connect app.
    api_secret : str
        The Zerodha API secret from Kite Connect app.
    access_token : str | None, default None
        The access token for API authentication.
        If None, will need to be generated through login flow.
    sandbox : bool, default True
        Whether to use sandbox/demo mode.
    base_url : str, default "https://api.kite.trade"
        The base URL for REST API endpoints.
    websocket_url : str, default "wss://ws.kite.trade"
        The WebSocket URL for real-time data.
    request_timeout : float, default 30.0
        The request timeout in seconds.
    rate_limit_per_second : int, default 3
        Maximum number of API requests per second (Zerodha limit).
    max_retries : int, default 3
        Maximum number of retries for failed requests.
    debug_mode : bool, default False
        Enable detailed logging of API requests/responses.
    """
    
    api_key: str
    api_secret: str
    access_token: str | None = None
    sandbox: bool = True
    base_url: str = "https://api.kite.trade"
    websocket_url: str = "wss://ws.kite.trade" 
    request_timeout: float = 30.0
    rate_limit_per_second: int = 3
    max_retries: int = 3
    debug_mode: bool = False

    def __post_init__(self):
        """Validate the configuration after initialization."""
        if not self.api_key:
            raise ValueError("api_key cannot be empty")
        if not self.api_secret:
            raise ValueError("api_secret cannot be empty")
        if self.request_timeout <= 0:
            raise ValueError("request_timeout must be positive")
        if self.rate_limit_per_second <= 0:
            raise ValueError("rate_limit_per_second must be positive")

    @property
    def login_url(self) -> str:
        """
        Generate the login URL for manual access token generation.
        
        Returns
        -------
        str
            The Kite Connect login URL.
        """
        return f"https://kite.trade/connect/login?api_key={self.api_key}&v=3"


@dataclass
class ZerodhaDataClientConfig(LiveDataClientConfig, frozen=True):
    """
    Configuration for `ZerodhaDataClient` instances.
    
    Parameters
    ----------
    api_key : str
        The Zerodha API key.
    api_secret : str  
        The Zerodha API secret.
    access_token : str | None, default None
        The access token for authentication.
    sandbox : bool, default True
        Whether to use sandbox mode.
    handle_revised_bars : bool, default True
        Whether to handle revised bar data.
    debug_mode : bool, default False
        Enable debug logging.
    """
    
    api_key: str
    api_secret: str
    access_token: str | None = None
    sandbox: bool = True
    handle_revised_bars: bool = True
    debug_mode: bool = False

    def __post_init__(self):
        """Validate configuration."""
        if not self.api_key:
            raise ValueError("api_key cannot be empty")
        if not self.api_secret:
            raise ValueError("api_secret cannot be empty")


@dataclass  
class ZerodhaExecClientConfig(LiveExecClientConfig, frozen=True):
    """
    Configuration for `ZerodhaExecutionClient` instances.
    
    Parameters
    ----------
    api_key : str
        The Zerodha API key.
    api_secret : str
        The Zerodha API secret.
    access_token : str | None, default None
        The access token for authentication.
    sandbox : bool, default True
        Whether to use sandbox mode.
    account_id : str | None, default None
        The trading account ID. If None, will use default account.
    debug_mode : bool, default False
        Enable debug logging.
    """
    
    api_key: str
    api_secret: str  
    access_token: str | None = None
    sandbox: bool = True
    account_id: str | None = None
    debug_mode: bool = False

    def __post_init__(self):
        """Validate configuration."""
        if not self.api_key:
            raise ValueError("api_key cannot be empty")
        if not self.api_secret:
            raise ValueError("api_secret cannot be empty")


@dataclass
class ZerodhaInstrumentProviderConfig(InstrumentProviderConfig, frozen=True):
    """
    Configuration for `ZerodhaInstrumentProvider` instances.
    
    Parameters
    ----------
    api_key : str
        The Zerodha API key.
    api_secret : str
        The Zerodha API secret.
    access_token : str | None, default None
        The access token for authentication.
    load_all_instruments : bool, default False
        Load all instruments on startup. Can be memory intensive.
    load_nse_equity : bool, default True
        Load NSE equity instruments.
    load_nse_options : bool, default True
        Load NSE options instruments (NFO segment).
    load_bse_equity : bool, default False
        Load BSE equity instruments.
    load_bse_options : bool, default False  
        Load BSE options instruments (BFO segment).
    filter_expiry_days : int | None, default 90
        Only load options with expiry within this many days.
        If None, load all expiries.
    """
    
    api_key: str
    api_secret: str
    access_token: str | None = None
    load_all_instruments: bool = False
    load_nse_equity: bool = True
    load_nse_options: bool = True
    load_bse_equity: bool = False
    load_bse_options: bool = False
    filter_expiry_days: int | None = 90

    def __post_init__(self):
        """Validate configuration."""
        if not self.api_key:
            raise ValueError("api_key cannot be empty")
        if not self.api_secret:
            raise ValueError("api_secret cannot be empty")
        if self.filter_expiry_days is not None and self.filter_expiry_days <= 0:
            raise ValueError("filter_expiry_days must be positive")


def create_zerodha_config_from_env() -> ZerodhaConfig:
    """
    Create Zerodha configuration from environment variables.
    
    Expected environment variables:
    - ZERODHA_API_KEY: API key
    - ZERODHA_API_SECRET: API secret  
    - ZERODHA_ACCESS_TOKEN: Access token (optional)
    - ZERODHA_SANDBOX: "true" or "false" (optional, default "true")
    - ZERODHA_DEBUG: "true" or "false" (optional, default "false")
    
    Returns
    -------
    ZerodhaConfig
        The configuration instance.
        
    Raises
    ------
    ValueError
        If required environment variables are missing.
    """
    import os
    
    api_key = os.getenv("ZERODHA_API_KEY")
    if not api_key:
        raise ValueError("ZERODHA_API_KEY environment variable is required")
        
    api_secret = os.getenv("ZERODHA_API_SECRET") 
    if not api_secret:
        raise ValueError("ZERODHA_API_SECRET environment variable is required")
        
    access_token = os.getenv("ZERODHA_ACCESS_TOKEN")
    sandbox = os.getenv("ZERODHA_SANDBOX", "true").lower() == "true"
    debug_mode = os.getenv("ZERODHA_DEBUG", "false").lower() == "true"
    
    return ZerodhaConfig(
        api_key=api_key,
        api_secret=api_secret,
        access_token=access_token,
        sandbox=sandbox,
        debug_mode=debug_mode,
    )