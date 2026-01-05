//! Order service implementation with Zerodha integration

use std::sync::Arc;
use tonic::{Request, Response, Status};
use tokio_stream::wrappers::ReceiverStream;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::proto::order::{
    order_service_server::OrderService,
    BracketOrderRequest, BracketOrderResponse, CancelOrderRequest,
    GttCancelRequest, GttOrderRequest, GttOrderResponse, ModifyOrderRequest,
    OrderFilter, OrderRequest, OrderResponse, OrderUpdate, OrderStatus,
    OrderSide, OrderType, ProductType,
};

use crate::registry::SharedClients;
use nautilus_zerodha::ZerodhaHttpClient;
use nautilus_zerodha::enums::{Exchange, TransactionType, Product, OrderType as ZerodhaOrderType, Validity};

/// Order service implementation with Zerodha support
pub struct OrderServiceImpl {
    /// Shared Zerodha clients
    clients: SharedClients,
}

impl std::fmt::Debug for OrderServiceImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrderServiceImpl").finish()
    }
}

impl OrderServiceImpl {
    pub fn new(clients: SharedClients) -> Self {
        Self { clients }
    }

    /// Get client for a specific broker
    async fn get_client(&self, broker_id: &str) -> Result<Arc<ZerodhaHttpClient>, Status> {
        self.clients.get(broker_id).await
            .ok_or_else(|| Status::not_found(format!("Broker not found: {}", broker_id)))
    }

    /// Convert proto exchange to Zerodha exchange
    fn parse_exchange(exchange: &str) -> Result<Exchange, Status> {
        match exchange.to_uppercase().as_str() {
            "NSE" => Ok(Exchange::NSE),
            "BSE" => Ok(Exchange::BSE),
            "NFO" => Ok(Exchange::NFO),
            "BFO" => Ok(Exchange::BFO),
            "MCX" => Ok(Exchange::MCX),
            "CDS" => Ok(Exchange::CDS),
            _ => Err(Status::invalid_argument(format!("Invalid exchange: {}", exchange))),
        }
    }

    /// Convert proto order side to Zerodha transaction type
    fn to_transaction_type(side: i32) -> TransactionType {
        match OrderSide::try_from(side) {
            Ok(OrderSide::Buy) => TransactionType::BUY,
            Ok(OrderSide::Sell) => TransactionType::SELL,
            _ => TransactionType::BUY, // Default
        }
    }

    /// Convert proto product type to Zerodha product
    fn to_product(product: i32) -> Product {
        match ProductType::try_from(product) {
            Ok(ProductType::Mis) => Product::MIS,
            Ok(ProductType::Cnc) => Product::CNC,
            Ok(ProductType::Nrml) => Product::NRML,
            _ => Product::MIS, // Default
        }
    }

    /// Convert proto order type to Zerodha order type
    fn to_zerodha_order_type(order_type: i32) -> ZerodhaOrderType {
        match OrderType::try_from(order_type) {
            Ok(OrderType::Market) => ZerodhaOrderType::MARKET,
            Ok(OrderType::Limit) => ZerodhaOrderType::LIMIT,
            Ok(OrderType::Sl) => ZerodhaOrderType::SL,
            Ok(OrderType::SlM) => ZerodhaOrderType::SLM,
            _ => ZerodhaOrderType::MARKET, // Default
        }
    }

    /// Convert Zerodha order status to proto status
    fn from_zerodha_status(status: &nautilus_zerodha::enums::OrderStatus) -> OrderStatus {
        match status {
            nautilus_zerodha::enums::OrderStatus::COMPLETE => OrderStatus::Complete,
            nautilus_zerodha::enums::OrderStatus::CANCELLED => OrderStatus::Cancelled,
            nautilus_zerodha::enums::OrderStatus::REJECTED => OrderStatus::Rejected,
            nautilus_zerodha::enums::OrderStatus::OPEN => OrderStatus::Open,
            nautilus_zerodha::enums::OrderStatus::TriggerPending => OrderStatus::Pending,
        }
    }

    /// Convert Zerodha transaction type to proto order side
    fn from_zerodha_side(txn_type: &TransactionType) -> OrderSide {
        match txn_type {
            TransactionType::BUY => OrderSide::Buy,
            TransactionType::SELL => OrderSide::Sell,
        }
    }

