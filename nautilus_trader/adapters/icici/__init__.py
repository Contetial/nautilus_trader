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
ICICI Direct Breeze adapter for NautilusTrader.

Provides integration with ICICI Direct's Breeze API for Indian stock markets,
with support for NSE, BSE, and NFO segments.
"""

from nautilus_trader.adapters.icici.config import ICICIConfig
from nautilus_trader.adapters.icici.config import ICICIDataClientConfig
from nautilus_trader.adapters.icici.config import ICICIExecClientConfig
from nautilus_trader.adapters.icici.config import ICICIInstrumentProviderConfig
from nautilus_trader.adapters.icici.factories import ICICILiveDataClientFactory
from nautilus_trader.adapters.icici.factories import ICICILiveExecClientFactory

__all__ = [
    "ICICIConfig",
    "ICICIDataClientConfig",
    "ICICIExecClientConfig",
    "ICICIInstrumentProviderConfig",
    "ICICILiveDataClientFactory",
    "ICICILiveExecClientFactory",
]
