//! Shared client registry for broker connections
//!
//! Provides centralized management of broker connections that can be shared
//! across all gRPC services.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};

use nautilus_zerodha::{ZerodhaConfig, ZerodhaHttpClient};

/// Event emitted when broker state changes
#[derive(Debug, Clone)]
pub enum BrokerEvent {
    /// Broker client was added and connected
    Connected {
        broker_id: String,
        client: Arc<ZerodhaHttpClient>,
    },
    /// Broker was disconnected/removed
    Disconnected {
        broker_id: String,
    },
}

/// Connection state for a broker
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Offline,
    Connecting,
    Online,
    Error,
}

/// Information about a registered broker
#[derive(Debug, Clone)]
pub struct BrokerInfo {
    pub id: String,
    pub name: String,
    pub broker_type: String,
    pub state: ConnectionState,
    pub last_connected: Option<u64>,
    pub error_message: Option<String>,
}

/// Internal broker connection data
struct BrokerConnection {
    info: BrokerInfo,
    config: ZerodhaConfig,
    client: Arc<ZerodhaHttpClient>,
}

/// Shared registry for managing broker connections
///
/// This registry is designed to be shared across all gRPC services
/// using `Arc<ClientRegistry>`. Services can subscribe to events
/// to be notified when brokers connect/disconnect.
pub struct ClientRegistry {
    /// Broker connections indexed by broker_id
    brokers: RwLock<HashMap<String, BrokerConnection>>,
    /// Event broadcast channel
    event_tx: broadcast::Sender<BrokerEvent>,
}

impl std::fmt::Debug for ClientRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientRegistry").finish()
    }
}

impl Default for ClientRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientRegistry {
    /// Create a new client registry
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(64);
        Self {
            brokers: RwLock::new(HashMap::new()),
            event_tx,
        }
    }

    /// Subscribe to broker events
    pub fn subscribe(&self) -> broadcast::Receiver<BrokerEvent> {
        self.event_tx.subscribe()
    }

    /// Add a new broker connection
    ///
    /// Creates a ZerodhaHttpClient from the provided configuration and
    /// registers it with the given broker_id.
    pub async fn add_broker(
        &self,
        broker_id: String,
        name: String,
        broker_type: String,
        config: ZerodhaConfig,
    ) -> Result<BrokerInfo, String> {
        let client = ZerodhaHttpClient::new(config.clone())
            .map_err(|e| format!("Failed to create client: {}", e))?;

        let info = BrokerInfo {
            id: broker_id.clone(),
            name,
            broker_type,
            state: ConnectionState::Offline,
            last_connected: None,
            error_message: None,
        };

        let mut brokers = self.brokers.write().await;
        brokers.insert(broker_id, BrokerConnection {
            info: info.clone(),
            config,
            client: Arc::new(client),
        });

        Ok(info)
    }

    /// Remove a broker connection
    pub async fn remove_broker(&self, broker_id: &str) -> bool {
        let mut brokers = self.brokers.write().await;
        if brokers.remove(broker_id).is_some() {
            // Notify subscribers
            let _ = self.event_tx.send(BrokerEvent::Disconnected {
                broker_id: broker_id.to_string(),
            });
            true
        } else {
            false
        }
    }

    /// Test connection for a broker and update its state
    ///
    /// Returns the user profile info if successful.
    pub async fn test_connection(
        &self,
        broker_id: &str,
    ) -> Result<(String, String, Vec<String>), String> {
        // First get the client and test connection
        let client = {
            let mut brokers = self.brokers.write().await;
            let conn = brokers.get_mut(broker_id)
                .ok_or_else(|| format!("Broker not found: {}", broker_id))?;
            conn.info.state = ConnectionState::Connecting;
            Arc::clone(&conn.client)
        };

        match client.get_user_profile().await {
            Ok(profile) => {
                let mut brokers = self.brokers.write().await;
                if let Some(conn) = brokers.get_mut(broker_id) {
                    conn.info.state = ConnectionState::Online;
                    conn.info.last_connected = Some(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs()
                    );
                    conn.info.error_message = None;

                    // Notify subscribers of successful connection
                    let _ = self.event_tx.send(BrokerEvent::Connected {
                        broker_id: broker_id.to_string(),
                        client: Arc::clone(&conn.client),
                    });
                }

                Ok((profile.user_id, profile.user_name, profile.exchanges))
            }
            Err(e) => {
                let mut brokers = self.brokers.write().await;
                if let Some(conn) = brokers.get_mut(broker_id) {
                    conn.info.state = ConnectionState::Error;
                    conn.info.error_message = Some(e.to_string());
                }
                Err(format!("Connection failed: {}", e))
            }
        }
    }

    /// Get a shared client for a broker
    ///
    /// Returns None if broker doesn't exist or isn't connected.
    pub async fn get_client(&self, broker_id: &str) -> Option<Arc<ZerodhaHttpClient>> {
        let brokers = self.brokers.read().await;
        let conn = brokers.get(broker_id)?;

        if conn.info.state != ConnectionState::Online {
            return None;
        }

        Some(Arc::clone(&conn.client))
    }

    /// Get all connected clients
    pub async fn get_all_clients(&self) -> HashMap<String, Arc<ZerodhaHttpClient>> {
        let brokers = self.brokers.read().await;
        let mut clients = HashMap::new();

        for (id, conn) in brokers.iter() {
            if conn.info.state == ConnectionState::Online {
                clients.insert(id.clone(), Arc::clone(&conn.client));
            }
        }

        clients
    }

    /// Get broker info
    pub async fn get_broker_info(&self, broker_id: &str) -> Option<BrokerInfo> {
        let brokers = self.brokers.read().await;
        brokers.get(broker_id).map(|conn| conn.info.clone())
    }

    /// List all registered brokers
    pub async fn list_brokers(&self) -> Vec<BrokerInfo> {
        let brokers = self.brokers.read().await;
        brokers.values().map(|conn| conn.info.clone()).collect()
    }
}

