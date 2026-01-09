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
Factory classes for Kotak Neo adapter components.
"""

from __future__ import annotations

import asyncio
from typing import Any

from nautilus_trader.adapters.kotak.config import KotakDataClientConfig
from nautilus_trader.adapters.kotak.config import KotakExecClientConfig
from nautilus_trader.adapters.kotak.config import KotakInstrumentProviderConfig
from nautilus_trader.adapters.kotak.data import KotakDataClient
from nautilus_trader.adapters.kotak.execution import KotakLiveExecClient
from nautilus_trader.adapters.kotak.providers import KotakInstrumentProvider
from nautilus_trader.cache.cache import Cache
from nautilus_trader.common.component import LiveClock
from nautilus_trader.common.component import Logger
from nautilus_trader.common.component import MessageBus
from nautilus_trader.live.data_client import LiveDataClient
from nautilus_trader.live.execution_client import LiveExecutionClient


def get_neo_client(consumer_key: str, consumer_secret: str) -> Any:
    """
    Create a Neo API client instance.

    Parameters
    ----------
    consumer_key : str
        The Kotak Neo consumer key.
    consumer_secret : str
        The Kotak Neo consumer secret.

    Returns
    -------
    Any
        The Neo API client instance.

    Raises
    ------
    ImportError
        If neo_api_client package is not installed.
    """
    try:
        from neo_api_client import NeoAPI
        return NeoAPI(
            consumer_key=consumer_key,
            consumer_secret=consumer_secret,
            environment="prod",
        )
    except ImportError as e:
        raise ImportError(
            "The 'neo_api_client' package is required for Kotak Neo integration. "
            "Install it with: pip install neo-api-client"
        ) from e


class KotakLiveDataClientFactory:
    """
    Factory for creating Kotak Neo live data clients.
    """

    @staticmethod
    def create(
        loop: asyncio.AbstractEventLoop,
        name: str,
        config: KotakDataClientConfig,
        msgbus: MessageBus,
        cache: Cache,
        clock: LiveClock,
        logger: Logger | None = None,
    ) -> LiveDataClient:
        """
        Create a new Kotak Neo data client.

        Parameters
        ----------
        loop : asyncio.AbstractEventLoop
            The event loop.
        name : str
            The client name.
        config : KotakDataClientConfig
            The configuration.
        msgbus : MessageBus
            The message bus.
        cache : Cache
            The cache.
        clock : LiveClock
            The clock.
        logger : Logger | None
            The logger.

        Returns
        -------
        LiveDataClient
            The data client.
        """
        client = get_neo_client(config.consumer_key, config.consumer_secret)

        provider_config = KotakInstrumentProviderConfig(
            consumer_key=config.consumer_key,
            consumer_secret=config.consumer_secret,
            access_token=config.access_token,
        )

        instrument_provider = KotakInstrumentProvider(
            client=client,
            config=provider_config,
            logger=logger or Logger(name="KotakInstrumentProvider"),
        )

        return KotakDataClient(
            loop=loop,
            client=client,
            msgbus=msgbus,
            cache=cache,
            clock=clock,
            config=config,
            instrument_provider=instrument_provider,
        )


class KotakLiveExecClientFactory:
    """
    Factory for creating Kotak Neo live execution clients.
    """

    @staticmethod
    def create(
        loop: asyncio.AbstractEventLoop,
        name: str,
        config: KotakExecClientConfig,
        msgbus: MessageBus,
        cache: Cache,
        clock: LiveClock,
        logger: Logger | None = None,
    ) -> LiveExecutionClient:
        """
        Create a new Kotak Neo execution client.

        Parameters
        ----------
        loop : asyncio.AbstractEventLoop
            The event loop.
        name : str
            The client name.
        config : KotakExecClientConfig
            The configuration.
        msgbus : MessageBus
            The message bus.
        cache : Cache
            The cache.
        clock : LiveClock
            The clock.
        logger : Logger | None
            The logger.

        Returns
        -------
        LiveExecutionClient
            The execution client.
        """
        client = get_neo_client(config.consumer_key, config.consumer_secret)

        provider_config = KotakInstrumentProviderConfig(
            consumer_key=config.consumer_key,
            consumer_secret=config.consumer_secret,
            access_token=config.access_token,
        )

        instrument_provider = KotakInstrumentProvider(
            client=client,
            config=provider_config,
            logger=logger or Logger(name="KotakInstrumentProvider"),
        )

        return KotakLiveExecClient(
            loop=loop,
            client=client,
            msgbus=msgbus,
            cache=cache,
            clock=clock,
            config=config,
            instrument_provider=instrument_provider,
        )


class KotakInstrumentProviderFactory:
    """
    Factory for creating Kotak Neo instrument providers.
    """

    @staticmethod
    def create(
        config: KotakInstrumentProviderConfig,
        logger: Logger | None = None,
    ) -> KotakInstrumentProvider:
        """
        Create a Kotak Neo instrument provider.

        Parameters
        ----------
        config : KotakInstrumentProviderConfig
            The configuration.
        logger : Logger | None
            The logger.

        Returns
        -------
        KotakInstrumentProvider
            The instrument provider.
        """
        client = get_neo_client(config.consumer_key, config.consumer_secret)

        return KotakInstrumentProvider(
            client=client,
            config=config,
            logger=logger or Logger(name="KotakInstrumentProvider"),
        )


def create_kotak_live_data_client(
    loop: asyncio.AbstractEventLoop,
    consumer_key: str,
    consumer_secret: str,
    access_token: str,
    msgbus: MessageBus,
    cache: Cache,
    clock: LiveClock,
    sandbox: bool = False,
) -> LiveDataClient:
    """
    Convenience function to create a Kotak Neo data client.

    Parameters
    ----------
    loop : asyncio.AbstractEventLoop
        The event loop.
    consumer_key : str
        The consumer key.
    consumer_secret : str
        The consumer secret.
    access_token : str
        The access token.
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
        The data client.
    """
    config = KotakDataClientConfig(
        consumer_key=consumer_key,
        consumer_secret=consumer_secret,
        access_token=access_token,
        sandbox=sandbox,
    )

    return KotakLiveDataClientFactory.create(
        loop=loop,
        name="KOTAK",
        config=config,
        msgbus=msgbus,
        cache=cache,
        clock=clock,
    )


def create_kotak_live_exec_client(
    loop: asyncio.AbstractEventLoop,
    consumer_key: str,
    consumer_secret: str,
    access_token: str,
    msgbus: MessageBus,
    cache: Cache,
    clock: LiveClock,
    account_id: str | None = None,
    sandbox: bool = False,
) -> LiveExecutionClient:
    """
    Convenience function to create a Kotak Neo execution client.

    Parameters
    ----------
    loop : asyncio.AbstractEventLoop
        The event loop.
    consumer_key : str
        The consumer key.
    consumer_secret : str
        The consumer secret.
    access_token : str
        The access token.
    msgbus : MessageBus
        The message bus.
    cache : Cache
        The cache.
    clock : LiveClock
        The clock.
    account_id : str | None
        The account ID.
    sandbox : bool
        Whether to use sandbox mode.

    Returns
    -------
    LiveExecutionClient
        The execution client.
    """
    config = KotakExecClientConfig(
        consumer_key=consumer_key,
        consumer_secret=consumer_secret,
        access_token=access_token,
        account_id=account_id,
        sandbox=sandbox,
    )

    return KotakLiveExecClientFactory.create(
        loop=loop,
        name="KOTAK",
        config=config,
        msgbus=msgbus,
        cache=cache,
        clock=clock,
    )
