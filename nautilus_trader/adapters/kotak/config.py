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
Configuration for the Kotak Neo adapter.
"""

from __future__ import annotations

from dataclasses import dataclass

from nautilus_trader.config import InstrumentProviderConfig
from nautilus_trader.config import LiveDataClientConfig
from nautilus_trader.config import LiveExecClientConfig
from nautilus_trader.config import NautilusConfig


@dataclass
class KotakConfig(NautilusConfig, frozen=True):
    """
    Configuration for Kotak Neo API integration.

    Parameters
    ----------
    consumer_key : str
        The Kotak Neo consumer key.
    consumer_secret : str
        The Kotak Neo consumer secret.
    access_token : str | None, default None
        The access token from OAuth login.
    mobile_number : str | None, default None
        The registered mobile number.
    password : str | None, default None
        The login password (for auto-login).
    mpin : str | None, default None
        The MPIN for 2FA.
    sandbox : bool, default True
        Whether to use sandbox/demo mode.
    request_timeout : float, default 30.0
        The request timeout in seconds.
    rate_limit_per_second : int, default 5
        Maximum number of API requests per second.
    max_retries : int, default 3
        Maximum number of retries for failed requests.
    debug_mode : bool, default False
        Enable detailed logging.
    """

    consumer_key: str
    consumer_secret: str
    access_token: str | None = None
    mobile_number: str | None = None
    password: str | None = None
    mpin: str | None = None
    sandbox: bool = True
    request_timeout: float = 30.0
    rate_limit_per_second: int = 5
    max_retries: int = 3
    debug_mode: bool = False

    def __post_init__(self):
        """Validate the configuration after initialization."""
        if not self.consumer_key:
            raise ValueError("consumer_key cannot be empty")
        if not self.consumer_secret:
            raise ValueError("consumer_secret cannot be empty")
        if self.request_timeout <= 0:
            raise ValueError("request_timeout must be positive")


@dataclass
class KotakDataClientConfig(LiveDataClientConfig, frozen=True):
    """
    Configuration for `KotakDataClient` instances.

    Parameters
    ----------
    consumer_key : str
        The Kotak Neo consumer key.
    consumer_secret : str
        The Kotak Neo consumer secret.
    access_token : str | None, default None
        The access token for authentication.
    sandbox : bool, default True
        Whether to use sandbox mode.
    handle_revised_bars : bool, default True
        Whether to handle revised bar data.
    debug_mode : bool, default False
        Enable debug logging.
    """

    consumer_key: str
    consumer_secret: str
    access_token: str | None = None
    sandbox: bool = True
    handle_revised_bars: bool = True
    debug_mode: bool = False

    def __post_init__(self):
        """Validate configuration."""
        if not self.consumer_key:
            raise ValueError("consumer_key cannot be empty")
        if not self.consumer_secret:
            raise ValueError("consumer_secret cannot be empty")


@dataclass
class KotakExecClientConfig(LiveExecClientConfig, frozen=True):
    """
    Configuration for `KotakExecutionClient` instances.

    Parameters
    ----------
    consumer_key : str
        The Kotak Neo consumer key.
    consumer_secret : str
        The Kotak Neo consumer secret.
    access_token : str | None, default None
        The access token for authentication.
    sandbox : bool, default True
        Whether to use sandbox mode.
    account_id : str | None, default None
        The trading account ID.
    debug_mode : bool, default False
        Enable debug logging.
    """

    consumer_key: str
    consumer_secret: str
    access_token: str | None = None
    sandbox: bool = True
    account_id: str | None = None
    debug_mode: bool = False

    def __post_init__(self):
        """Validate configuration."""
        if not self.consumer_key:
            raise ValueError("consumer_key cannot be empty")
        if not self.consumer_secret:
            raise ValueError("consumer_secret cannot be empty")


@dataclass
class KotakInstrumentProviderConfig(InstrumentProviderConfig, frozen=True):
    """
    Configuration for `KotakInstrumentProvider` instances.

    Parameters
    ----------
    consumer_key : str
        The Kotak Neo consumer key.
    consumer_secret : str
        The Kotak Neo consumer secret.
    access_token : str | None, default None
        The access token for authentication.
    load_all_instruments : bool, default False
        Load all instruments on startup.
    load_nse_equity : bool, default True
        Load NSE equity instruments.
    load_nse_fno : bool, default True
        Load NSE F&O instruments.
    load_bse_equity : bool, default False
        Load BSE equity instruments.
    load_bse_fno : bool, default False
        Load BSE F&O instruments.
    filter_expiry_days : int | None, default 90
        Only load derivatives with expiry within this many days.
    """

    consumer_key: str
    consumer_secret: str
    access_token: str | None = None
    load_all_instruments: bool = False
    load_nse_equity: bool = True
    load_nse_fno: bool = True
    load_bse_equity: bool = False
    load_bse_fno: bool = False
    filter_expiry_days: int | None = 90

    def __post_init__(self):
        """Validate configuration."""
        if not self.consumer_key:
            raise ValueError("consumer_key cannot be empty")
        if not self.consumer_secret:
            raise ValueError("consumer_secret cannot be empty")


def create_kotak_config_from_env() -> KotakConfig:
    """
    Create Kotak configuration from environment variables.

    Expected environment variables:
    - KOTAK_CONSUMER_KEY: Consumer key
    - KOTAK_CONSUMER_SECRET: Consumer secret
    - KOTAK_ACCESS_TOKEN: Access token (optional)
    - KOTAK_MOBILE: Mobile number (optional)
    - KOTAK_SANDBOX: "true" or "false" (optional)

    Returns
    -------
    KotakConfig
        The configuration instance.
    """
    import os

    consumer_key = os.getenv("KOTAK_CONSUMER_KEY")
    if not consumer_key:
        raise ValueError("KOTAK_CONSUMER_KEY environment variable is required")

    consumer_secret = os.getenv("KOTAK_CONSUMER_SECRET")
    if not consumer_secret:
        raise ValueError("KOTAK_CONSUMER_SECRET environment variable is required")

    access_token = os.getenv("KOTAK_ACCESS_TOKEN")
    mobile_number = os.getenv("KOTAK_MOBILE")
    sandbox = os.getenv("KOTAK_SANDBOX", "true").lower() == "true"
    debug_mode = os.getenv("KOTAK_DEBUG", "false").lower() == "true"

    return KotakConfig(
        consumer_key=consumer_key,
        consumer_secret=consumer_secret,
        access_token=access_token,
        mobile_number=mobile_number,
        sandbox=sandbox,
        debug_mode=debug_mode,
    )