/// Shared client store that services use to access broker clients
///
/// This is a simpler interface for services that just need to access
/// clients without managing the full broker lifecycle.
#[derive(Clone)]
pub struct SharedClients {
    clients: Arc<RwLock<HashMap<String, Arc<ZerodhaHttpClient>>>>,
}

impl std::fmt::Debug for SharedClients {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedClients").finish()
    }
}

impl Default for SharedClients {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedClients {
    /// Create a new shared clients store
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a client
    pub async fn register(&self, broker_id: String, client: Arc<ZerodhaHttpClient>) {
        let mut clients = self.clients.write().await;
        clients.insert(broker_id, client);
    }

    /// Unregister a client
    pub async fn unregister(&self, broker_id: &str) {
        let mut clients = self.clients.write().await;
        clients.remove(broker_id);
    }

    /// Get a client by broker_id
    pub async fn get(&self, broker_id: &str) -> Option<Arc<ZerodhaHttpClient>> {
        let clients = self.clients.read().await;
        clients.get(broker_id).cloned()
    }

    /// Get the first available client
    pub async fn get_any(&self) -> Option<Arc<ZerodhaHttpClient>> {
        let clients = self.clients.read().await;
        clients.values().next().cloned()
    }

    /// Get all clients
    pub async fn get_all(&self) -> HashMap<String, Arc<ZerodhaHttpClient>> {
        let clients = self.clients.read().await;
        clients.clone()
    }

    /// Get internal Arc for sharing with services
    pub fn inner(&self) -> Arc<RwLock<HashMap<String, Arc<ZerodhaHttpClient>>>> {
        Arc::clone(&self.clients)
    }
}

/// Start a background task that syncs ClientRegistry events to SharedClients
pub fn start_client_sync(
    registry: Arc<ClientRegistry>,
    shared: SharedClients,
) -> tokio::task::JoinHandle<()> {
    let mut events = registry.subscribe();

    tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(BrokerEvent::Connected { broker_id, client }) => {
                    tracing::info!("Client connected: {}", broker_id);
                    shared.register(broker_id, client).await;
                }
                Ok(BrokerEvent::Disconnected { broker_id }) => {
                    tracing::info!("Client disconnected: {}", broker_id);
                    shared.unregister(&broker_id).await;
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("Client sync lagged by {} events", n);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    tracing::info!("Client registry closed, stopping sync");
                    break;
                }
            }
        }
    })
}
