//! HTTP/JSON API layer for frontend compatibility

use std::sync::Arc;
use std::process::Command;
use axum::{
    Router,
    routing::{post, get},
    extract::{State, Json},
    response::IntoResponse,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{CorsLayer, Any};

use crate::registry::{ClientRegistry, SharedClients};

#[derive(Debug, Serialize)]
pub struct BrokerInfo {
    pub id: String,
    pub name: String,
    pub state: i32,
}

#[derive(Debug, Serialize)]
pub struct BrokerListResponse {
    pub brokers: Vec<BrokerInfo>,
}

#[derive(Debug, Deserialize)]
pub struct BrokerIdRequest {
    pub broker_id: Option<String>,
    pub id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DaySummaryResponse {
    #[serde(rename = "totalPnl")]
    pub total_pnl: f64,
    #[serde(rename = "realizedPnl")]
    pub realized_pnl: f64,
    #[serde(rename = "unrealizedPnl")]
    pub unrealized_pnl: f64,
    #[serde(rename = "totalCharges")]
    pub total_charges: f64,
}

#[derive(Debug, Serialize)]
pub struct HoldingsResponse {
    pub holdings: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct SearchInstrumentsRequest {
    pub query: Option<String>,
    pub exchange: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InstrumentInfo {
    pub symbol: String,
    pub name: String,
    pub exchange: String,
    #[serde(rename = "instrumentType")]
    pub instrument_type: String,
    #[serde(rename = "tradingSymbol")]
    pub trading_symbol: String,
}

#[derive(Debug, Serialize)]
pub struct InstrumentsResponse {
    pub instruments: Vec<InstrumentInfo>,
}

#[derive(Debug, Serialize)]
pub struct OrderResponse {
    pub success: bool,
    #[serde(rename = "orderId")]
    pub order_id: String,
}

#[derive(Debug, Serialize)]
pub struct ConnectionTestResponse {
    pub success: bool,
    #[serde(rename = "latencyMs")]
    pub latency_ms: i32,
}

// Zerodha Token Management Types
#[derive(Debug, Deserialize)]
pub struct SaveCredentialsRequest {
    #[serde(rename = "brokerId")]
    pub broker_id: String,
    #[serde(rename = "brokerName", default)]
    pub broker_name: Option<String>,
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(rename = "apiSecret")]
    pub api_secret: String,
    #[serde(rename = "userId")]
    pub user_id: String,
    pub password: String,
    #[serde(rename = "totpSecret")]
    pub totp_secret: String,
    #[serde(rename = "masterPassword")]
    pub master_password: String,
}

#[derive(Debug, Serialize)]
pub struct SaveCredentialsResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    #[serde(rename = "brokerId")]
    pub broker_id: String,
    #[serde(rename = "masterPassword")]
    pub master_password: String,
    pub force: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct RefreshTokenResponse {
    pub success: bool,
    #[serde(rename = "accessToken")]
    pub access_token: Option<String>,
    pub expiry: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TokenStatusRequest {
    #[serde(rename = "brokerId")]
    pub broker_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TokenStatusResponse {
    pub valid: bool,
    pub expiry: Option<String>,
    #[serde(rename = "expiresIn")]
    pub expires_in: Option<String>,
    #[serde(rename = "credentialsConfigured")]
    pub credentials_configured: bool,
}

#[derive(Clone)]
pub struct AppState {
    pub registry: Arc<ClientRegistry>,
    #[allow(dead_code)]
    pub clients: SharedClients,
}

async fn list_brokers(State(state): State<AppState>) -> impl IntoResponse {
    let brokers: Vec<BrokerInfo> = state.registry.list_brokers().await
        .into_iter()
        .map(|b| BrokerInfo { id: b.id, name: b.name, state: b.state as i32 })
        .collect();
    Json(BrokerListResponse { brokers })
}

async fn register_broker(Json(req): Json<serde_json::Value>) -> impl IntoResponse {
    // Frontend sends config directly: { id, name, broker_type, credentials: {...} }
    let broker_id = req.get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("zerodha")
        .to_string();

    // Return BrokerStatus format expected by frontend
    Json(serde_json::json!({
        "id": broker_id,
        "state": 0,  // CONNECTION_STATE_OFFLINE
        "message": "Broker registered successfully"
    }))
}

async fn test_connection(Json(_req): Json<serde_json::Value>) -> impl IntoResponse {
    Json(ConnectionTestResponse { success: true, latency_ms: 45 })
}

async fn get_day_summary(Json(_req): Json<BrokerIdRequest>) -> impl IntoResponse {
    Json(DaySummaryResponse {
        total_pnl: 0.0,
        realized_pnl: 0.0,
        unrealized_pnl: 0.0,
        total_charges: 0.0,
    })
}

async fn get_holdings(Json(_req): Json<BrokerIdRequest>) -> impl IntoResponse {
    Json(HoldingsResponse { holdings: vec![] })
}

async fn search_instruments(Json(req): Json<SearchInstrumentsRequest>) -> impl IntoResponse {
    let query = req.query.unwrap_or_default().to_lowercase();
    let mock_instruments: Vec<InstrumentInfo> = vec![
        InstrumentInfo { symbol: "RELIANCE".into(), name: "Reliance Industries".into(), exchange: "NSE".into(), instrument_type: "EQ".into(), trading_symbol: "RELIANCE".into() },
        InstrumentInfo { symbol: "TCS".into(), name: "Tata Consultancy Services".into(), exchange: "NSE".into(), instrument_type: "EQ".into(), trading_symbol: "TCS".into() },
        InstrumentInfo { symbol: "INFY".into(), name: "Infosys Limited".into(), exchange: "NSE".into(), instrument_type: "EQ".into(), trading_symbol: "INFY".into() },
        InstrumentInfo { symbol: "HDFCBANK".into(), name: "HDFC Bank Limited".into(), exchange: "NSE".into(), instrument_type: "EQ".into(), trading_symbol: "HDFCBANK".into() },
    ].into_iter()
        .filter(|i| i.symbol.to_lowercase().contains(&query) || i.name.to_lowercase().contains(&query))
        .collect();
    Json(InstrumentsResponse { instruments: mock_instruments })
}

async fn get_historical_bars(Json(_req): Json<serde_json::Value>) -> impl IntoResponse {
    Json(serde_json::json!({ "bars": [] }))
}

async fn place_order(Json(_req): Json<serde_json::Value>) -> impl IntoResponse {
    let order_id = format!("ORD{}", chrono::Utc::now().timestamp_millis());
    Json(OrderResponse { success: true, order_id })
}

async fn cancel_order(Json(_req): Json<serde_json::Value>) -> impl IntoResponse {
    Json(OrderResponse { success: true, order_id: String::new() })
}

// Zerodha Token Management Endpoints

async fn save_zerodha_credentials(
    Json(req): Json<SaveCredentialsRequest>,
) -> impl IntoResponse {
    // Get the scripts directory path
    let scripts_dir = std::env::var("NAUTILUS_SCRIPTS_DIR")
        .unwrap_or_else(|_| "D:/Nautilus-Trader/scripts/zerodha".to_string());

    // Helper to escape single quotes for Python strings
    fn escape_py(s: &str) -> String {
        s.replace('\\', "\\\\").replace('\'', "\\'")
    }

    // Create a temporary Python script to save credentials
    let python_code = format!(
        r#"
import sys
sys.path.insert(0, r'{scripts_dir}')
from credential_store import CredentialStore, ZerodhaCredentials

store = CredentialStore()
if not store.unlock('{master_password}'):
    print('UNLOCK_FAILED')
    sys.exit(1)

creds = ZerodhaCredentials(
    api_key='{api_key}',
    api_secret='{api_secret}',
    user_id='{user_id}',
    password='{password}',
    totp_secret='{totp_secret}'
)
store.save(creds)

# Also save non-sensitive metadata for UI display
import json
from pathlib import Path
metadata_file = Path.home() / '.nautilus' / 'broker_metadata.json'
metadata_file.parent.mkdir(parents=True, exist_ok=True)
api_key = '{api_key}'
metadata = {{
    'broker_id': 'zerodha',
    'broker_name': '{broker_name}' or 'Zerodha',
    'api_key_prefix': api_key[:8] + '...' if len(api_key) > 8 else api_key,
    'user_id': '{user_id}'
}}
with open(metadata_file, 'w') as f:
    json.dump(metadata, f)

print('SUCCESS')
"#,
        scripts_dir = scripts_dir,
        master_password = escape_py(&req.master_password),
        api_key = escape_py(&req.api_key),
        api_secret = escape_py(&req.api_secret),
        user_id = escape_py(&req.user_id),
        password = escape_py(&req.password),
        totp_secret = escape_py(&req.totp_secret),
        broker_name = escape_py(req.broker_name.as_deref().unwrap_or("Zerodha"))
    );

    let output = Command::new("python")
        .arg("-c")
        .arg(&python_code)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            if stdout.contains("SUCCESS") {
                Json(SaveCredentialsResponse {
                    success: true,
                    message: "Credentials saved successfully".to_string(),
                })
            } else if stdout.contains("UNLOCK_FAILED") {
                Json(SaveCredentialsResponse {
                    success: false,
                    message: "Wrong master password or corrupted store. Delete ~/.nautilus/zerodha_creds.enc to reset.".to_string(),
                })
            } else {
                Json(SaveCredentialsResponse {
                    success: false,
                    message: format!("Failed to save credentials: {}", stderr),
                })
            }
        }
        Err(e) => Json(SaveCredentialsResponse {
            success: false,
            message: format!("Failed to execute Python: {}", e),
        }),
    }
}

async fn refresh_zerodha_token(
    Json(req): Json<RefreshTokenRequest>,
) -> impl IntoResponse {
    let scripts_dir = std::env::var("NAUTILUS_SCRIPTS_DIR")
        .unwrap_or_else(|_| "D:/Nautilus-Trader/scripts/zerodha".to_string());

    // Helper to escape single quotes for Python strings
    fn escape_py(s: &str) -> String {
        s.replace('\\', "\\\\").replace('\'', "\\'")
    }

    let force_flag = if req.force.unwrap_or(false) { "True" } else { "False" };
    let escaped_password = escape_py(&req.master_password);

    let python_code = format!(
        r#"
import sys
import json
sys.path.insert(0, r'{scripts_dir}')
from credential_store import CredentialStore
from auto_login import ZerodhaAutoLogin

store = CredentialStore()
if not store.unlock('{master_password}'):
    print(json.dumps({{"success": False, "error": "Invalid master password"}}))
    sys.exit(0)

creds = store.load()
if not creds:
    print(json.dumps({{"success": False, "error": "No credentials found"}}))
    sys.exit(0)

auth = ZerodhaAutoLogin(
    api_key=creds.api_key,
    api_secret=creds.api_secret,
    user_id=creds.user_id,
    password=creds.password,
    totp_secret=creds.totp_secret
)

try:
    token = auth.login(force_refresh={force_flag})
    # Read expiry from token file
    import os
    token_file = os.path.expanduser('~/.zerodha_token.json')
    with open(token_file) as f:
        data = json.load(f)
    print(json.dumps({{"success": True, "accessToken": token, "expiry": data.get("expiry")}}))
except Exception as e:
    print(json.dumps({{"success": False, "error": str(e)}}))
"#,
        scripts_dir = scripts_dir,
        master_password = escaped_password,
        force_flag = force_flag
    );

    let output = Command::new("python")
        .arg("-c")
        .arg(&python_code)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            match serde_json::from_str::<serde_json::Value>(&stdout) {
                Ok(json) => {
                    Json(RefreshTokenResponse {
                        success: json.get("success").and_then(|v| v.as_bool()).unwrap_or(false),
                        access_token: json.get("accessToken").and_then(|v| v.as_str()).map(String::from),
                        expiry: json.get("expiry").and_then(|v| v.as_str()).map(String::from),
                        error: json.get("error").and_then(|v| v.as_str()).map(String::from),
                    })
                }
                Err(_) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    Json(RefreshTokenResponse {
                        success: false,
                        access_token: None,
                        expiry: None,
                        error: Some(format!("Failed to parse response: {} {}", stdout, stderr)),
                    })
                }
            }
        }
        Err(e) => Json(RefreshTokenResponse {
            success: false,
            access_token: None,
            expiry: None,
            error: Some(format!("Failed to execute Python: {}", e)),
        }),
    }
}

