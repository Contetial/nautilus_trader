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
Execution client for ICICI Direct Breeze integration.

This module provides the LiveExecClient implementation for NautilusTrader,
enabling order placement, modification, and cancellation through ICICI Direct's Breeze API.
"""

from __future__ import annotations

import asyncio
from decimal import Decimal
from typing import Any

from nautilus_trader.adapters.icici.config import ICICIExecClientConfig
from nautilus_trader.adapters.icici.providers import ICICIInstrumentProvider
from nautilus_trader.common.component import LiveClock
from nautilus_trader.common.component import MessageBus
from nautilus_trader.core.uuid import UUID4
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
from nautilus_trader.model.identifiers import PositionId
from nautilus_trader.model.identifiers import StrategyId
from nautilus_trader.model.identifiers import TradeId
from nautilus_trader.model.identifiers import Venue
from nautilus_trader.model.identifiers import VenueOrderId
from nautilus_trader.model.objects import AccountBalance
from nautilus_trader.model.objects import Currency
from nautilus_trader.model.objects import MarginBalance
from nautilus_trader.model.objects import Money
from nautilus_trader.model.objects import Price
from nautilus_trader.model.objects import Quantity
from nautilus_trader.model.orders import Order


class ICICILiveExecClient(LiveExecutionClient):
    """
    Provides a live execution client for ICICI Direct Breeze.

    This client handles order placement, modification, cancellation, and position
    tracking through ICICI Direct's Breeze API.

    Parameters
    ----------
    loop : asyncio.AbstractEventLoop
        The event loop for the client.
    client : Any
        The Breeze API client.
    msgbus : MessageBus
        The message bus for the client.
    cache : Cache
        The cache for the client.
    clock : LiveClock
        The clock for the client.
    config : ICICIExecClientConfig
        The configuration for the client.
    instrument_provider : ICICIInstrumentProvider | None
        The instrument provider for the client.
    """

    def __init__(
        self,
        loop: asyncio.AbstractEventLoop,
        client: Any,
        msgbus: MessageBus,
        cache: Any,
        clock: LiveClock,
        config: ICICIExecClientConfig,
        instrument_provider: ICICIInstrumentProvider | None = None,
    ) -> None:
        # Account setup
        account_id_str = config.account_id or "DEFAULT"
        account_id = AccountId(f"ICICI-{account_id_str}")

        super().__init__(
            loop=loop,
            client_id=ClientId("ICICI"),
            venue=None,  # Multi-venue support
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

        # Order tracking
        self._order_id_to_client_order_id: dict[str, ClientOrderId] = {}
        self._client_order_id_to_order_id: dict[ClientOrderId, str] = {}

        # Account information
        self._account_id = account_id
        self._account_balance = Decimal("0")
        self._available_margin = Decimal("0")

        # Order monitoring
        self._order_update_task: asyncio.Task | None = None

        self._log.info("Initialized ICICILiveExecClient")

    # =========================================================================
    # Connection Lifecycle
    # =========================================================================

    async def _connect(self) -> None:
        """Connect to ICICI Direct execution services."""
        try:
            self._log.info("Connecting to ICICI Direct execution services...")

            # Generate session if needed
            if self._config.session_token:
                self._client.generate_session(
                    api_secret=self._config.api_secret,
                    session_token=self._config.session_token,
                )

            # Update account info
            await self._update_account_info()

            # Load existing orders and positions
            await self._load_existing_orders()
            await self._load_existing_positions()

            # Start order monitoring
            self._order_update_task = asyncio.create_task(self._poll_order_updates())

            self._log.info("Connected to ICICI Direct execution services")

        except Exception as e:
            self._log.error(f"Failed to connect to ICICI Direct: {e}")
            raise

    async def _disconnect(self) -> None:
        """Disconnect from ICICI Direct execution services."""
        self._log.info("Disconnecting from ICICI Direct execution services...")

        # Stop order monitoring
        if self._order_update_task:
            self._order_update_task.cancel()
            try:
                await self._order_update_task
            except asyncio.CancelledError:
                pass

        # Clean up
        self._order_id_to_client_order_id.clear()
        self._client_order_id_to_order_id.clear()

        self._log.info("Disconnected from ICICI Direct execution services")

    # =========================================================================
    # Account Management
    # =========================================================================

    async def _update_account_info(self) -> None:
        """Update account balance and margin information."""
        try:
            response = self._client.get_funds()

            if response and response.get("Success"):
                data = response["Success"]
                # Parse balance data - structure depends on Breeze API response
                self._account_balance = Decimal(str(data.get("available_cash", 0)))
                self._available_margin = Decimal(str(data.get("available_margin", 0)))

                self._log.info(f"Account balance: {self._account_balance} INR")

        except Exception as e:
            self._log.error(f"Failed to update account info: {e}")

    async def _load_existing_orders(self) -> None:
        """Load existing orders from ICICI Direct."""
        try:
            from datetime import datetime

            today = datetime.now().strftime("%Y-%m-%d")
            response = self._client.get_order_list(
                exchange_code="NSE",
                from_date=today,
                to_date=today,
            )

            if response and response.get("Success"):
                orders = response["Success"]
                self._log.info(f"Loaded {len(orders)} existing orders")

                for order_data in orders:
                    await self._process_order_update(order_data)

        except Exception as e:
            self._log.error(f"Failed to load existing orders: {e}")

    async def _load_existing_positions(self) -> None:
        """Load existing positions from ICICI Direct."""
        try:
            response = self._client.get_portfolio_positions()

            if response and response.get("Success"):
                positions = response["Success"]
                self._log.info(f"Loaded {len(positions)} existing positions")

        except Exception as e:
            self._log.error(f"Failed to load existing positions: {e}")

    # =========================================================================
    # Order Submission
    # =========================================================================

    async def _submit_order(self, command: SubmitOrder) -> None:
        """
        Submit an order to ICICI Direct.

        Parameters
        ----------
        command : SubmitOrder
            The submit order command.
        """
        try:
            order = command.order
            instrument_id = order.instrument_id

            self._log.info(f"Submitting order: {order}")

            # Map order parameters
            symbol = str(instrument_id.symbol)
            exchange = self._venue_to_exchange(instrument_id.venue)
            action = "buy" if order.side == OrderSide.BUY else "sell"
            order_type = self._map_order_type(order.order_type)
            product = self._get_product_type(instrument_id)
            quantity = int(order.quantity)

            # Price handling
            price = 0
            trigger_price = 0

            if order.order_type == OrderType.LIMIT:
                price = float(order.price)
            elif order.order_type == OrderType.STOP_LIMIT:
                price = float(order.price)
                trigger_price = float(order.trigger_price) if order.trigger_price else 0
            elif order.order_type == OrderType.STOP_MARKET:
                trigger_price = float(order.trigger_price) if order.trigger_price else 0

            # Validity mapping
            validity = self._map_time_in_force(order.time_in_force)

            # Place order via Breeze API
            response = self._client.place_order(
                stock_code=symbol,
                exchange_code=exchange,
                product=product,
                action=action,
                order_type=order_type,
                quantity=str(quantity),
                price=str(price) if price > 0 else "",
                validity=validity,
                stoploss=str(trigger_price) if trigger_price > 0 else "",
                disclosed_quantity="0",
            )

            if response and response.get("Success"):
                venue_order_id = response["Success"].get("order_id")

                # Track order mapping
                self._order_id_to_client_order_id[venue_order_id] = order.client_order_id
                self._client_order_id_to_order_id[order.client_order_id] = venue_order_id

                self._log.info(f"Order submitted: {venue_order_id}")

                # Generate accepted report
                self._generate_order_accepted(order, venue_order_id)

            else:
                error = response.get("Error") if response else "Unknown error"
                self._log.error(f"Order rejected: {error}")
                self._generate_order_rejected(order, str(error))

        except Exception as e:
            self._log.error(f"Failed to submit order: {e}")
            self._generate_order_rejected(command.order, str(e))

    async def _modify_order(self, command: ModifyOrder) -> None:
        """
        Modify an existing order.

        Parameters
        ----------
        command : ModifyOrder
            The modify order command.
        """
        try:
            client_order_id = command.client_order_id
            venue_order_id = self._client_order_id_to_order_id.get(client_order_id)

            if not venue_order_id:
                self._log.error(f"Cannot find venue order ID for {client_order_id}")
                return

            order = self._cache.order(client_order_id)
            if not order:
                self._log.error(f"Cannot find order {client_order_id}")
                return

            exchange = self._venue_to_exchange(order.instrument_id.venue)

            # Modify via Breeze API
            response = self._client.modify_order(
                order_id=venue_order_id,
                exchange_code=exchange,
                quantity=str(int(command.quantity)) if command.quantity else "",
                price=str(float(command.price)) if command.price else "",
            )

            if response and response.get("Success"):
                self._log.info(f"Order modified: {venue_order_id}")
            else:
                error = response.get("Error") if response else "Unknown error"
                self._log.error(f"Order modification failed: {error}")

        except Exception as e:
            self._log.error(f"Failed to modify order: {e}")

    async def _cancel_order(self, command: CancelOrder) -> None:
        """
        Cancel an order.

        Parameters
        ----------
        command : CancelOrder
            The cancel order command.
        """
        try:
            client_order_id = command.client_order_id
            venue_order_id = self._client_order_id_to_order_id.get(client_order_id)

            if not venue_order_id:
                self._log.error(f"Cannot find venue order ID for {client_order_id}")
                return

            order = self._cache.order(client_order_id)
            if not order:
                self._log.error(f"Cannot find order {client_order_id}")
                return

            exchange = self._venue_to_exchange(order.instrument_id.venue)

            # Cancel via Breeze API
            response = self._client.cancel_order(
                order_id=venue_order_id,
                exchange_code=exchange,
            )

            if response and response.get("Success"):
                self._log.info(f"Order cancelled: {venue_order_id}")
                self._generate_order_canceled(order)
            else:
                error = response.get("Error") if response else "Unknown error"
                self._log.error(f"Order cancellation failed: {error}")

        except Exception as e:
            self._log.error(f"Failed to cancel order: {e}")

    async def _cancel_all_orders(self, command: Any) -> None:
        """Cancel all orders for the given instrument or all instruments."""
        self._log.warning("Cancel all orders not fully implemented for ICICI Direct")

    # =========================================================================
    # Order Reports
    # =========================================================================

    async def generate_order_status_report(
        self,
        instrument_id: InstrumentId,
        client_order_id: ClientOrderId | None = None,
        venue_order_id: VenueOrderId | None = None,
    ) -> OrderStatusReport | None:
        """Generate order status report."""
        # Implementation would query Breeze API for order status
        return None

    async def generate_order_status_reports(
        self,
        instrument_id: InstrumentId | None = None,
        start: datetime | None = None,
        end: datetime | None = None,
        open_only: bool = False,
    ) -> list[OrderStatusReport]:
        """Generate order status reports."""
        return []

    async def generate_fill_reports(
        self,
        instrument_id: InstrumentId | None = None,
        venue_order_id: VenueOrderId | None = None,
        start: datetime | None = None,
        end: datetime | None = None,
    ) -> list[FillReport]:
        """Generate fill reports."""
        return []

    async def generate_position_status_reports(
        self,
        instrument_id: InstrumentId | None = None,
        start: datetime | None = None,
        end: datetime | None = None,
    ) -> list[PositionStatusReport]:
        """Generate position status reports."""
        return []

    # =========================================================================
    # Order Monitoring
    # =========================================================================

    async def _poll_order_updates(self) -> None:
        """Poll for order updates periodically."""
        while True:
            try:
                await asyncio.sleep(2)  # Poll every 2 seconds

                from datetime import datetime
                today = datetime.now().strftime("%Y-%m-%d")

                response = self._client.get_order_list(
                    exchange_code="NSE",
                    from_date=today,
                    to_date=today,
                )

                if response and response.get("Success"):
                    for order_data in response["Success"]:
                        await self._process_order_update(order_data)

            except asyncio.CancelledError:
                break
            except Exception as e:
                self._log.error(f"Error polling order updates: {e}")
                await asyncio.sleep(5)

    async def _process_order_update(self, order_data: dict) -> None:
        """Process an order update from Breeze API."""
        try:
            venue_order_id = order_data.get("order_id")
            status = order_data.get("status", "").upper()

            client_order_id = self._order_id_to_client_order_id.get(venue_order_id)
            if not client_order_id:
                return  # Not our order

            order = self._cache.order(client_order_id)
            if not order:
                return

            # Map Breeze status to NautilusTrader status
            if status in ["COMPLETE", "EXECUTED"]:
                filled_qty = int(order_data.get("quantity", 0))
                avg_price = float(order_data.get("average_price", 0))
                self._generate_order_filled(order, filled_qty, avg_price)

            elif status in ["REJECTED", "CANCELLED"]:
                reason = order_data.get("rejection_reason", "")
                if status == "CANCELLED":
                    self._generate_order_canceled(order)
                else:
                    self._generate_order_rejected(order, reason)

        except Exception as e:
            self._log.error(f"Error processing order update: {e}")

    # =========================================================================
    # Report Generation
    # =========================================================================

    def _generate_order_accepted(self, order: Order, venue_order_id: str) -> None:
        """Generate order accepted event."""
        self._send_order_status_report(
            order=order,
            venue_order_id=VenueOrderId(venue_order_id),
            order_status=OrderStatus.ACCEPTED,
        )

    def _generate_order_rejected(self, order: Order, reason: str) -> None:
        """Generate order rejected event."""
        self._send_order_status_report(
            order=order,
            venue_order_id=None,
            order_status=OrderStatus.REJECTED,
            reason=reason,
        )

    def _generate_order_canceled(self, order: Order) -> None:
        """Generate order canceled event."""
        venue_order_id = self._client_order_id_to_order_id.get(order.client_order_id)
        self._send_order_status_report(
            order=order,
            venue_order_id=VenueOrderId(venue_order_id) if venue_order_id else None,
            order_status=OrderStatus.CANCELED,
        )

    def _generate_order_filled(self, order: Order, filled_qty: int, avg_price: float) -> None:
        """Generate order filled event."""
        venue_order_id = self._client_order_id_to_order_id.get(order.client_order_id)

        instrument = self._cache.instrument(order.instrument_id)
        if not instrument:
            return

        # Generate fill report
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
        self,
        order: Order,
        venue_order_id: VenueOrderId | None,
        order_status: OrderStatus,
        reason: str = "",
    ) -> None:
        """Send order status report to message bus."""
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
        """Send fill report to message bus."""
        self._msgbus.send(endpoint="ExecEngine.process", msg=report)

    # =========================================================================
    # Helper Methods
    # =========================================================================

    def _venue_to_exchange(self, venue: Venue) -> str:
        """Convert NautilusTrader Venue to Breeze exchange code."""
        mapping = {
            "NSE": "NSE",
            "BSE": "BSE",
            "NFO": "NFO",
            "BFO": "BFO",
        }
        return mapping.get(venue.value, "NSE")

    def _get_product_type(self, instrument_id: InstrumentId) -> str:
        """Determine Breeze product type from instrument."""
        venue = instrument_id.venue.value
        if venue in ["NFO", "BFO"]:
            symbol = str(instrument_id.symbol)
            if "CE" in symbol or "PE" in symbol:
                return "options"
            return "futures"
        return "cash"

    def _map_order_type(self, order_type: OrderType) -> str:
        """Map NautilusTrader order type to Breeze order type."""
        mapping = {
            OrderType.MARKET: "market",
            OrderType.LIMIT: "limit",
            OrderType.STOP_MARKET: "stop_loss",
            OrderType.STOP_LIMIT: "stop_loss",
        }
        return mapping.get(order_type, "limit")

    def _map_time_in_force(self, tif: TimeInForce) -> str:
        """Map NautilusTrader time in force to Breeze validity."""
        mapping = {
            TimeInForce.DAY: "day",
            TimeInForce.IOC: "ioc",
            TimeInForce.GTC: "day",  # Breeze doesn't support GTC, default to day
        }
        return mapping.get(tif, "day")
