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
Execution client for Zerodha integration.

This module provides the LiveExecClient implementation for NautilusTrader,
enabling order placement, modification, and cancellation through Zerodha's API.
"""

from __future__ import annotations

import asyncio
from decimal import Decimal
from typing import Any

from nautilus_trader.adapters.zerodha.config import ZerodhaExecClientConfig
from nautilus_trader.adapters.zerodha.providers import ZerodhaInstrumentProvider
from nautilus_trader.common.component import LiveClock
from nautilus_trader.common.component import MessageBus
from nautilus_trader.core.uuid import UUID4
from nautilus_trader.execution.client import LiveExecClient
from nautilus_trader.execution.messages import CancelOrder
from nautilus_trader.execution.messages import ModifyOrder
from nautilus_trader.execution.messages import SubmitOrder
from nautilus_trader.execution.reports import ExecutionReport
from nautilus_trader.execution.reports import FillReport
from nautilus_trader.execution.reports import OrderStatusReport
from nautilus_trader.execution.reports import PositionStatusReport
from nautilus_trader.model.currencies import INR
from nautilus_trader.model.enums import AccountType
from nautilus_trader.model.enums import LiquiditySide
from nautilus_trader.model.enums import OrderSide
from nautilus_trader.model.enums import OrderStatus
from nautilus_trader.model.enums import OrderType
from nautilus_trader.model.identifiers import AccountId
from nautilus_trader.model.identifiers import ClientOrderId
from nautilus_trader.model.identifiers import InstrumentId
from nautilus_trader.model.identifiers import StrategyId
from nautilus_trader.model.identifiers import TradeId
from nautilus_trader.model.identifiers import VenueOrderId
from nautilus_trader.model.objects import AccountBalance
from nautilus_trader.model.objects import Currency
from nautilus_trader.model.objects import MarginBalance
from nautilus_trader.model.objects import Money
from nautilus_trader.model.objects import Price
from nautilus_trader.model.objects import Quantity
from nautilus_trader.model.orders import Order
from nautilus_trader.model.position import Position


class ZerodhaLiveExecClient(LiveExecClient):
    """
    Provides a live execution client for Zerodha.
    
    This client handles order placement, modification, cancellation, and position 
    tracking through Zerodha's Kite Connect API.
    
    Parameters
    ----------
    loop : asyncio.AbstractEventLoop
        The event loop for the client.
    client : Any
        The Zerodha HTTP client (Rust-based).
    msgbus : MessageBus  
        The message bus for the client.
    cache : Cache
        The cache for the client.
    clock : LiveClock
        The clock for the client.
    config : ZerodhaExecClientConfig
        The configuration for the client.
    instrument_provider : ZerodhaInstrumentProvider, optional
        The instrument provider for the client.
    """

    def __init__(
        self,
        loop: asyncio.AbstractEventLoop,
        client: Any,
        msgbus: MessageBus,
        cache: Any,
        clock: LiveClock,
        config: ZerodhaExecClientConfig,
        instrument_provider: ZerodhaInstrumentProvider | None = None,
    ) -> None:
        # Account setup
        account_id = AccountId(f"ZERODHA-{config.account_id}")
        
        super().__init__(
            loop=loop,
            client_id=config.client_id,
            venue=None,  # Multi-venue support
            oms_type=config.oms_type,
            account_type=AccountType.CASH,  # or MARGIN based on config
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
        
        # Position tracking
        self._positions: dict[InstrumentId, Position] = {}
        
        # Account information
        self._account_id = account_id
        self._account_balance = Decimal("0")
        self._available_margin = Decimal("0")
        
        # Order status tracking
        self._order_status_manager = None  # Will be set up during connection
        self._order_update_task: asyncio.Task | None = None
        
        self._log.info("✅ ZerodhaLiveExecClient initialized")

    # =========================================================================
    # Connection Lifecycle
    # =========================================================================

    async def _connect(self) -> None:
        """Connect to Zerodha execution services."""
        try:
            self._log.info("🔌 Connecting to Zerodha execution services...")
            
            # Test connection with account info
            await self._update_account_info()
            
            # Load existing orders and positions
            await self._load_existing_orders()
            await self._load_existing_positions()
            
            # Start order status monitoring
            await self._start_order_monitoring()
            
            self._log.info("✅ Connected to Zerodha execution services")
            
        except Exception as e:
            self._log.error(f"❌ Failed to connect to Zerodha: {e}")
            raise

    async def _disconnect(self) -> None:
        """Disconnect from Zerodha execution services."""
        self._log.info("🔌 Disconnecting from Zerodha execution services...")
        
        # Stop order monitoring
        if self._order_update_task:
            self._order_update_task.cancel()
            try:
                await self._order_update_task
            except asyncio.CancelledError:
                pass
        
        # Clean up resources
        self._order_id_to_client_order_id.clear()
        self._client_order_id_to_order_id.clear()
        self._positions.clear()
        
        self._log.info("✅ Disconnected from Zerodha execution services")

    # =========================================================================
    # Account Management
    # =========================================================================

    async def _update_account_info(self) -> None:
        """Update account balance and margin information."""
        try:
            balance, margin = await self._client.get_account_info()
            
            self._account_balance = Decimal(str(balance))
            self._available_margin = Decimal(str(margin))
            
            # Generate account state report
            account_balance = AccountBalance(
                total=Money(self._account_balance, INR),
                locked=Money(0, INR),
                free=Money(self._available_margin, INR),
            )
            
            margin_balance = MarginBalance(
                initial=Money(0, INR),
                maintenance=Money(0, INR),
                instrument=Money(self._available_margin, INR),
            )
            
            self.generate_account_state(
                balances=[account_balance],
                margins=[margin_balance],
                reported=True,
                ts_event=self._clock.timestamp_ns(),
            )
            
            self._log.debug(f"💰 Account updated: Balance=₹{self._account_balance:.2f}, "
                           f"Margin=₹{self._available_margin:.2f}")
            
        except Exception as e:
            self._log.error(f"❌ Failed to update account info: {e}")

    async def _load_existing_orders(self) -> None:
        """Load existing orders from Zerodha."""
        try:
            orders = await self._client.get_orders()
            
            for order_data in orders:
                # Convert to NautilusTrader order status report
                status_report = self._create_order_status_report(order_data)
                if status_report:
                    self.generate_order_status_report(status_report)
            
            self._log.info(f"📄 Loaded {len(orders)} existing orders")
            
        except Exception as e:
            self._log.error(f"❌ Failed to load existing orders: {e}")

    async def _load_existing_positions(self) -> None:
        """Load existing positions from Zerodha."""
        try:
            positions = await self._client.get_positions()
            
            position_count = 0
            for segment, position_list in positions.items():
                for position_data in position_list:
                    if position_data.quantity != 0:
                        # Convert to NautilusTrader position status report
                        status_report = self._create_position_status_report(position_data)
                        if status_report:
                            self.generate_position_status_report(status_report)
                            position_count += 1
                            
                            # Cache position for tracking
                            self._cache_position(position_data)
            
            self._log.info(f"🎯 Loaded {position_count} existing positions")
            
        except Exception as e:
            self._log.error(f"❌ Failed to load existing positions: {e}")

    def _cache_position(self, position_data: Any) -> None:
        """Cache position data for tracking."""
        try:
            # Create instrument ID
            if self._instrument_provider:
                instrument_id = self._instrument_provider.get_instrument_id_by_symbol(
                    position_data.tradingsymbol, position_data.exchange
                )
            else:
                from nautilus_trader.model.identifiers import Symbol, Venue
                symbol = Symbol(position_data.tradingsymbol)
                venue = Venue(position_data.exchange)
                instrument_id = InstrumentId(symbol, venue)
            
            # Store position data
            self._positions[instrument_id] = position_data
            
        except Exception as e:
            self._log.error(f"❌ Failed to cache position: {e}")

    # =========================================================================
    # Order Management
    # =========================================================================

    async def _submit_order(self, command: SubmitOrder) -> None:
        """Submit an order to Zerodha."""
        try:
            order = command.order
            
            # Validate order against risk management rules
            if not await self._validate_order_risk(order):
                self._log.error(f"❌ Order {order.client_order_id} failed risk validation")
                self.generate_order_rejected(
                    strategy_id=order.strategy_id,
                    instrument_id=order.instrument_id,
                    client_order_id=order.client_order_id,
                    reason="Failed risk validation - insufficient funds or critical account health",
                    ts_event=self._clock.timestamp_ns(),
                )
                return
            
            # Convert NautilusTrader order to Zerodha format
            order_request = self._create_order_request(order)
            
            # Submit order to Zerodha
            response = await self._client.place_order(order_request)
            
            # Store order ID mapping
            zerodha_order_id = response.order_id
            self._order_id_to_client_order_id[zerodha_order_id] = order.client_order_id
            self._client_order_id_to_order_id[order.client_order_id] = zerodha_order_id
            
            # Generate order accepted report
            venue_order_id = VenueOrderId(zerodha_order_id)
            self.generate_order_accepted(
                strategy_id=order.strategy_id,
                instrument_id=order.instrument_id,
                client_order_id=order.client_order_id,
                venue_order_id=venue_order_id,
                ts_event=self._clock.timestamp_ns(),
            )
            
            self._log.info(f"✅ Order submitted: {order.client_order_id} -> {zerodha_order_id}")
            
        except Exception as e:
            # Generate order rejected report
            self.generate_order_rejected(
                strategy_id=command.order.strategy_id,
                instrument_id=command.order.instrument_id,
                client_order_id=command.order.client_order_id,
                reason=str(e),
                ts_event=self._clock.timestamp_ns(),
            )
            
            self._log.error(f"❌ Order submission failed: {e}")

    async def _modify_order(self, command: ModifyOrder) -> None:
        """Modify an existing order."""
        try:
            # Get Zerodha order ID
            zerodha_order_id = self._client_order_id_to_order_id.get(command.client_order_id)
            if not zerodha_order_id:
                raise ValueError(f"Order not found: {command.client_order_id}")
            
            # Create modification request
            modify_request = {
                "order_id": zerodha_order_id,
            }
            
            if command.quantity:
                modify_request["quantity"] = str(int(command.quantity))
            
            if command.price:
                modify_request["price"] = str(float(command.price))
            
            if command.trigger_price:
                modify_request["trigger_price"] = str(float(command.trigger_price))
            
            # Submit modification
            await self._client.modify_order(modify_request)
            
            # Generate order updated report
            venue_order_id = VenueOrderId(zerodha_order_id)
            self.generate_order_updated(
                strategy_id=command.strategy_id,
                instrument_id=command.instrument_id,
                client_order_id=command.client_order_id,
                venue_order_id=venue_order_id,
                quantity=command.quantity,
                price=command.price,
                trigger_price=command.trigger_price,
                ts_event=self._clock.timestamp_ns(),
            )
            
            self._log.info(f"✅ Order modified: {command.client_order_id}")
            
        except Exception as e:
            self._log.error(f"❌ Order modification failed: {e}")

    async def _cancel_order(self, command: CancelOrder) -> None:
        """Cancel an existing order."""
        try:
            # Get Zerodha order ID
            zerodha_order_id = self._client_order_id_to_order_id.get(command.client_order_id)
            if not zerodha_order_id:
                raise ValueError(f"Order not found: {command.client_order_id}")
            
            # Submit cancellation
            await self._client.cancel_order(zerodha_order_id)
            
            # Generate order canceled report
            venue_order_id = VenueOrderId(zerodha_order_id)
            self.generate_order_canceled(
                strategy_id=command.strategy_id,
                instrument_id=command.instrument_id,
                client_order_id=command.client_order_id,
                venue_order_id=venue_order_id,
                ts_event=self._clock.timestamp_ns(),
            )
            
            self._log.info(f"✅ Order cancelled: {command.client_order_id}")
            
        except Exception as e:
            self._log.error(f"❌ Order cancellation failed: {e}")

    # =========================================================================
    # Helper Methods
    # =========================================================================

    def _create_order_request(self, order: Order) -> dict[str, str]:
        """Convert NautilusTrader order to Zerodha order request."""
        # Get instrument symbol
        if self._instrument_provider:
            token = self._instrument_provider.get_instrument_token(order.instrument_id)
            instrument = self._instrument_provider.get_instrument(order.instrument_id)
            tradingsymbol = str(instrument.symbol) if instrument else str(order.instrument_id.symbol)
        else:
            tradingsymbol = str(order.instrument_id.symbol)
        
        # Convert order side
        transaction_type = "BUY" if order.side == OrderSide.BUY else "SELL"
        
        # Convert order type
        if order.order_type == OrderType.MARKET:
            order_type = "MARKET"
            price = None
        elif order.order_type == OrderType.LIMIT:
            order_type = "LIMIT"
            price = float(order.price) if order.price else None
        elif order.order_type == OrderType.STOP_MARKET:
            order_type = "SL-M"
            price = None
        elif order.order_type == OrderType.STOP_LIMIT:
            order_type = "SL"
            price = float(order.price) if order.price else None
        else:
            order_type = "LIMIT"
            price = float(order.price) if order.price else None
        
        # Create request
        request = {
            "tradingsymbol": tradingsymbol,
            "exchange": str(order.instrument_id.venue),
            "transaction_type": transaction_type,
            "order_type": order_type,
            "product": self._config.default_product_type,
            "validity": self._config.default_validity,
            "quantity": str(int(order.quantity)),
            "tag": str(order.client_order_id),
        }
        
        if price:
            request["price"] = str(price)
        
        if hasattr(order, 'trigger_price') and order.trigger_price:
            request["trigger_price"] = str(float(order.trigger_price))
        
        return request

    def _create_order_status_report(self, order_data: Any) -> OrderStatusReport | None:
        """Create OrderStatusReport from Zerodha order data."""
        try:
            # Convert order status
            status_map = {
                "OPEN": OrderStatus.ACCEPTED,
                "COMPLETE": OrderStatus.FILLED,
                "CANCELLED": OrderStatus.CANCELED,
                "REJECTED": OrderStatus.REJECTED,
                "TRIGGER PENDING": OrderStatus.ACCEPTED,
                "PENDING": OrderStatus.PENDING_SUBMIT,
            }
            
            order_status = status_map.get(order_data.status, OrderStatus.PENDING_SUBMIT)
            
            # Get client order ID from tag or create one
            client_order_id = ClientOrderId(order_data.tag) if order_data.tag else ClientOrderId(order_data.order_id)
            venue_order_id = VenueOrderId(order_data.order_id)
            
            # Create instrument ID
            if self._instrument_provider:
                instrument_id = self._instrument_provider.get_instrument_id(order_data.instrument_token)
            else:
                from nautilus_trader.model.identifiers import Symbol, Venue
                symbol = Symbol(order_data.tradingsymbol)
                venue = Venue(order_data.exchange)
                instrument_id = InstrumentId(symbol, venue)
            
            return OrderStatusReport(
                account_id=self._account_id,
                instrument_id=instrument_id,
                client_order_id=client_order_id,
                venue_order_id=venue_order_id,
                order_side=OrderSide.BUY if order_data.transaction_type == "BUY" else OrderSide.SELL,
                order_type=self._convert_order_type(order_data.order_type),
                quantity=Quantity.from_int(order_data.quantity),
                filled_qty=Quantity.from_int(order_data.filled_quantity),
                remaining_qty=Quantity.from_int(order_data.pending_quantity),
                order_status=order_status,
                price=Price.from_str(str(order_data.price)) if order_data.price else None,
                avg_px=Price.from_str(str(order_data.average_price)) if order_data.average_price else None,
                ts_accepted=self._clock.timestamp_ns(),
                ts_last=self._clock.timestamp_ns(),
                report_id=UUID4(),
                ts_init=self._clock.timestamp_ns(),
            )
            
        except Exception as e:
            self._log.error(f"❌ Failed to create order status report: {e}")
            return None

    def _create_position_status_report(self, position_data: Any) -> PositionStatusReport | None:
        """Create PositionStatusReport from Zerodha position data."""
        try:
            # Create instrument ID
            if self._instrument_provider:
                instrument_id = self._instrument_provider.get_instrument_id_by_symbol(
                    position_data.tradingsymbol, position_data.exchange
                )
            else:
                from nautilus_trader.model.identifiers import Symbol, Venue
                symbol = Symbol(position_data.tradingsymbol)
                venue = Venue(position_data.exchange)
                instrument_id = InstrumentId(symbol, venue)
            
            # Determine position side
            order_side = OrderSide.BUY if position_data.quantity > 0 else OrderSide.SELL
            
            return PositionStatusReport(
                account_id=self._account_id,
                instrument_id=instrument_id,
                position_side=order_side,
                quantity=Quantity.from_int(abs(position_data.quantity)),
                signed_qty=position_data.quantity,
                ts_last=self._clock.timestamp_ns(),
                report_id=UUID4(),
                ts_init=self._clock.timestamp_ns(),
            )
            
        except Exception as e:
            self._log.error(f"❌ Failed to create position status report: {e}")
            return None

    def _convert_order_type(self, zerodha_order_type: str) -> OrderType:
        """Convert Zerodha order type to NautilusTrader OrderType."""
        type_map = {
            "MARKET": OrderType.MARKET,
            "LIMIT": OrderType.LIMIT,
            "SL": OrderType.STOP_LIMIT,
            "SL-M": OrderType.STOP_MARKET,
        }
        return type_map.get(zerodha_order_type, OrderType.LIMIT)

    # =========================================================================
    # Account Queries
    # =========================================================================

    async def generate_account_state(
        self,
        balances: list[AccountBalance],
        margins: list[MarginBalance],
        reported: bool,
        ts_event: int,
    ) -> None:
        """Generate account state event."""
        # Implementation would call parent method
        super().generate_account_state(balances, margins, reported, ts_event)

    # =========================================================================
    # Order Status Monitoring  
    # =========================================================================

    async def _start_order_monitoring(self) -> None:
        """Start order status monitoring."""
        try:
            # Initialize order status manager
            self._order_status_manager = await self._client.create_order_status_manager()
            
            # Start order update processing task
            self._order_update_task = asyncio.create_task(self._process_order_updates())
            
            self._log.info("📊 Order status monitoring started")
            
        except Exception as e:
            self._log.error(f"❌ Failed to start order monitoring: {e}")

    async def _process_order_updates(self) -> None:
        """Process order status updates from WebSocket."""
        if not self._order_status_manager:
            return
            
        try:
            # Get order update receiver from Rust client
            update_receiver = await self._order_status_manager.get_update_receiver()
            
            while True:
                # Receive order update
                update = await update_receiver.recv()
                if update is None:
                    break
                    
                await self._handle_order_update(update)
                
        except asyncio.CancelledError:
            self._log.info("📊 Order update processing cancelled")
        except Exception as e:
            self._log.error(f"❌ Error processing order updates: {e}")

    async def _handle_order_update(self, update: Any) -> None:
        """Handle individual order status update."""
        try:
            order_id = update.order_id
            client_order_id = self._order_id_to_client_order_id.get(order_id)
            
            if not client_order_id:
                self._log.warning(f"⚠️  Received update for unknown order: {order_id}")
                return
            
            # Convert status to NautilusTrader format
            status_map = {
                "OPEN": OrderStatus.ACCEPTED,
                "COMPLETE": OrderStatus.FILLED,
                "CANCELLED": OrderStatus.CANCELED,
                "REJECTED": OrderStatus.REJECTED,
                "TRIGGER PENDING": OrderStatus.ACCEPTED,
                "PENDING": OrderStatus.PENDING_SUBMIT,
            }
            
            nautilus_status = status_map.get(str(update.status), OrderStatus.ACCEPTED)
            
            # Get cached order
            order = self._cache.order(client_order_id)
            if not order:
                self._log.warning(f"⚠️  Order not found in cache: {client_order_id}")
                return
            
            # Generate appropriate event based on status change
            venue_order_id = VenueOrderId(order_id)
            
            if nautilus_status == OrderStatus.FILLED and update.filled_quantity > 0:
                # Generate fill report
                fill_report = FillReport(
                    account_id=self._account_id,
                    instrument_id=order.instrument_id,
                    venue_order_id=venue_order_id,
                    trade_id=TradeId(f"{order_id}_{update.filled_quantity}"),
                    order_side=order.side,
                    last_qty=Quantity.from_int(update.filled_quantity),
                    last_px=Price.from_str(str(update.average_price)) if update.average_price else order.price,
                    liquidity_side=LiquiditySide.NO_LIQUIDITY_SIDE,
                    ts_event=self._clock.timestamp_ns(),
                    report_id=UUID4(),
                    ts_init=self._clock.timestamp_ns(),
                )
                
                self.generate_fill_report(fill_report)
                
                # Update position tracking
                await self._update_position_on_fill(order, update)
                
            elif nautilus_status == OrderStatus.CANCELED:
                # Generate order canceled event
                self.generate_order_canceled(
                    strategy_id=order.strategy_id,
                    instrument_id=order.instrument_id,
                    client_order_id=client_order_id,
                    venue_order_id=venue_order_id,
                    ts_event=self._clock.timestamp_ns(),
                )
                
            elif nautilus_status == OrderStatus.REJECTED:
                # Generate order rejected event
                self.generate_order_rejected(
                    strategy_id=order.strategy_id,
                    instrument_id=order.instrument_id,
                    client_order_id=client_order_id,
                    reason=update.status_message or "Order rejected by exchange",
                    ts_event=self._clock.timestamp_ns(),
                )
            
            self._log.debug(f"📋 Processed order update: {client_order_id} -> {nautilus_status}")
            
        except Exception as e:
            self._log.error(f"❌ Error handling order update: {e}")

    async def _update_position_on_fill(self, order: Order, update: Any) -> None:
        """Update position tracking when an order is filled."""
        try:
            instrument_id = order.instrument_id
            
            # Get current position or create new one
            current_position = self._positions.get(instrument_id)
            
            if current_position is None:
                # Create new position
                new_position = self._create_position_from_fill(order, update)
                self._positions[instrument_id] = new_position
                self._log.info(f"🆕 New position created: {instrument_id} qty={update.filled_quantity}")
            else:
                # Update existing position
                await self._update_existing_position(current_position, order, update)
                self._log.debug(f"📊 Position updated: {instrument_id}")
            
            # Refresh position from API for accuracy
            await self._refresh_position(instrument_id)
            
        except Exception as e:
            self._log.error(f"❌ Error updating position on fill: {e}")

    def _create_position_from_fill(self, order: Order, update: Any) -> Any:
        """Create new position object from order fill."""
        # Create a basic position structure
        # This would be enhanced with actual Zerodha position data structure
        return {
            'tradingsymbol': str(order.instrument_id.symbol),
            'exchange': str(order.instrument_id.venue),
            'quantity': update.filled_quantity if order.side == OrderSide.BUY else -update.filled_quantity,
            'average_price': update.average_price or float(order.price) if order.price else 0.0,
            'last_price': update.average_price or float(order.price) if order.price else 0.0,
            'pnl': 0.0,
            'unrealised': 0.0,
            'realised': 0.0,
        }

    async def _update_existing_position(self, position: Any, order: Order, update: Any) -> None:
        """Update existing position with new fill."""
        try:
            fill_qty = update.filled_quantity
            if order.side == OrderSide.SELL:
                fill_qty = -fill_qty
            
            # Update position quantity
            old_qty = position.get('quantity', 0)
            new_qty = old_qty + fill_qty
            position['quantity'] = new_qty
            
            # Update average price calculation
            fill_price = update.average_price or float(order.price) if order.price else 0.0
            old_avg_price = position.get('average_price', 0.0)
            
            if new_qty != 0:
                # Calculate new average price
                total_value = (old_qty * old_avg_price) + (fill_qty * fill_price)
                position['average_price'] = total_value / new_qty
            
            self._log.debug(f"📈 Position updated: {order.instrument_id} "
                           f"qty {old_qty} -> {new_qty} @ ₹{position['average_price']:.2f}")
            
        except Exception as e:
            self._log.error(f"❌ Error updating existing position: {e}")

    async def _refresh_position(self, instrument_id: InstrumentId) -> None:
        """Refresh position data from Zerodha API."""
        try:
            # Fetch latest positions
            positions = await self._client.get_positions()
            
            # Find the specific position
            symbol = str(instrument_id.symbol)
            venue = str(instrument_id.venue)
            
            for segment, position_list in positions.items():
                for position_data in position_list:
                    if (position_data.tradingsymbol == symbol and 
                        position_data.exchange == venue):
                        
                        # Update cached position
                        self._positions[instrument_id] = position_data
                        
                        # Generate position status report if quantity changed
                        if position_data.quantity != 0:
                            status_report = self._create_position_status_report(position_data)
                            if status_report:
                                self.generate_position_status_report(status_report)
                        
                        self._log.debug(f"🔄 Position refreshed: {instrument_id}")
                        return
            
            # Position not found - might be closed
            if instrument_id in self._positions:
                del self._positions[instrument_id]
                self._log.info(f"🚫 Position closed: {instrument_id}")
                
        except Exception as e:
            self._log.error(f"❌ Error refreshing position: {e}")

    # =========================================================================
    # Position Monitoring
    # =========================================================================

    async def _start_position_monitoring(self) -> None:
        """Start periodic position monitoring."""
        asyncio.create_task(self._monitor_positions())

    async def _monitor_positions(self) -> None:
        """Monitor positions for P&L changes."""
        while True:
            try:
                await asyncio.sleep(self._config.position_update_interval)
                await self._update_all_positions()
            except asyncio.CancelledError:
                break
            except Exception as e:
                self._log.error(f"❌ Error monitoring positions: {e}")
                await asyncio.sleep(5)

    async def _update_all_positions(self) -> None:
        """Update all position data from Zerodha."""
        try:
            positions = await self._client.get_positions()
            
            # Track which positions we've seen
            seen_positions = set()
            
            for segment, position_list in positions.items():
                for position_data in position_list:
                    if position_data.quantity == 0:
                        continue
                        
                    # Create instrument ID
                    if self._instrument_provider:
                        instrument_id = self._instrument_provider.get_instrument_id_by_symbol(
                            position_data.tradingsymbol, position_data.exchange
                        )
                    else:
                        from nautilus_trader.model.identifiers import Symbol, Venue
                        symbol = Symbol(position_data.tradingsymbol)
                        venue = Venue(position_data.exchange)
                        instrument_id = InstrumentId(symbol, venue)
                    
                    seen_positions.add(instrument_id)
                    
                    # Check if position changed
                    old_position = self._positions.get(instrument_id)
                    if (not old_position or 
                        old_position.quantity != position_data.quantity or
                        abs(old_position.pnl - position_data.pnl) > 0.01):
                        
                        # Update cached position
                        self._positions[instrument_id] = position_data
                        
                        # Generate position status report
                        status_report = self._create_position_status_report(position_data)
                        if status_report:
                            self.generate_position_status_report(status_report)
                        
                        self._log.debug(f"📊 Position updated: {instrument_id} "
                                      f"qty={position_data.quantity} P&L=₹{position_data.pnl:.2f}")
            
            # Remove positions that are no longer active
            closed_positions = set(self._positions.keys()) - seen_positions
            for instrument_id in closed_positions:
                del self._positions[instrument_id]
                self._log.info(f"🚫 Position closed: {instrument_id}")
                
        except Exception as e:
            self._log.error(f"❌ Error updating all positions: {e}")

    # =========================================================================
    # Periodic Updates
    # =========================================================================

    async def _update_account_state(self) -> None:
        """Periodically update account state."""
        while True:
            try:
                await asyncio.sleep(self._config.account_update_interval)
                await self._update_account_info()
            except asyncio.CancelledError:
                break
            except Exception as e:
                self._log.error(f"❌ Error updating account state: {e}")
                await asyncio.sleep(5)  # Wait before retrying

    async def _monitor_positions(self) -> None:
        """Periodically monitor positions."""
        while True:
            try:
                await asyncio.sleep(self._config.position_update_interval)
                await self._update_all_positions()
            except asyncio.CancelledError:
                break
            except Exception as e:
                self._log.error(f"❌ Error monitoring positions: {e}")
                await asyncio.sleep(5)  # Wait before retrying

    # =========================================================================
    # Account Information Methods
    # =========================================================================

    async def get_user_profile(self) -> Any:
        """Get user profile information."""
        try:
            self._log.info("👤 Fetching user profile...")
            profile = await self._exec_client.get_user_profile()
            self._log.info(f"✅ Profile loaded for user: {profile.user_name}")
            return profile
        except Exception as e:
            self._log.error(f"❌ Failed to get user profile: {e}")
            raise

    async def get_account_summary(self) -> Any:
        """Get comprehensive account summary including profile and margins."""
        try:
            self._log.info("📊 Fetching account summary...")
            summary = await self._exec_client.get_account_summary()
            
            # Update cached values
            self._account_balance = summary.margins.available_cash
            self._available_margin = summary.margins.available.net
            
            self._log.info(f"💰 Account Balance: ₹{summary.margins.available_cash:.2f}")
            self._log.info(f"💳 Available Margin: ₹{summary.margins.available.net:.2f}")
            
            return summary
        except Exception as e:
            self._log.error(f"❌ Failed to get account summary: {e}")
            raise

    async def get_account_balance_info(self) -> Any:
        """Get simplified account balance information."""
        try:
            self._log.info("💰 Fetching account balance...")
            balance = await self._exec_client.get_account_balance_info()
            
            # Update cached values
            self._account_balance = balance.available_cash
            self._available_margin = balance.net_balance
            
            return balance
        except Exception as e:
            self._log.error(f"❌ Failed to get account balance: {e}")
            raise

    async def check_account_health(self) -> str:
        """Check account health and return status."""
        try:
            health_status = await self._exec_client.check_account_health()
            
            status_str = {
                "Healthy": "healthy",
                "LowCash": "low_cash", 
                "Warning": "warning",
                "Critical": "critical"
            }.get(str(health_status), "unknown")
            
            if status_str in ("warning", "critical"):
                self._log.warning(f"⚠️ Account health status: {status_str}")
            elif status_str == "low_cash":
                self._log.warning(f"💸 Account health status: {status_str}")
            else:
                self._log.debug(f"✅ Account health status: {status_str}")
                
            return status_str
        except Exception as e:
            self._log.error(f"❌ Failed to check account health: {e}")
            return "error"

    async def get_margin_utilization(self) -> float:
        """Get current margin utilization percentage."""
        try:
            utilization = await self._exec_client.get_margin_utilization()
            
            if utilization > 75.0:
                self._log.warning(f"⚠️ High margin utilization: {utilization:.1f}%")
            else:
                self._log.debug(f"📊 Margin utilization: {utilization:.1f}%")
                
            return utilization
        except Exception as e:
            self._log.error(f"❌ Failed to get margin utilization: {e}")
            return 0.0

    async def check_order_affordability(self, order_value: float, product_type: str = "MIS") -> bool:
        """Check if sufficient funds are available for order."""
        try:
            can_afford = await self._exec_client.check_order_affordability(order_value, product_type)
            
            if not can_afford:
                self._log.warning(f"💰 Insufficient funds for order value: ₹{order_value:.2f}")
            else:
                self._log.debug(f"✅ Sufficient funds for order value: ₹{order_value:.2f}")
                
            return can_afford
        except Exception as e:
            self._log.error(f"❌ Failed to check order affordability: {e}")
            return False

    async def refresh_account_info(self) -> None:
        """Manually refresh account information."""
        try:
            await self._exec_client.refresh_account_info()
            await self._update_account_info()  # Update NautilusTrader account state
            self._log.info("🔄 Account information refreshed")
        except Exception as e:
            self._log.error(f"❌ Failed to refresh account info: {e}")

    # =========================================================================
    # Risk Management Integration
    # =========================================================================

    async def _validate_order_risk(self, order: SubmitOrder) -> bool:
        """Validate order against risk management rules."""
        try:
            # Calculate order value
            price = float(order.price) if order.price else 0.0
            if price == 0.0 and order.order_type != OrderType.MARKET:
                self._log.warning(f"⚠️ No price for non-market order: {order.client_order_id}")
                return False
                
            # Estimate order value (for market orders, use last price or approximation)
            order_value = price * float(order.quantity)
            if order_value == 0.0:
                order_value = 1000.0 * float(order.quantity)  # Conservative estimate
            
            # Check affordability
            product_type = getattr(self._config, 'default_product_type', 'MIS')
            can_afford = await self.check_order_affordability(order_value, product_type)
            
            if not can_afford:
                self._log.error(f"❌ Order {order.client_order_id} rejected - insufficient funds")
                return False
            
            # Check account health
            health_status = await self.check_account_health()
            if health_status == "critical":
                self._log.error(f"❌ Order {order.client_order_id} rejected - critical account health")
                return False
                
            return True
            
        except Exception as e:
            self._log.error(f"❌ Risk validation failed for order {order.client_order_id}: {e}")
            return False