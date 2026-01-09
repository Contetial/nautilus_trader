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
Configuration for the ICICI Direct Breeze adapter.
"""

from __future__ import annotations

from dataclasses import dataclass

from nautilus_trader.config import InstrumentProviderConfig
from nautilus_trader.config import LiveDataClientConfig
from nautilus_trader.config import LiveExecClientConfig
from nautilus_trader.config import NautilusConfig


@dataclass
class ICICIConfig(NautilusConfig, frozen=True):
    """
    Configuration for ICICI Direct Breeze API integration.

    Parameters
    ----------
    api_key : str
        The ICICI Direct API key from Breeze app.
    api_secret : str
        The ICICI Direct API secret from Breeze app.
    session_token : str | None, default None
        The session token obtained from web login.
        Required for trading operations.
    sandbox : bool, default True
        Whether to use sandbox/demo mode.
    request_timeout : float, default 30.0
        The request timeout in seconds.
    rate_limit_per_second : int, default 3
        Maximum number of API requests per second.
    max_retries : int, default 3
        Maximum number of retries for failed requests.
    debug_mode : bool, default False
        Enable detailed logging of API requests/responses.
    """

    api_key: str
    api_secret: str
    session_token: str | None = None
    sandbox: bool = True
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


@dataclass
class ICICIDataClientConfig(LiveDataClientConfig, frozen=True):
    """
    Configuration for `ICICIDataClient` instances.

    Parameters
    ----------
    api_key : str
        The ICICI Direct API key.
    api_secret : str
        The ICICI Direct API secret.
    session_token : str | None, default None
        The session token for authentication.
    sandbox : bool, default True
        Whether to use sandbox mode.
    handle_revised_bars : bool, default True
        Whether to handle revised bar data.
    debug_mode : bool, default False
        Enable debug logging.
    """

    api_key: str
    api_secret: str
    session_token: str | None = None
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
class ICICIExecClientConfig(LiveExecClientConfig, frozen=True):
    """
    Configuration for `ICICIExecutionClient` instances.

    Parameters
    ----------
    api_key : str
        The ICICI Direct API key.
    api_secret : str
        The ICICI Direct API secret.
    session_token : str | None, default None
        The session token for authentication.
    sandbox : bool, default True
        Whether to use sandbox mode.
    account_id : str | None, default None
        The trading account ID. If None, will use default account.
    debug_mode : bool, default False
        Enable debug logging.
    """

    api_key: str
    api_secret: str
    session_token: str | None = None
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
class ICICIInstrumentProviderConfig(InstrumentProviderConfig, frozen=True):
    """
    Configuration for `ICICIInstrumentProvider` instances.

    Parameters
    ----------
    api_key : str
        The ICICI Direct API key.
    api_secret : str
        The ICICI Direct API secret.
    session_token : str | None, default None
        The session token for authentication.
    load_all_instruments : bool, default False
        Load all instruments on startup. Can be memory intensive.
    load_nse_equity : bool, default True
        Load NSE equity instruments.
    load_nse_fno : bool, default True
        Load NSE F&O instruments (futures and options).
    load_bse_equity : bool, default False
        Load BSE equity instruments.
    load_bse_fno : bool, default False
        Load BSE F&O instruments.
    filter_expiry_days : int | None, default 90
        Only load derivatives with expiry within this many days.
        If None, load all expiries.
    """

    api_key: str
    api_secret: str
    session_token: str | None = None
    load_all_instruments: bool = False
    load_nse_equity: bool = True
    load_nse_fno: bool = True
    load_bse_equity: bool = False
    load_bse_fno: bool = False
    filter_expiry_days: int | None = 90

    def __post_init__(self):
        """Validate configuration."""
        if not self.api_key:
            raise ValueError("api_key cannot be empty")
        if not self.api_secret:
            raise ValueError("api_secret cannot be empty")
        if self.filter_expiry_days is not None and self.filter_expiry_days <= 0:
            raise ValueError("filter_expiry_days must be positive")


def create_icici_config_from_env() -> ICICIConfig:
    """
    Create ICICI configuration from environment variables.

    Expected environment variables:
    - ICICI_API_KEY: API key
    - ICICI_API_SECRET: API secret
    - ICICI_SESSION_TOKEN: Session token (optional)
    - ICICI_SANDBOX: "true" or "false" (optional, default "true")
    - ICICI_DEBUG: "true" or "false" (optional, default "false")

    Returns
    -------
    ICICIConfig
        The configuration instance.

    Raises
    ------
    ValueError
        If required environment variables are missing.
    """
    import os

    api_key = os.getenv("ICICI_API_KEY")
    if not api_key:
        raise ValueError("ICICI_API_KEY environment variable is required")

    api_secret = os.getenv("ICICI_API_SECRET")
    if not api_secret:
        raise ValueError("ICICI_API_SECRET environment variable is required")

    session_token = os.getenv("ICICI_SESSION_TOKEN")
    sandbox = os.getenv("ICICI_SANDBOX", "true").lower() == "true"
    debug_mode = os.getenv("ICICI_DEBUG", "false").lower() == "true"

    return ICICIConfig(
        api_key=api_key,
        api_secret=api_secret,
        session_token=session_token,
        sandbox=sandbox,
        debug_mode=debug_mode,
    )
