//! Portfolio service implementation with Zerodha integration

use std::sync::Arc;
use tonic::{Request, Response, Status};
use tokio_stream::wrappers::ReceiverStream;
use rust_decimal::prelude::ToPrimitive;

use crate::proto::portfolio::{
    portfolio_service_server::PortfolioService,
    DaySummary, DaySummaryRequest, HoldingsList, HoldingsRequest,
    MarginRequest, MarginUpdate, PnLRequest, PnLUpdate,
    PositionRequest, PositionUpdate,
};

use crate::registry::SharedClients;
use nautilus_zerodha::ZerodhaHttpClient;
use nautilus_zerodha::enums::OrderStatus as ZerodhaOrderStatus;

/// Portfolio service implementation with Zerodha support
pub struct PortfolioServiceImpl {
    /// Shared Zerodha clients
    clients: SharedClients,
}

impl std::fmt::Debug for PortfolioServiceImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortfolioServiceImpl").finish()
    }
}

impl PortfolioServiceImpl {
    pub fn new(clients: SharedClients) -> Self {
        Self { clients }
    }

    /// Get client for a specific broker
    async fn get_client(&self, broker_id: &str) -> Result<Arc<ZerodhaHttpClient>, Status> {
        self.clients.get(broker_id).await
            .ok_or_else(|| Status::not_found(format!("Broker not found: {}", broker_id)))
    }
}

#[tonic::async_trait]
impl PortfolioService for PortfolioServiceImpl {
    type GetPositionsStream = ReceiverStream<Result<PositionUpdate, Status>>;

