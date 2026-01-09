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
Factory classes for creating ICICI Direct Breeze adapter components.

This module provides factory methods to create data clients, execution clients,
and instrument providers for the ICICI Direct integration.
"""

from __future__ import annotations

import asyncio
from typing import Any

from nautilus_trader.adapters.icici.config import ICICIDataClientConfig
from nautilus_trader.adapters.icici.config import ICICIExecClientConfig
from nautilus_trader.adapters.icici.config import ICICIInstrumentProviderConfig
from nautilus_trader.adapters.icici.data import ICICIDataClient
from nautilus_trader.adapters.icici.execution import ICICILiveExecClient
from nautilus_trader.adapters.icici.providers import ICICIInstrumentProvider
from nautilus_trader.cache.cache import Cache
from nautilus_trader.common.component import LiveClock
from nautilus_trader.common.component import Logger
from nautilus_trader.common.component import MessageBus
from nautilus_trader.live.data_client import LiveDataClient
from nautilus_trader.live.execution_client import LiveExecutionClient


def get_breeze_client(api_key: str) -> Any:
    """
    Create a Breeze Connect client instance.

    Parameters
    ----------
    api_key : str
        The ICICI Direct API key.

    Returns
    -------
    Any
        The BreezeConnect client instance.

    Raises
    ------
    ImportError
        If breeze_connect package is not installed.
    """
    try:
        from breeze_connect import BreezeConnect
        return BreezeConnect(api_key=api_key)
    except ImportError as e:
        raise ImportError(
            "The 'breeze_connect' package is required for ICICI Direct integration. "
            "Install it with: pip install breeze-connect"
        ) from e


class ICICILiveDataClientFactory:
    """
    Factory for creating ICICI Direct live data clients.

    This factory creates properly configured data clients for streaming
    market data from ICICI Direct's Breeze API.
    """

    @staticmethod
    def create(
        loop: asyncio.AbstractEventLoop,
        name: str,
        config: ICICIDataClientConfig,
        msgbus: MessageBus,
        cache: Cache,
        clock: LiveClock,
        logger: Logger | None = None,
    ) -> LiveDataClient:
        """
        Create a new ICICI Direct data client.

        Parameters
        ----------
        loop : asyncio.AbstractEventLoop
            The event loop for the client.
        name : str
            The client name.
        config : ICICIDataClientConfig
            The client configuration.
        msgbus : MessageBus
            The message bus.
        cache : Cache
            The cache.
        clock : LiveClock
            The clock.
        logger : Logger | None
            The logger (optional).

        Returns
        -------
        LiveDataClient
            The configured data client.
        """
        # Create Breeze client
        client = get_breeze_client(config.api_key)

        # Create instrument provider
        provider_config = ICICIInstrumentProviderConfig(
            api_key=config.api_key,
            api_secret=config.api_secret,
            session_token=config.session_token,
        )

        instrument_provider = ICICIInstrumentProvider(
            client=client,
            config=provider_config,
            logger=logger or Logger(name="ICICIInstrumentProvider"),
        )

        # Create data client
        data_client = ICICIDataClient(
            loop=loop,
            client=client,
            msgbus=msgbus,
            cache=cache,
            clock=clock,
            config=config,
            instrument_provider=instrument_provider,
        )

        return data_client


class ICICILiveExecClientFactory:
    """
    Factory for creating ICICI Direct live execution clients.

    This factory creates properly configured execution clients for order
    management through ICICI Direct's Breeze API.
    """

    @staticmethod
    def create(
        loop: asyncio.AbstractEventLoop,
        name: str,
        config: ICICIExecClientConfig,
        msgbus: MessageBus,
        cache: Cache,
        clock: LiveClock,
        logger: Logger | None = None,
    ) -> LiveExecutionClient:
        """
        Create a new ICICI Direct execution client.

        Parameters
        ----------
        loop : asyncio.AbstractEventLoop
            The event loop for the client.
        name : str
            The client name.
        config : ICICIExecClientConfig
            The client configuration.
        msgbus : MessageBus
            The message bus.
        cache : Cache
            The cache.
        clock : LiveClock
            The clock.
        logger : Logger | None
            The logger (optional).

        Returns
        -------
        LiveExecutionClient
            The configured execution client.
        """
        # Create Breeze client
        client = get_breeze_client(config.api_key)

        # Create instrument provider
        provider_config = ICICIInstrumentProviderConfig(
            api_key=config.api_key,
            api_secret=config.api_secret,
            session_token=config.session_token,
        )

        instrument_provider = ICICIInstrumentProvider(
            client=client,
            config=provider_config,
            logger=logger or Logger(name="ICICIInstrumentProvider"),
        )

        # Create execution client
        exec_client = ICICILiveExecClient(
            loop=loop,
            client=client,
            msgbus=msgbus,
            cache=cache,
            clock=clock,
            config=config,
            instrument_provider=instrument_provider,
        )

        return exec_client


class ICICIInstrumentProviderFactory:
    """
    Factory for creating ICICI Direct instrument providers.
    """

    @staticmethod
    def create(
        config: ICICIInstrumentProviderConfig,
        logger: Logger | None = None,
    ) -> ICICIInstrumentProvider:
        """
        Create a new ICICI Direct instrument provider.

        Parameters
        ----------
        config : ICICIInstrumentProviderConfig
            The provider configuration.
        logger : Logger | None
            The logger (optional).

        Returns
        -------
        ICICIInstrumentProvider
            The configured instrument provider.
        """
        # Create Breeze client
        client = get_breeze_client(config.api_key)

        # Create and return provider
        return ICICIInstrumentProvider(
            client=client,
            config=config,
            logger=logger or Logger(name="ICICIInstrumentProvider"),
        )


def create_icici_live_data_client(
    loop: asyncio.AbstractEventLoop,
    api_key: str,
    api_secret: str,
    session_token: str,
    msgbus: MessageBus,
    cache: Cache,
    clock: LiveClock,
    sandbox: bool = False,
) -> LiveDataClient:
    """
    Convenience function to create an ICICI Direct data client.

    Parameters
    ----------
    loop : asyncio.AbstractEventLoop
        The event loop.
    api_key : str
        The ICICI Direct API key.
    api_secret : str
        The ICICI Direct API secret.
    session_token : str
        The session token from login.
    msgbus : MessageBus
        The message bus.
    cache : Cache
        The cache.
    clock : LiveClock
        The clock.
    sandbox : bool
        Whether to use sandbox mode.

    Returns
    -------
    LiveDataClient
        The configured data client.
    """
    config = ICICIDataClientConfig(
        api_key=api_key,
        api_secret=api_secret,
        session_token=session_token,
        sandbox=sandbox,
    )

    return ICICILiveDataClientFactory.create(
        loop=loop,
        name="ICICI",
        config=config,
        msgbus=msgbus,
        cache=cache,
        clock=clock,
    )


def create_icici_live_exec_client(
    loop: asyncio.AbstractEventLoop,
    api_key: str,
    api_secret: str,
    session_token: str,
    msgbus: MessageBus,
    cache: Cache,
    clock: LiveClock,
    account_id: str | None = None,
    sandbox: bool = False,
) -> LiveExecutionClient:
    """
    Convenience function to create an ICICI Direct execution client.

    Parameters
    ----------
    loop : asyncio.AbstractEventLoop
        The event loop.
    api_key : str
        The ICICI Direct API key.
    api_secret : str
        The ICICI Direct API secret.
    session_token : str
        The session token from login.
    msgbus : MessageBus
        The message bus.
    cache : Cache
        The cache.
    clock : LiveClock
        The clock.
    account_id : str | None
        The trading account ID.
    sandbox : bool
        Whether to use sandbox mode.

    Returns
    -------
    LiveExecutionClient
        The configured execution client.
    """
    config = ICICIExecClientConfig(
        api_key=api_key,
        api_secret=api_secret,
        session_token=session_token,
        account_id=account_id,
        sandbox=sandbox,
    )

    return ICICILiveExecClientFactory.create(
        loop=loop,
        name="ICICI",
        config=config,
        msgbus=msgbus,
        cache=cache,
        clock=clock,
    )
