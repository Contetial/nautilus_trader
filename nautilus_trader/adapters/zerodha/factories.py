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
Factory functions for creating Zerodha adapter components.
"""

from __future__ import annotations

import asyncio
from typing import Any

from nautilus_trader.adapters.zerodha.config import ZerodhaDataClientConfig
from nautilus_trader.adapters.zerodha.config import ZerodhaExecClientConfig  
from nautilus_trader.adapters.zerodha.config import ZerodhaInstrumentProviderConfig
from nautilus_trader.cache.cache import Cache
from nautilus_trader.common.component import LiveClock
from nautilus_trader.common.component import MessageBus
from nautilus_trader.live.data_client import LiveDataClient
from nautilus_trader.live.execution_client import LiveExecClient
from nautilus_trader.model.identifiers import InstrumentId


# TODO: Import actual client implementations when ready
# from nautilus_trader.adapters.zerodha.data import ZerodhaDataClient
# from nautilus_trader.adapters.zerodha.execution import ZerodhaExecutionClient
# from nautilus_trader.adapters.zerodha.providers import ZerodhaInstrumentProvider


class ZerodhaLiveDataClientFactory:
    """
    Provides a `ZerodhaDataClient` factory.
    """

    @staticmethod
    def create(
        loop: asyncio.AbstractEventLoop,
        name: str,
        config: ZerodhaDataClientConfig,
        msgbus: MessageBus,
        cache: Cache,
        clock: LiveClock,
    ) -> LiveDataClient:
        """
        Create a new Zerodha data client.

        Parameters
        ----------
        loop : asyncio.AbstractEventLoop
            The event loop for the client.
        name : str
            The custom client ID.
        config : ZerodhaDataClientConfig
            The configuration for the client.
        msgbus : MessageBus
            The message bus for the client.
        cache : Cache
            The cache for the client.
        clock : LiveClock
            The clock for the client.

        Returns
        -------
        LiveDataClient
            The Zerodha data client.
        """
        # TODO: Implement actual ZerodhaDataClient
        # For now, return a placeholder
        raise NotImplementedError("ZerodhaDataClient not yet implemented")
        
        # return ZerodhaDataClient(
        #     loop=loop,
        #     client=None,  # TODO: Create HTTP client
        #     msgbus=msgbus,
        #     cache=cache,
        #     clock=clock,
        #     config=config,
        # )


class ZerodhaLiveExecClientFactory:
    """
    Provides a `ZerodhaExecutionClient` factory.
    """

    @staticmethod
    def create(
        loop: asyncio.AbstractEventLoop,
        name: str,
        config: ZerodhaExecClientConfig,
        msgbus: MessageBus,
        cache: Cache,
        clock: LiveClock,
    ) -> LiveExecClient:
        """
        Create a new Zerodha execution client.

        Parameters
        ----------
        loop : asyncio.AbstractEventLoop
            The event loop for the client.
        name : str
            The custom client ID.
        config : ZerodhaExecClientConfig
            The configuration for the client.
        msgbus : MessageBus
            The message bus for the client.
        cache : Cache
            The cache for the client.
        clock : LiveClock
            The clock for the client.

        Returns
        -------
        LiveExecClient
            The Zerodha execution client.
        """
        # TODO: Implement actual ZerodhaExecutionClient
        # For now, return a placeholder
        raise NotImplementedError("ZerodhaExecutionClient not yet implemented")
        
        # return ZerodhaExecutionClient(
        #     loop=loop,
        #     client=None,  # TODO: Create HTTP client
        #     msgbus=msgbus,
        #     cache=cache,
        #     clock=clock,
        #     config=config,
        # )


def create_zerodha_instrument_provider(
    config: ZerodhaInstrumentProviderConfig,
    logger: Any,
) -> Any:  # TODO: Return ZerodhaInstrumentProvider when implemented
    """
    Create a Zerodha instrument provider.
    
    Parameters
    ----------
    config : ZerodhaInstrumentProviderConfig
        The configuration for the instrument provider.
    logger : Any
        The logger for the provider.
        
    Returns
    -------
    ZerodhaInstrumentProvider
        The instrument provider instance.
    """
    # TODO: Implement ZerodhaInstrumentProvider
    raise NotImplementedError("ZerodhaInstrumentProvider not yet implemented")
    
    # return ZerodhaInstrumentProvider(
    #     client=None,  # TODO: Create HTTP client
    #     config=config,
    #     logger=logger,
    # )