    async fn get_positions(
        &self,
        request: Request<PositionRequest>,
    ) -> Result<Response<Self::GetPositionsStream>, Status> {
        let req = request.into_inner();
        tracing::info!("Fetching positions for broker {}", req.broker_id);

        let client = self.get_client(&req.broker_id).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(128);
        let broker_id = req.broker_id.clone();
        let product_filter = req.product_type;

        // Fetch positions once and send them
        let positions_result = client.get_positions().await;

        match positions_result {
            Ok(positions_map) => {
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64;

                for (_key, positions) in positions_map {
                    for pos in positions {
                        // Filter by product type if specified
                        if !product_filter.is_empty() {
                            let product_str = pos.product.to_string();
                            if product_str != product_filter {
                                continue;
                            }
                        }

                        let pnl_percent = if pos.average_price.is_zero() {
                            0.0
                        } else {
                            (pos.pnl / pos.average_price * rust_decimal::Decimal::from(100))
                                .to_f64()
                                .unwrap_or(0.0)
                        };

                        let update = PositionUpdate {
                            symbol: pos.tradingsymbol.clone(),
                            exchange: pos.exchange.to_string(),
                            product: pos.product.to_string(),
                            quantity: pos.quantity,
                            average_price: pos.average_price.to_f64().unwrap_or(0.0),
                            ltp: pos.last_price.to_f64().unwrap_or(0.0),
                            pnl: pos.pnl.to_f64().unwrap_or(0.0),
                            pnl_percent,
                            value: pos.value.to_f64().unwrap_or(0.0),
                            buy_quantity: pos.buy_quantity as f64,
                            sell_quantity: pos.sell_quantity as f64,
                            buy_value: pos.buy_value.to_f64().unwrap_or(0.0),
                            sell_value: pos.sell_value.to_f64().unwrap_or(0.0),
                            timestamp,
                            broker_id: broker_id.clone(),
                        };

                        let _ = tx.send(Ok(update)).await;
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to get positions: {}", e);
            }
        }

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn get_holdings(
        &self,
        request: Request<HoldingsRequest>,
    ) -> Result<Response<HoldingsList>, Status> {
        let req = request.into_inner();
        tracing::info!("Fetching holdings for broker {}", req.broker_id);

        // TODO: Zerodha doesn't have a separate holdings endpoint in the HTTP client
        // Holdings are typically from the "portfolio/holdings" endpoint
        // For now, return empty list - needs additional API implementation
        Ok(Response::new(HoldingsList {
            holdings: vec![],
            total_invested: 0.0,
            total_current: 0.0,
            total_pnl: 0.0,
            total_pnl_percent: 0.0,
        }))
    }

    type GetMarginsStream = ReceiverStream<Result<MarginUpdate, Status>>;

    async fn get_margins(
        &self,
        request: Request<MarginRequest>,
    ) -> Result<Response<Self::GetMarginsStream>, Status> {
        let req = request.into_inner();
        tracing::info!("Fetching margins for broker {}", req.broker_id);

        let client = self.get_client(&req.broker_id).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(128);
        let broker_id = req.broker_id.clone();

        // Fetch margins once and send them
        let margins_result = client.get_margins().await;

        match margins_result {
            Ok(margins) => {
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64;

                let update = MarginUpdate {
                    broker_id: broker_id.clone(),
                    segment: "equity".to_string(),
                    available: margins.available.live_balance,
                    used: margins.utilised.total,
                    total: margins.available.net,
                    cash: margins.available.cash,
                    collateral: margins.available.collateral,
                    timestamp,
                };

                let _ = tx.send(Ok(update)).await;
            }
            Err(e) => {
                tracing::error!("Failed to get margins: {}", e);
            }
        }

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    type GetPnLStream = ReceiverStream<Result<PnLUpdate, Status>>;

    async fn get_pn_l(
        &self,
        request: Request<PnLRequest>,
    ) -> Result<Response<Self::GetPnLStream>, Status> {
        let req = request.into_inner();
        tracing::info!("Fetching P&L for broker {}", req.broker_id);

        let client = self.get_client(&req.broker_id).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(128);
        let broker_id = req.broker_id.clone();

        // Fetch positions and calculate P&L
        let positions_result = client.get_positions().await;

        match positions_result {
            Ok(positions_map) => {
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64;

                let mut realized_pnl = 0.0;
                let mut unrealized_pnl = 0.0;

                for (_key, positions) in positions_map {
                    for pos in positions {
                        realized_pnl += pos.realised.to_f64().unwrap_or(0.0);
                        unrealized_pnl += pos.unrealised.to_f64().unwrap_or(0.0);
                    }
                }

                let update = PnLUpdate {
                    broker_id: broker_id.clone(),
                    realized_pnl,
                    unrealized_pnl,
                    total_pnl: realized_pnl + unrealized_pnl,
                    total_charges: 0.0, // TODO: Calculate from trades
                    timestamp,
                };

                let _ = tx.send(Ok(update)).await;
            }
            Err(e) => {
                tracing::error!("Failed to get P&L: {}", e);
            }
        }

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn get_day_summary(
        &self,
        request: Request<DaySummaryRequest>,
    ) -> Result<Response<DaySummary>, Status> {
        let req = request.into_inner();
        tracing::info!("Fetching day summary for broker {}", req.broker_id);

        let client = self.get_client(&req.broker_id).await?;

        // Get orders and positions to calculate summary
        let orders_result = client.get_orders().await;
        let positions_result = client.get_positions().await;

        let mut total_trades = 0;
        let mut winning_trades = 0;
        let mut losing_trades = 0;
        let mut gross_pnl = 0.0;
        let mut turnover = 0.0;
        let mut max_profit = 0.0_f64;
        let mut max_loss = 0.0_f64;

        // Calculate from orders
        if let Ok(orders) = orders_result {
            for order in orders {
                if matches!(order.status, ZerodhaOrderStatus::COMPLETE) {
                    total_trades += 1;
                    let trade_value = order.average_price.unwrap_or_default().to_f64().unwrap_or(0.0) * order.filled_quantity as f64;
                    turnover += trade_value;
                }
            }
        }

        // Calculate from positions
        if let Ok(positions_map) = positions_result {
            for (_key, positions) in positions_map {
                for pos in positions {
                    let pnl = pos.pnl.to_f64().unwrap_or(0.0);
                    gross_pnl += pnl;

                    if pnl > 0.0 {
                        winning_trades += 1;
                        max_profit = max_profit.max(pnl);
                    } else if pnl < 0.0 {
                        losing_trades += 1;
                        max_loss = max_loss.min(pnl);
                    }
                }
            }
        }

        Ok(Response::new(DaySummary {
            broker_id: req.broker_id,
            total_trades,
            winning_trades,
            losing_trades,
            gross_pnl,
            charges: 0.0, // TODO: Calculate actual charges
            net_pnl: gross_pnl, // gross_pnl - charges
            turnover,
            max_profit_trade: max_profit,
            max_loss_trade: max_loss,
        }))
    }
}