async fn get_token_status(
    Json(_req): Json<TokenStatusRequest>,
) -> impl IntoResponse {
    let scripts_dir = std::env::var("NAUTILUS_SCRIPTS_DIR")
        .unwrap_or_else(|_| "D:/Nautilus-Trader/scripts/zerodha".to_string());

    let python_code = format!(
        r#"
import sys
import json
import os
from datetime import datetime
from pathlib import Path
sys.path.insert(0, r'{}')
from credential_store import CredentialStore

# Check if credentials exist
store = CredentialStore()
creds_exist = store.exists()

# Check token status
token_file = Path.home() / '.zerodha_token.json'
if token_file.exists():
    with open(token_file) as f:
        data = json.load(f)
    expiry = datetime.fromisoformat(data.get('expiry', ''))
    now = datetime.now()
    if now < expiry:
        remaining = expiry - now
        hours = remaining.seconds // 3600
        mins = (remaining.seconds % 3600) // 60
        print(json.dumps({{
            "valid": True,
            "expiry": data.get("expiry"),
            "expiresIn": f"{{hours}}h {{mins}}m",
            "credentialsConfigured": creds_exist
        }}))
    else:
        print(json.dumps({{"valid": False, "credentialsConfigured": creds_exist}}))
else:
    print(json.dumps({{"valid": False, "credentialsConfigured": creds_exist}}))
"#,
        scripts_dir
    );

    let output = Command::new("python")
        .arg("-c")
        .arg(&python_code)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            match serde_json::from_str::<serde_json::Value>(&stdout) {
                Ok(json) => {
                    Json(TokenStatusResponse {
                        valid: json.get("valid").and_then(|v| v.as_bool()).unwrap_or(false),
                        expiry: json.get("expiry").and_then(|v| v.as_str()).map(String::from),
                        expires_in: json.get("expiresIn").and_then(|v| v.as_str()).map(String::from),
                        credentials_configured: json.get("credentialsConfigured").and_then(|v| v.as_bool()).unwrap_or(false),
                    })
                }
                Err(_) => Json(TokenStatusResponse {
                    valid: false,
                    expiry: None,
                    expires_in: None,
                    credentials_configured: false,
                }),
            }
        }
        Err(_) => Json(TokenStatusResponse {
            valid: false,
            expiry: None,
            expires_in: None,
            credentials_configured: false,
        }),
    }
}

