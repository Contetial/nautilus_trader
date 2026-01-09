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
Kotak Neo adapter for NautilusTrader.

Provides integration with Kotak Securities' Neo API for Indian stock markets,
supporting equity, derivatives, and currency trading on NSE and BSE.
"""

from nautilus_trader.adapters.kotak.config import KotakConfig
from nautilus_trader.adapters.kotak.config import KotakDataClientConfig
from nautilus_trader.adapters.kotak.config import KotakExecClientConfig
from nautilus_trader.adapters.kotak.config import KotakInstrumentProviderConfig
from nautilus_trader.adapters.kotak.factories import KotakLiveDataClientFactory
from nautilus_trader.adapters.kotak.factories import KotakLiveExecClientFactory

__all__ = [
    "KotakConfig",
    "KotakDataClientConfig",
    "KotakExecClientConfig",
    "KotakInstrumentProviderConfig",
    "KotakLiveDataClientFactory",
    "KotakLiveExecClientFactory",
]
