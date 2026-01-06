//! NautilusTrader gRPC Gateway
//!
//! This crate provides a gRPC server that exposes NautilusTrader functionality
//! to the Trading Terminal UI.

pub mod proto;
pub mod registry;
pub mod services;
pub mod security;
pub mod http_api;
pub mod backtest_service;
pub mod broker_adapter;
pub mod brokers;
pub mod db;

pub use registry::{ClientRegistry, SharedClients, BrokerEvent, BrokerInfo, ConnectionState, start_client_sync};

use thiserror::Error;

/// Gateway error types
#[derive(Error, Debug)]
pub enum GatewayError {
    #[error("Authentication failed: {0}")]
    AuthError(String),

    #[error("Broker error: {0}")]
    BrokerError(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<GatewayError> for tonic::Status {
    fn from(err: GatewayError) -> Self {
        match err {
            GatewayError::AuthError(msg) => tonic::Status::unauthenticated(msg),
            GatewayError::BrokerError(msg) => tonic::Status::unavailable(msg),
            GatewayError::InvalidRequest(msg) => tonic::Status::invalid_argument(msg),
            GatewayError::NotFound(msg) => tonic::Status::not_found(msg),
            GatewayError::Internal(msg) => tonic::Status::internal(msg),
        }
    }
}

/// Gateway configuration
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    /// Server bind address
    pub bind_addr: String,
    /// Server port
    pub port: u16,
    /// Enable TLS
    pub tls_enabled: bool,
    /// Path to TLS certificate
    pub tls_cert_path: Option<String>,
    /// Path to TLS key
    pub tls_key_path: Option<String>,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1".to_string(),
            port: 8080,
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
        }
    }
}
