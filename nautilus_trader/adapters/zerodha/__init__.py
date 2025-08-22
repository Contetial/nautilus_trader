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
Zerodha adapter for NautilusTrader.

Provides integration with Zerodha's Kite Connect API for Indian stock markets,
with specialized support for options trading on NSE and BSE.
"""

from nautilus_trader.adapters.zerodha.config import ZerodhaConfig
from nautilus_trader.adapters.zerodha.factories import ZerodhaLiveDataClientFactory
from nautilus_trader.adapters.zerodha.factories import ZerodhaLiveExecClientFactory

__all__ = [
    "ZerodhaConfig",
    "ZerodhaLiveDataClientFactory", 
    "ZerodhaLiveExecClientFactory",
]