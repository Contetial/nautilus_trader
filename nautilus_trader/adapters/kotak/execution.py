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
Execution client for Kotak Neo integration.
"""

from __future__ import annotations

import asyncio
from decimal import Decimal
from typing import Any

from nautilus_trader.adapters.kotak.config import KotakExecClientConfig
from nautilus_trader.adapters.kotak.providers import KotakInstrumentProvider
from nautilus_trader.common.component import LiveClock
from nautilus_trader.common.component import MessageBus
from nautilus_trader.execution.messages import CancelOrder
from nautilus_trader.execution.messages import ModifyOrder
from nautilus_trader.execution.messages import SubmitOrder
from nautilus_trader.execution.reports import FillReport
from nautilus_trader.execution.reports import OrderStatusReport
from nautilus_trader.execution.reports import PositionStatusReport
from nautilus_trader.live.execution_client import LiveExecutionClient
from nautilus_trader.model.currencies import INR
from nautilus_trader.model.enums import AccountType
from nautilus_trader.model.enums import LiquiditySide
from nautilus_trader.model.enums import OmsType
from nautilus_trader.model.enums import OrderSide
from nautilus_trader.model.enums import OrderStatus
from nautilus_trader.model.enums import OrderType
from nautilus_trader.model.enums import TimeInForce
from nautilus_trader.model.identifiers import AccountId
from nautilus_trader.model.identifiers import ClientId
from nautilus_trader.model.identifiers import ClientOrderId
from nautilus_trader.model.identifiers import InstrumentId
from nautilus_trader.model.identifiers import TradeId
from nautilus_trader.model.identifiers import Venue
from nautilus_trader.model.identifiers import VenueOrderId
from nautilus_trader.model.objects import Money
from nautilus_trader.model.objects import Price
from nautilus_trader.model.objects import Quantity
from nautilus_trader.model.orders import Order


class KotakLiveExecClient(LiveExecutionClient):
    """
    Provides a live execution client for Kotak Neo.

    Parameters
    ----------
    loop : asyncio.AbstractEventLoop
        The event loop.
    client : Any
        The Neo API client.
    msgbus : MessageBus
        The message bus.
    cache : Any
        The cache.
    clock : LiveClock
        The clock.
    config : KotakExecClientConfig
        The configuration.
    instrument_provider : KotakInstrumentProvider | None
        The instrument provider.
    """

    def __init__(
        self,
        loop: asyncio.AbstractEventLoop,
        client: Any,
        msgbus: MessageBus,
        cache: Any,
        clock: LiveClock,
        config: KotakExecClientConfig,
        instrument_provider: KotakInstrumentProvider | None = None,
    ) -> None:
        account_id_str = config.account_id or "DEFAULT"
        account_id = AccountId(f"KOTAK-{account_id_str}")

        super().__init__(
            loop=loop,
            client_id=ClientId("KOTAK"),
            venue=None,
            oms_type=OmsType.NETTING,
            account_type=AccountType.CASH,
            base_currency=INR,
            msgbus=msgbus,
            cache=cache,
            clock=clock,
        )

        self._client = client
        self._config = config
        self._instrument_provider = instrument_provider
        self._account_id = account_id

        # Order tracking
        self._order_id_to_client_order_id: dict[str, ClientOrderId] = {}
        self._client_order_id_to_order_id: dict[ClientOrderId, str] = {}

        self._order_update_task: asyncio.Task | None = None

        self._log.info("Initialized KotakLiveExecClient")

    async def _connect(self) -> None:
        """Connect to Kotak Neo execution services."""
        try:
            self._log.info("Connecting to Kotak Neo execution...")

            if self._config.access_token and hasattr(self._client, 'session_init'):
                self._client.session_init(access_token=self._config.access_token)

            await self._load_existing_orders()
            await self._load_existing_positions()

            self._order_update_task = asyncio.create_task(self._poll_order_updates())

            self._log.info("Connected to Kotak Neo execution")

        except Exception as e:
            self._log.error(f"Failed to connect: {e}")
            raise

    async def _disconnect(self) -> None:
        """Disconnect from Kotak Neo execution services."""
        self._log.info("Disconnecting from Kotak Neo execution...")

        if self._order_update_task:
            self._order_update_task.cancel()
            try:
                await self._order_update_task
            except asyncio.CancelledError:
                pass

        self._order_id_to_client_order_id.clear()
        self._client_order_id_to_order_id.clear()

        self._log.info("Disconnected from Kotak Neo execution")

    async def _load_existing_orders(self) -> None:
        """Load existing orders."""
        try:
            if hasattr(self._client, 'order_report'):
                response = self._client.order_report()
                if response and response.get("data"):
                    self._log.info(f"Loaded {len(response['data'])} existing orders")
        except Exception as e:
            self._log.error(f"Failed to load orders: {e}")

    async def _load_existing_positions(self) -> None:
        """Load existing positions."""
        try:
            if hasattr(self._client, 'positions'):
                response = self._client.positions()
                if response and response.get("data"):
                    self._log.info(f"Loaded {len(response['data'])} existing positions")
        except Exception as e:
            self._log.error(f"Failed to load positions: {e}")

    async def _submit_order(self, command: SubmitOrder) -> None:
        """Submit an order to Kotak Neo."""
        try:
            order = command.order
            instrument_id = order.instrument_id

            self._log.info(f"Submitting order: {order}")

            # Map order parameters
            exchange = self._venue_to_exchange(instrument_id.venue)
            transaction_type = "B" if order.side == OrderSide.BUY else "S"
            order_type = self._map_order_type(order.order_type)
            product = self._get_product_type(instrument_id)
            quantity = int(order.quantity)

            price = "0"
            trigger_price = "0"

            if order.order_type == OrderType.LIMIT:
                price = str(float(order.price))
            elif order.order_type in [OrderType.STOP_LIMIT, OrderType.STOP_MARKET]:
                if order.trigger_price:
                    trigger_price = str(float(order.trigger_price))
                if order.order_type == OrderType.STOP_LIMIT:
                    price = str(float(order.price))

            validity = self._map_time_in_force(order.time_in_force)

            # Place order via Neo API
            response = self._client.place_order(
                exchange_segment=exchange,
                product=product,
                price=price,
                order_type=order_type,
                quantity=str(quantity),
                validity=validity,
                trading_symbol=str(instrument_id.symbol),
                transaction_type=transaction_type,
                trigger_price=trigger_price,
            )

            if response and response.get("stat") == "Ok":
                venue_order_id = response.get("nOrdNo")

                self._order_id_to_client_order_id[venue_order_id] = order.client_order_id
                self._client_order_id_to_order_id[order.client_order_id] = venue_order_id

                self._log.info(f"Order submitted: {venue_order_id}")
                self._generate_order_accepted(order, venue_order_id)
            else:
                error = response.get("errMsg") if response else "Unknown error"
                self._log.error(f"Order rejected: {error}")
                self._generate_order_rejected(order, str(error))

        except Exception as e:
            self._log.error(f"Failed to submit order: {e}")
            self._generate_order_rejected(command.order, str(e))

    async def _modify_order(self, command: ModifyOrder) -> None:
        """Modify an existing order."""
        try:
            client_order_id = command.client_order_id
            venue_order_id = self._client_order_id_to_order_id.get(client_order_id)

            if not venue_order_id:
                self._log.error(f"Cannot find venue order ID for {client_order_id}")
                return

            order = self._cache.order(client_order_id)
            if not order:
                return

            response = self._client.modify_order(
                order_id=venue_order_id,
                price=str(float(command.price)) if command.price else None,
                quantity=str(int(command.quantity)) if command.quantity else None,
            )

            if response and response.get("stat") == "Ok":
                self._log.info(f"Order modified: {venue_order_id}")
            else:
                error = response.get("errMsg") if response else "Unknown error"
                self._log.error(f"Modification failed: {error}")

        except Exception as e:
            self._log.error(f"Failed to modify order: {e}")

    async def _cancel_order(self, command: CancelOrder) -> None:
        """Cancel an order."""
        try:
            client_order_id = command.client_order_id
            venue_order_id = self._client_order_id_to_order_id.get(client_order_id)

            if not venue_order_id:
                self._log.error(f"Cannot find venue order ID for {client_order_id}")
                return

            order = self._cache.order(client_order_id)
            if not order:
                return

            response = self._client.cancel_order(order_id=venue_order_id)

            if response and response.get("stat") == "Ok":
                self._log.info(f"Order cancelled: {venue_order_id}")
                self._generate_order_canceled(order)
            else:
                error = response.get("errMsg") if response else "Unknown error"
                self._log.error(f"Cancellation failed: {error}")

        except Exception as e:
            self._log.error(f"Failed to cancel order: {e}")

    async def _cancel_all_orders(self, command: Any) -> None:
        """Cancel all orders."""
        self._log.warning("Cancel all orders not implemented for Kotak Neo")

    async def _poll_order_updates(self) -> None:
        """Poll for order updates."""
        while True:
            try:
                await asyncio.sleep(2)

                if hasattr(self._client, 'order_report'):
                    response = self._client.order_report()
                    if response and response.get("data"):
                        for order_data in response["data"]:
                            await self._process_order_update(order_data)

            except asyncio.CancelledError:
                break
            except Exception as e:
                self._log.error(f"Error polling orders: {e}")
                await asyncio.sleep(5)

    async def _process_order_update(self, order_data: dict) -> None:
        """Process an order update."""
        try:
            venue_order_id = order_data.get("nOrdNo")
            status = order_data.get("ordSt", "").upper()

            client_order_id = self._order_id_to_client_order_id.get(venue_order_id)
            if not client_order_id:
                return

            order = self._cache.order(client_order_id)
            if not order:
                return

            if status in ["COMPLETE", "TRADED"]:
                filled_qty = int(order_data.get("fldQty", 0))
                avg_price = float(order_data.get("avgPrc", 0))
                self._generate_order_filled(order, filled_qty, avg_price)
            elif status in ["REJECTED", "CANCELLED"]:
                if status == "CANCELLED":
                    self._generate_order_canceled(order)
                else:
                    reason = order_data.get("rejRsn", "")
                    self._generate_order_rejected(order, reason)

        except Exception as e:
            self._log.error(f"Error processing order update: {e}")

    def _generate_order_accepted(self, order: Order, venue_order_id: str) -> None:
        """Generate order accepted event."""
        self._send_order_status_report(order, VenueOrderId(venue_order_id), OrderStatus.ACCEPTED)

    def _generate_order_rejected(self, order: Order, reason: str) -> None:
        """Generate order rejected event."""
        self._send_order_status_report(order, None, OrderStatus.REJECTED, reason)

    def _generate_order_canceled(self, order: Order) -> None:
        """Generate order canceled event."""
        venue_order_id = self._client_order_id_to_order_id.get(order.client_order_id)
        self._send_order_status_report(
            order, VenueOrderId(venue_order_id) if venue_order_id else None, OrderStatus.CANCELED
        )

    def _generate_order_filled(self, order: Order, filled_qty: int, avg_price: float) -> None:
        """Generate order filled event."""
        venue_order_id = self._client_order_id_to_order_id.get(order.client_order_id)
        instrument = self._cache.instrument(order.instrument_id)
        if not instrument:
            return

        fill_report = FillReport(
            account_id=self._account_id,
            instrument_id=order.instrument_id,
            venue_order_id=VenueOrderId(venue_order_id) if venue_order_id else None,
            trade_id=TradeId(f"FILL-{venue_order_id}"),
            order_side=order.side,
            last_qty=Quantity(Decimal(str(filled_qty)), instrument.size_precision),
            last_px=Price(Decimal(str(avg_price)), instrument.price_precision),
            commission=Money(0, INR),
            liquidity_side=LiquiditySide.TAKER,
            ts_event=self._clock.timestamp_ns(),
            ts_init=self._clock.timestamp_ns(),
        )
        self._send_fill_report(fill_report)

    def _send_order_status_report(
        self, order: Order, venue_order_id: VenueOrderId | None,
        order_status: OrderStatus, reason: str = ""
    ) -> None:
        """Send order status report."""
        instrument = self._cache.instrument(order.instrument_id)
        if not instrument:
            return

        report = OrderStatusReport(
            account_id=self._account_id,
            instrument_id=order.instrument_id,
            client_order_id=order.client_order_id,
            venue_order_id=venue_order_id,
            order_side=order.side,
            order_type=order.order_type,
            time_in_force=order.time_in_force,
            order_status=order_status,
            quantity=order.quantity,
            filled_qty=Quantity(0, instrument.size_precision),
            avg_px=None,
            cancel_reason=reason if order_status == OrderStatus.REJECTED else None,
            ts_accepted=self._clock.timestamp_ns(),
            ts_last=self._clock.timestamp_ns(),
            ts_init=self._clock.timestamp_ns(),
        )
        self._msgbus.send(endpoint="ExecEngine.process", msg=report)

    def _send_fill_report(self, report: FillReport) -> None:
        """Send fill report."""
        self._msgbus.send(endpoint="ExecEngine.process", msg=report)

    def _venue_to_exchange(self, venue: Venue) -> str:
        """Convert venue to Neo exchange segment."""
        mapping = {"NSE": "nse_cm", "BSE": "bse_cm", "NFO": "nse_fo", "BFO": "bse_fo"}
        return mapping.get(venue.value, "nse_cm")

    def _get_product_type(self, instrument_id: InstrumentId) -> str:
        """Get Neo product type."""
        venue = instrument_id.venue.value
        if venue in ["NFO", "BFO"]:
            return "NRML"
        return "CNC"

    def _map_order_type(self, order_type: OrderType) -> str:
        """Map order type to Neo format."""
        mapping = {OrderType.MARKET: "MKT", OrderType.LIMIT: "L", OrderType.STOP_MARKET: "SL-M", OrderType.STOP_LIMIT: "SL"}
        return mapping.get(order_type, "L")

    def _map_time_in_force(self, tif: TimeInForce) -> str:
        """Map time in force to Neo format."""
        mapping = {TimeInForce.DAY: "DAY", TimeInForce.IOC: "IOC", TimeInForce.GTC: "DAY"}
        return mapping.get(tif, "DAY")

    async def generate_order_status_report(self, *args, **kwargs) -> OrderStatusReport | None:
        return None

    async def generate_order_status_reports(self, *args, **kwargs) -> list[OrderStatusReport]:
        return []

    async def generate_fill_reports(self, *args, **kwargs) -> list[FillReport]:
        return []

    async def generate_position_status_reports(self, *args, **kwargs) -> list[PositionStatusReport]:
        return []