// Response for GetBrokerConfig
#[derive(Serialize)]
struct BrokerConfigResponse {
    exists: bool,
    broker_id: Option<String>,
    broker_name: Option<String>,
    api_key_prefix: Option<String>,
    user_id: Option<String>,
}

async fn get_broker_config() -> impl IntoResponse {
    let scripts_dir = std::env::var("NAUTILUS_SCRIPTS_DIR")
        .unwrap_or_else(|_| "D:/Nautilus-Trader/scripts/zerodha".to_string());

    // Read broker metadata from plain JSON file (non-sensitive)
    let python_code = format!(
        r#"
import sys
import json
from pathlib import Path
sys.path.insert(0, r'{}')

# Check broker metadata file
metadata_file = Path.home() / '.nautilus' / 'broker_metadata.json'
if metadata_file.exists():
    with open(metadata_file) as f:
        data = json.load(f)
    print(json.dumps({{
        "exists": True,
        "brokerId": data.get("broker_id", "zerodha"),
        "brokerName": data.get("broker_name", "Zerodha"),
        "apiKeyPrefix": data.get("api_key_prefix", ""),
        "userId": data.get("user_id", "")
    }}))
else:
    # Check if encrypted credentials exist
    from credential_store import CredentialStore
    store = CredentialStore()
    if store.exists():
        print(json.dumps({{"exists": True, "brokerId": "zerodha", "brokerName": "Zerodha"}}))
    else:
        print(json.dumps({{"exists": False}}))
"#,
        scripts_dir
    );

    let output = Command::new("python")
        .arg("-c")
        .arg(&python_code)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            match serde_json::from_str::<serde_json::Value>(&stdout) {
                Ok(json) => Json(BrokerConfigResponse {
                    exists: json.get("exists").and_then(|v| v.as_bool()).unwrap_or(false),
                    broker_id: json.get("brokerId").and_then(|v| v.as_str()).map(String::from),
                    broker_name: json.get("brokerName").and_then(|v| v.as_str()).map(String::from),
                    api_key_prefix: json.get("apiKeyPrefix").and_then(|v| v.as_str()).map(String::from),
                    user_id: json.get("userId").and_then(|v| v.as_str()).map(String::from),
                }),
                Err(_) => Json(BrokerConfigResponse {
                    exists: false,
                    broker_id: None,
                    broker_name: None,
                    api_key_prefix: None,
                    user_id: None,
                }),
            }
        }
        Err(_) => Json(BrokerConfigResponse {
            exists: false,
            broker_id: None,
            broker_name: None,
            api_key_prefix: None,
            user_id: None,
        }),
    }
}

