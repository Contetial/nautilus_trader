//! Strategy service implementation

use tonic::{Request, Response, Status};
use tokio_stream::wrappers::ReceiverStream;

use crate::proto::strategy::{
    strategy_service_server::StrategyService,
    BacktestRequest, BacktestUpdate, ListStrategiesRequest, LogEntry,
    Signal, Strategy, StrategyConfig, StrategyId, StrategyList, StrategyStatus,
    StartStrategyRequest,
};

/// Strategy service implementation
#[derive(Debug, Default)]
pub struct StrategyServiceImpl {
    // TODO: Add strategy engine references
}

impl StrategyServiceImpl {
    pub fn new() -> Self {
        Self::default()
    }
}

#[tonic::async_trait]
impl StrategyService for StrategyServiceImpl {
    async fn list_strategies(
        &self,
        _request: Request<ListStrategiesRequest>,
    ) -> Result<Response<StrategyList>, Status> {
        tracing::info!("Listing strategies");

        // TODO: List actual strategies
        Ok(Response::new(StrategyList {
            strategies: vec![],
        }))
    }

    async fn get_strategy(
        &self,
        request: Request<StrategyId>,
    ) -> Result<Response<Strategy>, Status> {
        let id = request.into_inner();
        tracing::info!("Getting strategy {}", id.id);

        // TODO: Get actual strategy
        Err(Status::not_found("Strategy not found"))
    }

    async fn start_strategy(
        &self,
        request: Request<StartStrategyRequest>,
    ) -> Result<Response<StrategyStatus>, Status> {
        let req = request.into_inner();
        tracing::info!("Starting strategy {}", req.id);

        // TODO: Start actual strategy
        Ok(Response::new(StrategyStatus {
            id: req.id,
            state: 0, // IDLE
            message: "Not implemented".to_string(),
            timestamp: 0,
        }))
    }

    async fn stop_strategy(
        &self,
        request: Request<StrategyId>,
    ) -> Result<Response<StrategyStatus>, Status> {
        let id = request.into_inner();
        tracing::info!("Stopping strategy {}", id.id);

        // TODO: Stop actual strategy
        Ok(Response::new(StrategyStatus {
            id: id.id,
            state: 3, // STOPPED
            message: "Stopped".to_string(),
            timestamp: 0,
        }))
    }

    async fn pause_strategy(
        &self,
        request: Request<StrategyId>,
    ) -> Result<Response<StrategyStatus>, Status> {
        let id = request.into_inner();
        tracing::info!("Pausing strategy {}", id.id);

        // TODO: Pause actual strategy
        Ok(Response::new(StrategyStatus {
            id: id.id,
            state: 2, // PAUSED
            message: "Paused".to_string(),
            timestamp: 0,
        }))
    }

    type GetStrategySignalsStream = ReceiverStream<Result<Signal, Status>>;

    async fn get_strategy_signals(
        &self,
        request: Request<StrategyId>,
    ) -> Result<Response<Self::GetStrategySignalsStream>, Status> {
        let id = request.into_inner();
        tracing::info!("Starting signal stream for strategy {}", id.id);

        let (tx, rx) = tokio::sync::mpsc::channel(128);

        // TODO: Stream actual signals
        tokio::spawn(async move {
            let _ = tx;
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    type GetStrategyLogsStream = ReceiverStream<Result<LogEntry, Status>>;

    async fn get_strategy_logs(
        &self,
        request: Request<StrategyId>,
    ) -> Result<Response<Self::GetStrategyLogsStream>, Status> {
        let id = request.into_inner();
        tracing::info!("Starting log stream for strategy {}", id.id);

        let (tx, rx) = tokio::sync::mpsc::channel(128);

        // TODO: Stream actual logs
        tokio::spawn(async move {
            let _ = tx;
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    type RunBacktestStream = ReceiverStream<Result<BacktestUpdate, Status>>;

    async fn run_backtest(
        &self,
        request: Request<BacktestRequest>,
    ) -> Result<Response<Self::RunBacktestStream>, Status> {
        let req = request.into_inner();
        tracing::info!("Running backtest for strategy {}", req.strategy_id);

        let (tx, rx) = tokio::sync::mpsc::channel(128);

        // TODO: Run actual backtest
        tokio::spawn(async move {
            let _ = tx;
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn configure_strategy(
        &self,
        request: Request<StrategyConfig>,
    ) -> Result<Response<StrategyStatus>, Status> {
        let config = request.into_inner();
        tracing::info!("Configuring strategy {}", config.id);

        // TODO: Configure actual strategy
        Ok(Response::new(StrategyStatus {
            id: config.id,
            state: 0, // IDLE
            message: "Configured".to_string(),
            timestamp: 0,
        }))
    }
}
