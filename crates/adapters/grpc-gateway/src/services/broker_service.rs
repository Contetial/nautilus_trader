//! Broker service implementation with Zerodha integration

use std::sync::Arc;
use tonic::{Request, Response, Status};
use tokio_stream::wrappers::ReceiverStream;

use crate::proto::broker::{
    broker_service_server::BrokerService,
    BrokerConfig, BrokerId, BrokerList, BrokerStatus, ConnectionResult, Empty,
    ConnectionState as ProtoConnectionState,
};

use crate::registry::{ClientRegistry, ConnectionState};
use nautilus_zerodha::ZerodhaConfig;

/// Broker service implementation with Zerodha support
pub struct BrokerServiceImpl {
    registry: Arc<ClientRegistry>,
}

impl std::fmt::Debug for BrokerServiceImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrokerServiceImpl").finish()
    }
}

impl BrokerServiceImpl {
    pub fn new(registry: Arc<ClientRegistry>) -> Self {
        Self { registry }
    }

    /// Convert internal connection state to proto
    fn to_proto_state(state: ConnectionState) -> ProtoConnectionState {
        match state {
            ConnectionState::Offline => ProtoConnectionState::Offline,
            ConnectionState::Connecting => ProtoConnectionState::Connecting,
            ConnectionState::Online => ProtoConnectionState::Online,
            ConnectionState::Error => ProtoConnectionState::Error,
        }
    }

    /// Create a Zerodha config from broker config
    fn create_zerodha_config(config: &BrokerConfig) -> Result<ZerodhaConfig, Status> {
        let api_key = config.credentials.get("api_key")
            .ok_or_else(|| Status::invalid_argument("Missing api_key in credentials"))?;
        let api_secret = config.credentials.get("api_secret")
            .ok_or_else(|| Status::invalid_argument("Missing api_secret in credentials"))?;
        let access_token = config.credentials.get("access_token");

        Ok(ZerodhaConfig::new(
            api_key.clone(),
            api_secret.clone(),
            access_token.cloned(),
        ))
    }
}

#[tonic::async_trait]
impl BrokerService for BrokerServiceImpl {
    async fn list_brokers(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<BrokerList>, Status> {
        let brokers = self.registry.list_brokers().await
            .iter()
            .map(|info| BrokerStatus {
                id: info.id.clone(),
                name: info.name.clone(),
                state: Self::to_proto_state(info.state).into(),
                error_message: info.error_message.clone().unwrap_or_default(),
                last_connected_timestamp: info.last_connected.unwrap_or(0) as i64,
                has_totp: info.has_totp,
                api_key: info.api_key.clone(),
                client_id: info.user_id.clone(),
            })
            .collect();

        Ok(Response::new(BrokerList { brokers }))
    }

    async fn add_broker(
        &self,
        request: Request<BrokerConfig>,
    ) -> Result<Response<BrokerStatus>, Status> {
        let config = request.into_inner();
        tracing::info!("Adding broker: {} ({})", config.id, config.broker_type);

        // Only Zerodha is supported for now
        if config.broker_type.to_lowercase() != "zerodha" {
            return Err(Status::invalid_argument(format!(
                "Unsupported broker type: {}. Only 'zerodha' is supported.",
                config.broker_type
            )));
        }

        // Extract credentials for persistence
        let api_secret = config.credentials.get("api_secret")
            .ok_or_else(|| Status::invalid_argument("Missing api_secret in credentials"))?
            .clone();
        let user_id = config.credentials.get("user_id")
            .cloned()
            .unwrap_or_default();
        let password = config.credentials.get("password")
            .cloned()
            .unwrap_or_default();
        let totp_secret = config.credentials.get("totp_secret").cloned();

        // Create Zerodha config
        let zerodha_config = Self::create_zerodha_config(&config)?;

        // Register broker
        let info = self.registry.add_broker(
            config.id.clone(),
            config.name.clone(),
            config.broker_type.clone(),
            zerodha_config,
            api_secret,
            user_id,
            password,
            totp_secret,
        ).await
            .map_err(|e| Status::internal(e))?;

        Ok(Response::new(BrokerStatus {
            id: info.id,
            name: info.name,
            state: Self::to_proto_state(info.state).into(),
            error_message: info.error_message.unwrap_or_default(),
            last_connected_timestamp: info.last_connected.unwrap_or(0) as i64,
            has_totp: info.has_totp,
            api_key: info.api_key.clone(),
            client_id: info.user_id.clone(),
        }))
    }

    async fn remove_broker(
        &self,
        request: Request<BrokerId>,
    ) -> Result<Response<Empty>, Status> {
        let broker_id = request.into_inner();
        tracing::info!("Removing broker: {}", broker_id.id);

        if self.registry.remove_broker(&broker_id.id).await {
            Ok(Response::new(Empty {}))
        } else {
            Err(Status::not_found(format!("Broker not found: {}", broker_id.id)))
        }
    }

    async fn test_connection(
        &self,
        request: Request<BrokerId>,
    ) -> Result<Response<ConnectionResult>, Status> {
        let broker_id = request.into_inner();
        tracing::info!("Testing connection for broker: {}", broker_id.id);

        match self.registry.test_connection(&broker_id.id).await {
            Ok((user_id, user_name, exchanges)) => {
                Ok(Response::new(ConnectionResult {
                    success: true,
                    message: "Connected successfully".to_string(),
                    user_id,
                    user_name,
                    exchanges,
                }))
            }
            Err(e) => {
                Ok(Response::new(ConnectionResult {
                    success: false,
                    message: e,
                    user_id: String::new(),
                    user_name: String::new(),
                    exchanges: vec![],
                }))
            }
        }
    }

    type GetBrokerStatusStream = ReceiverStream<Result<BrokerStatus, Status>>;

    async fn get_broker_status(
        &self,
        request: Request<BrokerId>,
    ) -> Result<Response<Self::GetBrokerStatusStream>, Status> {
        let broker_id = request.into_inner();
        tracing::info!("Fetching status for broker: {}", broker_id.id);

        let (tx, rx) = tokio::sync::mpsc::channel(128);

        // Return current status as a single snapshot
        if let Some(info) = self.registry.get_broker_info(&broker_id.id).await {
            let status = BrokerStatus {
                id: info.id,
                name: info.name,
                state: Self::to_proto_state(info.state).into(),
                error_message: info.error_message.unwrap_or_default(),
                last_connected_timestamp: info.last_connected.unwrap_or(0) as i64,
                has_totp: info.has_totp,
                api_key: info.api_key.clone(),
                client_id: info.user_id.clone(),
            };

            let _ = tx.send(Ok(status)).await;
        } else {
            let _ = tx.send(Err(Status::not_found("Broker not found"))).await;
        }

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