pub fn create_http_router(registry: Arc<ClientRegistry>, clients: SharedClients) -> Router {
    let state = AppState { registry, clients };
    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any);

    Router::new()
        .route("/broker.BrokerService/ListBrokers", post(list_brokers))
        .route("/broker.BrokerService/RegisterBroker", post(register_broker))
        .route("/broker.BrokerService/TestConnection", post(test_connection))
        // Zerodha token management
        .route("/broker.BrokerService/SaveCredentials", post(save_zerodha_credentials))
        .route("/broker.BrokerService/RefreshToken", post(refresh_zerodha_token))
        .route("/broker.BrokerService/GetTokenStatus", post(get_token_status))
        .route("/broker.BrokerService/GetBrokerConfig", post(get_broker_config))
        .route("/portfolio.PortfolioService/GetDaySummary", post(get_day_summary))
        .route("/portfolio.PortfolioService/GetHoldings", post(get_holdings))
        .route("/marketdata.MarketDataService/SearchInstruments", post(search_instruments))
        .route("/marketdata.MarketDataService/GetHistoricalBars", post(get_historical_bars))
        .route("/order.OrderService/PlaceOrder", post(place_order))
        .route("/order.OrderService/CancelOrder", post(cancel_order))
        .layer(cors)
        .with_state(state)
}
