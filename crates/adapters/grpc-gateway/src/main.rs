//! NautilusTrader Trading Server
//!
//! gRPC server and HTTP/JSON API for the Trading Terminal UI

use std::sync::Arc;

use nautilus_grpc_gateway::{
    proto::{
        ai::ai_service_server::AiServiceServer,
        backtest::backtest_service_server::BacktestServiceServer,
        broker::broker_service_server::BrokerServiceServer,
        market_data::market_data_service_server::MarketDataServiceServer,
        order::order_service_server::OrderServiceServer,
        portfolio::portfolio_service_server::PortfolioServiceServer,
        strategy::strategy_service_server::StrategyServiceServer,
    },
    services::{
        AiServiceImpl, BrokerServiceImpl, MarketDataServiceImpl, OrderServiceImpl,
        PortfolioServiceImpl, StrategyServiceImpl,
    },
    backtest_service::BacktestServiceImpl,
    db::BrokerDb,
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

    // Load saved brokers from database
    match registry.load_saved_brokers().await {
        Ok(count) => tracing::info!("Loaded {} brokers from database", count),
        Err(e) => tracing::warn!("Failed to load saved brokers: {}", e),
    }

    let shared_clients = SharedClients::new();

    // Start background task to sync registry events to shared clients
    let _sync_handle = start_client_sync(Arc::clone(&registry), shared_clients.clone());

    // Create broker database
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let db_path = std::path::Path::new(&home).join(".nautilus").join("brokers.db");
    let broker_db = Arc::new(BrokerDb::new(&db_path).expect("Failed to open broker database"));
    tracing::info!("Broker database opened at {:?}", db_path);

    // Create service implementations with shared state
    let ai_service = Arc::new(AiServiceImpl::new());
    let backtest_service = Arc::new(BacktestServiceImpl::new());
    let broker_service = BrokerServiceImpl::new(Arc::clone(&registry));
    let market_data_service = MarketDataServiceImpl::new(shared_clients.clone());
    let order_service = OrderServiceImpl::new(shared_clients.clone());
    let portfolio_service = PortfolioServiceImpl::new(shared_clients.clone());
    let strategy_service = StrategyServiceImpl::new();

    tracing::info!("Services initialized (AI, Backtest, Broker, MarketData, Order, Portfolio, Strategy)");

    // Create HTTP router
    let http_router = create_http_router(
        Arc::clone(&registry),
        shared_clients,
        Arc::clone(&ai_service),
        Arc::clone(&backtest_service),
        Arc::clone(&broker_db),
    );

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
            .add_service(AiServiceServer::from_arc(ai_service))
            .add_service(BacktestServiceServer::from_arc(backtest_service))
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