    /// Convert Zerodha product to proto product type
    fn from_zerodha_product(product: &Product) -> ProductType {
        match product {
            Product::MIS => ProductType::Mis,
            Product::CNC => ProductType::Cnc,
            Product::NRML => ProductType::Nrml,
            Product::CO => ProductType::Mis,
            Product::BO => ProductType::Mis,
        }
    }

    /// Convert Zerodha order type to proto order type
    fn from_zerodha_order_type(order_type: &ZerodhaOrderType) -> OrderType {
        match order_type {
            ZerodhaOrderType::MARKET => OrderType::Market,
            ZerodhaOrderType::LIMIT => OrderType::Limit,
            ZerodhaOrderType::SL => OrderType::Sl,
            ZerodhaOrderType::SLM => OrderType::SlM,
        }
    }
}

#[tonic::async_trait]
impl OrderService for OrderServiceImpl {
    async fn place_order(
        &self,
        request: Request<OrderRequest>,
    ) -> Result<Response<OrderResponse>, Status> {
        let order = request.into_inner();
        tracing::info!("Placing order for {} on {}", order.symbol, order.exchange);

        let client = self.get_client(&order.broker_id).await?;
        let exchange = Self::parse_exchange(&order.exchange)?;
        let transaction_type = Self::to_transaction_type(order.side);
        let product = Self::to_product(order.product);
        let order_type = Self::to_zerodha_order_type(order.order_type);

        let price = if order.price > 0.0 {
            Some(Decimal::from_f64_retain(order.price).unwrap_or_default())
        } else {
            None
        };

        let trigger_price = if order.trigger_price > 0.0 {
            Some(Decimal::from_f64_retain(order.trigger_price).unwrap_or_default())
        } else {
            None
        };

        let validity = match order.validity {
            1 => Some(Validity::IOC),
            _ => Some(Validity::DAY),
        };

        let tag = if order.tag.is_empty() { None } else { Some(order.tag.as_str()) };

        match client.place_order(
            exchange,
            &order.symbol,
            transaction_type,
            order.quantity as u32,
            product,
            order_type,
            price,
            trigger_price,
            validity,
            None, // disclosed_quantity
            tag,
        ).await {
            Ok(order_id) => {
                tracing::info!("Order placed successfully: {}", order_id);
                Ok(Response::new(OrderResponse {
                    success: true,
                    order_id,
                    message: "Order placed successfully".to_string(),
                    status: OrderStatus::Open.into(),
                }))
            }
            Err(e) => {
                tracing::error!("Order placement failed: {}", e);
                Ok(Response::new(OrderResponse {
                    success: false,
                    order_id: String::new(),
                    message: format!("Order failed: {}", e),
                    status: OrderStatus::Rejected.into(),
                }))
            }
        }
    }

    async fn modify_order(
        &self,
        request: Request<ModifyOrderRequest>,
    ) -> Result<Response<OrderResponse>, Status> {
        let modify = request.into_inner();
        tracing::info!("Modifying order {}", modify.order_id);

        let client = self.get_client(&modify.broker_id).await?;

        let mut params = std::collections::HashMap::new();
        params.insert("order_id".to_string(), modify.order_id.clone());

        if modify.quantity > 0 {
            params.insert("quantity".to_string(), modify.quantity.to_string());
        }
        if modify.price > 0.0 {
            params.insert("price".to_string(), modify.price.to_string());
        }
        if modify.trigger_price > 0.0 {
            params.insert("trigger_price".to_string(), modify.trigger_price.to_string());
        }

        match client.modify_order(params).await {
            Ok(response) => {
                tracing::info!("Order modified successfully: {}", response.order_id);
                Ok(Response::new(OrderResponse {
                    success: true,
                    order_id: response.order_id,
                    message: "Order modified successfully".to_string(),
                    status: OrderStatus::Open.into(),
                }))
            }
            Err(e) => {
                tracing::error!("Order modification failed: {}", e);
                Ok(Response::new(OrderResponse {
                    success: false,
                    order_id: modify.order_id,
                    message: format!("Modification failed: {}", e),
                    status: OrderStatus::Rejected.into(),
                }))
            }
        }
    }

