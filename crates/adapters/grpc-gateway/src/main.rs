//! NautilusTrader Trading Server
//!
//! gRPC server and HTTP/JSON API for the Trading Terminal UI

use std::sync::Arc;

use nautilus_grpc_gateway::{
    proto::{
        broker::broker_service_server::BrokerServiceServer,
        market_data::market_data_service_server::MarketDataServiceServer,
        order::order_service_server::OrderServiceServer,
        portfolio::portfolio_service_server::PortfolioServiceServer,
        strategy::strategy_service_server::StrategyServiceServer,
    },
    services::{
        BrokerServiceImpl, MarketDataServiceImpl, OrderServiceImpl,
        PortfolioServiceImpl, StrategyServiceImpl,
    },
    http_api::create_http_router,
    ClientRegistry, SharedClients, start_client_sync,
    GatewayConfig,
};
use tonic::transport::Server;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = GatewayConfig::default();
    let grpc_addr: std::net::SocketAddr = format!("{}:{}", config.bind_addr, config.port + 1).parse()?;
    let http_addr: std::net::SocketAddr = format!("{}:{}", config.bind_addr, config.port).parse()?;

    tracing::info!("Starting NautilusTrader Trading Server");
    tracing::info!("  HTTP/JSON API: http://{}", http_addr);
    tracing::info!("  gRPC API: http://{}", grpc_addr);

    // Create shared registry and client store
    let registry = Arc::new(ClientRegistry::new());
    let shared_clients = SharedClients::new();

    // Start background task to sync registry events to shared clients
    let _sync_handle = start_client_sync(Arc::clone(&registry), shared_clients.clone());

    // Create service implementations with shared state
    let broker_service = BrokerServiceImpl::new(Arc::clone(&registry));
    let market_data_service = MarketDataServiceImpl::new(shared_clients.clone());
    let order_service = OrderServiceImpl::new(shared_clients.clone());
    let portfolio_service = PortfolioServiceImpl::new(shared_clients.clone());
    let strategy_service = StrategyServiceImpl::new();

    tracing::info!("Services initialized");

    // Create HTTP router
    let http_router = create_http_router(Arc::clone(&registry), shared_clients);

    // Start HTTP server
    let http_server = async {
        let listener = tokio::net::TcpListener::bind(http_addr).await?;
        tracing::info!("HTTP server listening on {}", http_addr);
        axum::serve(listener, http_router).await
    };

    // Start gRPC server
    let grpc_server = async {
        tracing::info!("gRPC server listening on {}", grpc_addr);
        Server::builder()
            .add_service(BrokerServiceServer::new(broker_service))
            .add_service(MarketDataServiceServer::new(market_data_service))
            .add_service(OrderServiceServer::new(order_service))
            .add_service(PortfolioServiceServer::new(portfolio_service))
            .add_service(StrategyServiceServer::new(strategy_service))
            .serve(grpc_addr)
            .await
    };

    // Run both servers concurrently
    tokio::select! {
        result = http_server => {
            if let Err(e) = result {
                tracing::error!("HTTP server error: {}", e);
            }
        }
        result = grpc_server => {
            if let Err(e) = result {
                tracing::error!("gRPC server error: {}", e);
            }
        }
    }

    Ok(())
}