    async fn cancel_order(
        &self,
        request: Request<CancelOrderRequest>,
    ) -> Result<Response<OrderResponse>, Status> {
        let cancel = request.into_inner();
        tracing::info!("Cancelling order {}", cancel.order_id);

        let client = self.get_client(&cancel.broker_id).await?;

        match client.cancel_order(&cancel.order_id).await {
            Ok(response) => {
                tracing::info!("Order cancelled successfully: {}", response.order_id);
                Ok(Response::new(OrderResponse {
                    success: true,
                    order_id: response.order_id,
                    message: "Order cancelled successfully".to_string(),
                    status: OrderStatus::Cancelled.into(),
                }))
            }
            Err(e) => {
                tracing::error!("Order cancellation failed: {}", e);
                Ok(Response::new(OrderResponse {
                    success: false,
                    order_id: cancel.order_id,
                    message: format!("Cancellation failed: {}", e),
                    status: OrderStatus::Rejected.into(),
                }))
            }
        }
    }

    type GetOrdersStream = ReceiverStream<Result<OrderUpdate, Status>>;

    async fn get_orders(
        &self,
        request: Request<OrderFilter>,
    ) -> Result<Response<Self::GetOrdersStream>, Status> {
        let filter = request.into_inner();
        tracing::info!("Fetching orders for broker {}", filter.broker_id);

        let client = self.get_client(&filter.broker_id).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(128);
        let broker_id = filter.broker_id.clone();

        // Fetch orders once and send them
        match client.get_orders().await {
            Ok(orders) => {
                for order in orders {
                    let status = Self::from_zerodha_status(&order.status);
                    let side = Self::from_zerodha_side(&order.transaction_type);
                    let product = Self::from_zerodha_product(&order.product);
                    let order_type = Self::from_zerodha_order_type(&order.order_type);

                    let update = OrderUpdate {
                        order_id: order.order_id.clone(),
                        symbol: order.tradingsymbol.clone(),
                        exchange: order.exchange.to_string(),
                        side: side.into(),
                        order_type: order_type.into(),
                        product: product.into(),
                        quantity: order.quantity as i32,
                        filled_quantity: order.filled_quantity as i32,
                        price: order.price.to_f64().unwrap_or(0.0),
                        trigger_price: order.trigger_price.unwrap_or_default().to_f64().unwrap_or(0.0),
                        average_price: order.average_price.unwrap_or_default().to_f64().unwrap_or(0.0),
                        status: status.into(),
                        status_message: order.status_message.clone().unwrap_or_default(),
                        timestamp: order.order_timestamp.timestamp_millis(),
                        broker_id: broker_id.clone(),
                        tag: order.tag.clone().unwrap_or_default(),
                    };

                    let _ = tx.send(Ok(update)).await;
                }
            }
            Err(e) => {
                tracing::error!("Failed to get orders: {}", e);
            }
        }

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn place_bracket_order(
        &self,
        _request: Request<BracketOrderRequest>,
    ) -> Result<Response<BracketOrderResponse>, Status> {
        tracing::info!("Placing bracket order");

        // TODO: Zerodha bracket orders are deprecated, use regular orders with SL
        Ok(Response::new(BracketOrderResponse {
            success: false,
            entry_order_id: String::new(),
            stoploss_order_id: String::new(),
            target_order_id: String::new(),
            message: "Bracket orders are not supported by Zerodha. Use regular orders with SL.".to_string(),
        }))
    }

    async fn place_gtt_order(
        &self,
        request: Request<GttOrderRequest>,
    ) -> Result<Response<GttOrderResponse>, Status> {
        let gtt = request.into_inner();
        tracing::info!("Placing GTT order for {}", gtt.symbol);

        // TODO: Implement GTT order placement via Zerodha API
        Ok(Response::new(GttOrderResponse {
            success: false,
            gtt_id: String::new(),
            message: "GTT orders not yet implemented".to_string(),
        }))
    }

    async fn cancel_gtt_order(
        &self,
        request: Request<GttCancelRequest>,
    ) -> Result<Response<OrderResponse>, Status> {
        let cancel = request.into_inner();
        tracing::info!("Cancelling GTT order {}", cancel.gtt_id);

        // TODO: Implement GTT order cancellation via Zerodha API
        Ok(Response::new(OrderResponse {
            success: false,
            order_id: cancel.gtt_id,
            message: "GTT order cancellation not yet implemented".to_string(),
            status: OrderStatus::Rejected.into(),
        }))
    }
}
