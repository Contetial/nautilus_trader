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
use crate::proto::ai::ai_service_server::AiService;
use crate::proto::backtest::backtest_service_server::BacktestService;
use crate::services::AiServiceImpl;
use crate::backtest_service::BacktestServiceImpl;
use axum::response::sse::{Event, Sse};
use futures::stream::Stream;

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

#[derive(Debug, Deserialize)]
pub struct GetQuotesRequest {
    pub instruments: Vec<String>,  // Format: "EXCHANGE:SYMBOL"
}

#[derive(Debug, Serialize)]
pub struct QuoteData {
    pub symbol: String,
    pub exchange: String,
    pub ltp: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
    pub bid: f64,
    pub ask: f64,
    #[serde(rename = "bidQty")]
    pub bid_qty: i64,
    #[serde(rename = "askQty")]
    pub ask_qty: i64,
    pub change: f64,
    #[serde(rename = "changePercent")]
    pub change_percent: f64,
}

#[derive(Debug, Serialize)]
pub struct QuotesResponse {
    pub success: bool,
    pub quotes: Vec<QuoteData>,
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

// Order Management Types
#[derive(Debug, Deserialize)]
pub struct PlaceOrderRequest {
    #[serde(rename = "brokerId")]
    pub broker_id: String,
    pub symbol: String,
    pub exchange: String,
    pub side: String,  // "BUY" or "SELL"
    #[serde(rename = "orderType")]
    pub order_type: String,  // "MARKET", "LIMIT", "SL", "SL-M"
    pub product: String,  // "MIS", "CNC", "NRML"
    pub quantity: i32,
    pub price: Option<f64>,
    #[serde(rename = "triggerPrice")]
    pub trigger_price: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct ModifyOrderRequest {
    #[serde(rename = "brokerId")]
    pub broker_id: String,
    #[serde(rename = "orderId")]
    pub order_id: String,
    pub quantity: Option<i32>,
    pub price: Option<f64>,
    #[serde(rename = "triggerPrice")]
    pub trigger_price: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct CancelOrderRequest {
    #[serde(rename = "brokerId")]
    pub broker_id: String,
    #[serde(rename = "orderId")]
    pub order_id: String,
}

#[derive(Clone)]
pub struct AppState {
    pub registry: Arc<ClientRegistry>,
    #[allow(dead_code)]
    pub clients: SharedClients,
    pub ai_service: Arc<AiServiceImpl>,
    pub backtest_service: Arc<BacktestServiceImpl>,
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
    let scripts_dir = std::env::var("NAUTILUS_SCRIPTS_DIR")
        .unwrap_or_else(|_| "D:/Nautilus-Trader/scripts/zerodha".to_string());

    let python_code = format!(
        r#"
import sys
import json
from pathlib import Path
from datetime import datetime
sys.path.insert(0, r'{scripts_dir}')
from kite_api import KiteAPI

# Load API key and access token
metadata_file = Path.home() / '.nautilus' / 'broker_metadata.json'
token_file = Path.home() / '.zerodha_token.json'

import os
if not metadata_file.exists() or not token_file.exists():
    print(json.dumps({{"success": False, "error": "Credentials not configured"}}))
    sys.exit(0)

with open(metadata_file) as f:
    metadata = json.load(f)
api_key = metadata.get('api_key') or os.environ.get('KITE_API_KEY')
if not api_key:
    print(json.dumps({{"success": False, "error": "API key not configured. Re-register broker or set KITE_API_KEY."}}))
    sys.exit(0)

with open(token_file) as f:
    token_data = json.load(f)
access_token = token_data.get('access_token')
expiry = datetime.fromisoformat(token_data.get('expiry', ''))
if datetime.now() >= expiry:
    print(json.dumps({{"success": False, "error": "Token expired"}}))
    sys.exit(0)

try:
    api = KiteAPI(api_key, access_token)
    positions = api.get_positions()

    # Calculate P&L from positions
    total_pnl = 0.0
    realized_pnl = 0.0
    unrealized_pnl = 0.0

    for pos in positions.get('day', []):
        pnl = pos.get('pnl', 0)
        total_pnl += pnl
        if pos.get('quantity', 0) == 0:
            realized_pnl += pnl
        else:
            unrealized_pnl += pnl

    print(json.dumps({{
        "success": True,
        "totalPnl": total_pnl,
        "realizedPnl": realized_pnl,
        "unrealizedPnl": unrealized_pnl,
        "totalCharges": 0.0
    }}))
except Exception as e:
    print(json.dumps({{"success": False, "error": str(e)}}))
"#,
        scripts_dir = scripts_dir
    );

    let output = Command::new("python")
        .arg("-c")
        .arg(&python_code)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            match serde_json::from_str::<serde_json::Value>(&stdout) {
                Ok(json) if json.get("success").and_then(|v| v.as_bool()).unwrap_or(false) => {
                    Json(DaySummaryResponse {
                        total_pnl: json.get("totalPnl").and_then(|v| v.as_f64()).unwrap_or(0.0),
                        realized_pnl: json.get("realizedPnl").and_then(|v| v.as_f64()).unwrap_or(0.0),
                        unrealized_pnl: json.get("unrealizedPnl").and_then(|v| v.as_f64()).unwrap_or(0.0),
                        total_charges: json.get("totalCharges").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    })
                }
                _ => Json(DaySummaryResponse {
                    total_pnl: 0.0,
                    realized_pnl: 0.0,
                    unrealized_pnl: 0.0,
                    total_charges: 0.0,
                }),
            }
        }
        Err(_) => Json(DaySummaryResponse {
            total_pnl: 0.0,
            realized_pnl: 0.0,
            unrealized_pnl: 0.0,
            total_charges: 0.0,
        }),
    }
}

async fn get_holdings(Json(_req): Json<BrokerIdRequest>) -> impl IntoResponse {
    let scripts_dir = std::env::var("NAUTILUS_SCRIPTS_DIR")
        .unwrap_or_else(|_| "D:/Nautilus-Trader/scripts/zerodha".to_string());

    let python_code = format!(
        r#"
import sys
import json
from pathlib import Path
from datetime import datetime
sys.path.insert(0, r'{scripts_dir}')
from kite_api import KiteAPI

metadata_file = Path.home() / '.nautilus' / 'broker_metadata.json'
token_file = Path.home() / '.zerodha_token.json'

import os
if not metadata_file.exists() or not token_file.exists():
    print(json.dumps({{"success": False, "holdings": []}}))
    sys.exit(0)

with open(metadata_file) as f:
    metadata = json.load(f)
api_key = metadata.get('api_key') or os.environ.get('KITE_API_KEY')
if not api_key:
    print(json.dumps({{"success": False, "holdings": [], "error": "API key not configured"}}))
    sys.exit(0)

with open(token_file) as f:
    token_data = json.load(f)
access_token = token_data.get('access_token')
expiry = datetime.fromisoformat(token_data.get('expiry', ''))
if datetime.now() >= expiry:
    print(json.dumps({{"success": False, "holdings": []}}))
    sys.exit(0)

try:
    api = KiteAPI(api_key, access_token)
    holdings = api.get_holdings()
    print(json.dumps({{"success": True, "holdings": holdings}}))
except Exception as e:
    print(json.dumps({{"success": False, "holdings": [], "error": str(e)}}))
"#,
        scripts_dir = scripts_dir
    );

    let output = Command::new("python")
        .arg("-c")
        .arg(&python_code)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            match serde_json::from_str::<serde_json::Value>(&stdout) {
                Ok(json) => Json(HoldingsResponse {
                    holdings: json.get("holdings")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.clone())
                        .unwrap_or_default(),
                }),
                Err(_) => Json(HoldingsResponse { holdings: vec![] }),
            }
        }
        Err(_) => Json(HoldingsResponse { holdings: vec![] }),
    }
}

#[derive(Debug, Serialize)]
pub struct MarginsResponse {
    pub success: bool,
    pub equity: Option<serde_json::Value>,
    pub commodity: Option<serde_json::Value>,
}

async fn get_margins(Json(_req): Json<BrokerIdRequest>) -> impl IntoResponse {
    let scripts_dir = std::env::var("NAUTILUS_SCRIPTS_DIR")
        .unwrap_or_else(|_| "D:/Nautilus-Trader/scripts/zerodha".to_string());

    let python_code = format!(
        r#"
import sys
import json
from pathlib import Path
from datetime import datetime
sys.path.insert(0, r'{scripts_dir}')
from kite_api import KiteAPI

metadata_file = Path.home() / '.nautilus' / 'broker_metadata.json'
token_file = Path.home() / '.zerodha_token.json'

import os
if not metadata_file.exists() or not token_file.exists():
    print(json.dumps({{"success": False}}))
    sys.exit(0)

with open(metadata_file) as f:
    metadata = json.load(f)
api_key = metadata.get('api_key') or os.environ.get('KITE_API_KEY')
if not api_key:
    print(json.dumps({{"success": False, "error": "API key not configured"}}))
    sys.exit(0)

with open(token_file) as f:
    token_data = json.load(f)
access_token = token_data.get('access_token')
expiry = datetime.fromisoformat(token_data.get('expiry', ''))
if datetime.now() >= expiry:
    print(json.dumps({{"success": False}}))
    sys.exit(0)

try:
    api = KiteAPI(api_key, access_token)
    margins = api.get_margins()
    print(json.dumps({{"success": True, "equity": margins.get('equity'), "commodity": margins.get('commodity')}}))
except Exception as e:
    print(json.dumps({{"success": False, "error": str(e)}}))
"#,
        scripts_dir = scripts_dir
    );

    let output = Command::new("python")
        .arg("-c")
        .arg(&python_code)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            match serde_json::from_str::<serde_json::Value>(&stdout) {
                Ok(json) => Json(MarginsResponse {
                    success: json.get("success").and_then(|v| v.as_bool()).unwrap_or(false),
                    equity: json.get("equity").cloned(),
                    commodity: json.get("commodity").cloned(),
                }),
                Err(_) => Json(MarginsResponse { success: false, equity: None, commodity: None }),
            }
        }
        Err(_) => Json(MarginsResponse { success: false, equity: None, commodity: None }),
    }
}

#[derive(Debug, Serialize)]
pub struct PositionsResponse {
    pub success: bool,
    pub day: Vec<serde_json::Value>,
    pub net: Vec<serde_json::Value>,
}

async fn get_positions(Json(_req): Json<BrokerIdRequest>) -> impl IntoResponse {
    let scripts_dir = std::env::var("NAUTILUS_SCRIPTS_DIR")
        .unwrap_or_else(|_| "D:/Nautilus-Trader/scripts/zerodha".to_string());

    let python_code = format!(
        r#"
import sys
import json
from pathlib import Path
from datetime import datetime
sys.path.insert(0, r'{scripts_dir}')
from kite_api import KiteAPI

metadata_file = Path.home() / '.nautilus' / 'broker_metadata.json'
token_file = Path.home() / '.zerodha_token.json'

import os
if not metadata_file.exists() or not token_file.exists():
    print(json.dumps({{"success": False, "day": [], "net": []}}))
    sys.exit(0)

with open(metadata_file) as f:
    metadata = json.load(f)
api_key = metadata.get('api_key') or os.environ.get('KITE_API_KEY')
if not api_key:
    print(json.dumps({{"success": False, "day": [], "net": [], "error": "API key not configured"}}))
    sys.exit(0)

with open(token_file) as f:
    token_data = json.load(f)
access_token = token_data.get('access_token')
expiry = datetime.fromisoformat(token_data.get('expiry', ''))
if datetime.now() >= expiry:
    print(json.dumps({{"success": False, "day": [], "net": []}}))
    sys.exit(0)

try:
    api = KiteAPI(api_key, access_token)
    positions = api.get_positions()
    print(json.dumps({{
        "success": True,
        "day": positions.get('day', []),
        "net": positions.get('net', [])
    }}))
except Exception as e:
    print(json.dumps({{"success": False, "day": [], "net": [], "error": str(e)}}))
"#,
        scripts_dir = scripts_dir
    );

    let output = Command::new("python")
        .arg("-c")
        .arg(&python_code)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            match serde_json::from_str::<serde_json::Value>(&stdout) {
                Ok(json) => Json(PositionsResponse {
                    success: json.get("success").and_then(|v| v.as_bool()).unwrap_or(false),
                    day: json.get("day")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.clone())
                        .unwrap_or_default(),
                    net: json.get("net")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.clone())
                        .unwrap_or_default(),
                }),
                Err(_) => Json(PositionsResponse { success: false, day: vec![], net: vec![] }),
            }
        }
        Err(_) => Json(PositionsResponse { success: false, day: vec![], net: vec![] }),
    }
}

async fn search_instruments(Json(req): Json<SearchInstrumentsRequest>) -> impl IntoResponse {
    let scripts_dir = std::env::var("NAUTILUS_SCRIPTS_DIR")
        .unwrap_or_else(|_| "D:/Nautilus-Trader/scripts/zerodha".to_string());

    let query = req.query.unwrap_or_default();
    let exchange = req.exchange.unwrap_or_default();

    // Use real Zerodha instruments search
    let python_code = format!(
        r#"
import sys
sys.path.insert(0, r'{scripts_dir}')
from instruments import search_instruments_json
print(search_instruments_json('{query}', '{exchange}' if '{exchange}' else None, 50))
"#,
        scripts_dir = scripts_dir,
        query = query.replace('\'', "\\'"),
        exchange = exchange.replace('\'', "\\'")
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
                    let instruments: Vec<InstrumentInfo> = json.get("instruments")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|item| {
                                    Some(InstrumentInfo {
                                        symbol: item.get("symbol")?.as_str()?.to_string(),
                                        name: item.get("name")?.as_str()?.to_string(),
                                        exchange: item.get("exchange")?.as_str()?.to_string(),
                                        instrument_type: item.get("instrumentType")?.as_str()?.to_string(),
                                        trading_symbol: item.get("tradingSymbol")?.as_str()?.to_string(),
                                    })
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    Json(InstrumentsResponse { instruments })
                }
                Err(_) => Json(InstrumentsResponse { instruments: vec![] }),
            }
        }
        Err(_) => Json(InstrumentsResponse { instruments: vec![] }),
    }
}

async fn get_historical_bars(Json(_req): Json<serde_json::Value>) -> impl IntoResponse {
    Json(serde_json::json!({ "bars": [] }))
}

async fn get_quotes(Json(req): Json<GetQuotesRequest>) -> impl IntoResponse {
    let scripts_dir = std::env::var("NAUTILUS_SCRIPTS_DIR")
        .unwrap_or_else(|_| "D:/Nautilus-Trader/scripts/zerodha".to_string());

    if req.instruments.is_empty() {
        return Json(QuotesResponse { success: true, quotes: vec![] });
    }

    // Build Python list of instruments
    let instruments_str = req.instruments
        .iter()
        .map(|s| format!("'{}'", s.replace('\'', "\\'")))
        .collect::<Vec<_>>()
        .join(", ");

    let python_code = format!(
        r#"
import sys
import json
from pathlib import Path
from datetime import datetime
sys.path.insert(0, r'{scripts_dir}')
from kite_api import KiteAPI

metadata_file = Path.home() / '.nautilus' / 'broker_metadata.json'
token_file = Path.home() / '.zerodha_token.json'

if not metadata_file.exists() or not token_file.exists():
    print(json.dumps({{"success": False, "quotes": []}}))
    sys.exit(0)

import os
with open(metadata_file) as f:
    metadata = json.load(f)
api_key = metadata.get('api_key') or os.environ.get('KITE_API_KEY')
if not api_key:
    print(json.dumps({{"success": False, "quotes": [], "error": "API key not configured. Re-register broker or set KITE_API_KEY env var."}}))
    sys.exit(0)

with open(token_file) as f:
    token_data = json.load(f)
access_token = token_data.get('access_token')
expiry = datetime.fromisoformat(token_data.get('expiry', ''))
if datetime.now() >= expiry:
    print(json.dumps({{"success": False, "quotes": [], "error": "Token expired"}}))
    sys.exit(0)

try:
    api = KiteAPI(api_key, access_token)
    instruments = [{instruments_str}]
    raw_quotes = api.get_quote(instruments)

    quotes = []
    for key, q in raw_quotes.items():
        parts = key.split(':')
        exchange = parts[0] if len(parts) > 1 else ''
        symbol = parts[1] if len(parts) > 1 else key

        ohlc = q.get('ohlc', {{}})
        depth = q.get('depth', {{}})
        buy_depth = depth.get('buy', [{{}}])
        sell_depth = depth.get('sell', [{{}}])

        quotes.append({{
            "symbol": symbol,
            "exchange": exchange,
            "ltp": q.get('last_price', 0),
            "open": ohlc.get('open', 0),
            "high": ohlc.get('high', 0),
            "low": ohlc.get('low', 0),
            "close": ohlc.get('close', 0),
            "volume": q.get('volume', 0),
            "bid": buy_depth[0].get('price', 0) if buy_depth else 0,
            "ask": sell_depth[0].get('price', 0) if sell_depth else 0,
            "bidQty": buy_depth[0].get('quantity', 0) if buy_depth else 0,
            "askQty": sell_depth[0].get('quantity', 0) if sell_depth else 0,
            "change": q.get('change', 0),
            "changePercent": q.get('change', 0) / ohlc.get('close', 1) * 100 if ohlc.get('close') else 0
        }})

    print(json.dumps({{"success": True, "quotes": quotes}}))
except Exception as e:
    print(json.dumps({{"success": False, "quotes": [], "error": str(e)}}))
"#,
        scripts_dir = scripts_dir,
        instruments_str = instruments_str
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
                    let success = json.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
                    let quotes: Vec<QuoteData> = json.get("quotes")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|item| {
                                    Some(QuoteData {
                                        symbol: item.get("symbol")?.as_str()?.to_string(),
                                        exchange: item.get("exchange")?.as_str()?.to_string(),
                                        ltp: item.get("ltp")?.as_f64()?,
                                        open: item.get("open")?.as_f64().unwrap_or(0.0),
                                        high: item.get("high")?.as_f64().unwrap_or(0.0),
                                        low: item.get("low")?.as_f64().unwrap_or(0.0),
                                        close: item.get("close")?.as_f64().unwrap_or(0.0),
                                        volume: item.get("volume")?.as_i64().unwrap_or(0),
                                        bid: item.get("bid")?.as_f64().unwrap_or(0.0),
                                        ask: item.get("ask")?.as_f64().unwrap_or(0.0),
                                        bid_qty: item.get("bidQty")?.as_i64().unwrap_or(0),
                                        ask_qty: item.get("askQty")?.as_i64().unwrap_or(0),
                                        change: item.get("change")?.as_f64().unwrap_or(0.0),
                                        change_percent: item.get("changePercent")?.as_f64().unwrap_or(0.0),
                                    })
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    Json(QuotesResponse { success, quotes })
                }
                Err(_) => Json(QuotesResponse { success: false, quotes: vec![] }),
            }
        }
        Err(_) => Json(QuotesResponse { success: false, quotes: vec![] }),
    }
}

async fn place_order(Json(req): Json<PlaceOrderRequest>) -> impl IntoResponse {
    let scripts_dir = std::env::var("NAUTILUS_SCRIPTS_DIR")
        .unwrap_or_else(|_| "D:/Nautilus-Trader/scripts/zerodha".to_string());

    fn escape_py(s: &str) -> String {
        s.replace('\\', "\\\\").replace('\'', "\\'")
    }

    // Map order type
    let order_type = match req.order_type.as_str() {
        "MARKET" => "MARKET",
        "LIMIT" => "LIMIT",
        "SL" => "SL",
        "SL-M" => "SL-M",
        _ => "MARKET",
    };

    // Build Python code to place order via Kite API
    let python_code = format!(
        r#"
import sys
import json
import os
sys.path.insert(0, r'{scripts_dir}')
from kite_api import KiteAPI

# Load metadata
metadata_file = os.path.join(os.path.expanduser('~'), '.nautilus', 'broker_metadata.json')
if not os.path.exists(metadata_file):
    print(json.dumps({{"success": False, "error": "Broker not configured"}}))
    sys.exit(0)

with open(metadata_file) as f:
    metadata = json.load(f)

api_key = metadata.get('api_key') or os.environ.get('KITE_API_KEY')
access_token = metadata.get('access_token') or os.environ.get('KITE_ACCESS_TOKEN')

if not api_key or not access_token:
    print(json.dumps({{"success": False, "error": "API key or access token not configured"}}))
    sys.exit(0)

try:
    kite = KiteAPI(api_key, access_token)

    params = {{
        "tradingsymbol": "{symbol}",
        "exchange": "{exchange}",
        "transaction_type": "{side}",
        "order_type": "{order_type}",
        "product": "{product}",
        "quantity": {quantity},
    }}

    # Add price for LIMIT and SL orders
    if "{order_type}" in ["LIMIT", "SL"]:
        params["price"] = {price}

    # Add trigger price for SL and SL-M orders
    if "{order_type}" in ["SL", "SL-M"]:
        params["trigger_price"] = {trigger_price}

    result = kite.place_order(**params)

    if "order_id" in result:
        print(json.dumps({{"success": True, "orderId": str(result["order_id"])}}))
    else:
        print(json.dumps({{"success": False, "error": str(result)}}))

except Exception as e:
    print(json.dumps({{"success": False, "error": str(e)}}))
"#,
        scripts_dir = scripts_dir,
        symbol = escape_py(&req.symbol),
        exchange = escape_py(&req.exchange),
        side = escape_py(&req.side),
        order_type = order_type,
        product = escape_py(&req.product),
        quantity = req.quantity,
        price = req.price.unwrap_or(0.0),
        trigger_price = req.trigger_price.unwrap_or(0.0),
    );

    let output = Command::new("python")
        .arg("-c")
        .arg(&python_code)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);

            if let Ok(result) = serde_json::from_str::<serde_json::Value>(&stdout) {
                if result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let order_id = result.get("orderId")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    Json(serde_json::json!({
                        "success": true,
                        "orderId": order_id
                    }))
                } else {
                    let error = result.get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown error");
                    Json(serde_json::json!({
                        "success": false,
                        "message": error
                    }))
                }
            } else {
                Json(serde_json::json!({
                    "success": false,
                    "message": format!("Failed to parse response: {} {}", stdout, stderr)
                }))
            }
        }
        Err(e) => {
            Json(serde_json::json!({
                "success": false,
                "message": format!("Failed to execute order: {}", e)
            }))
        }
    }
}

async fn modify_order(Json(req): Json<ModifyOrderRequest>) -> impl IntoResponse {
    let scripts_dir = std::env::var("NAUTILUS_SCRIPTS_DIR")
        .unwrap_or_else(|_| "D:/Nautilus-Trader/scripts/zerodha".to_string());

    fn escape_py(s: &str) -> String {
        s.replace('\\', "\\\\").replace('\'', "\\'")
    }

    let python_code = format!(
        r#"
import sys
import json
import os
sys.path.insert(0, r'{scripts_dir}')
from kite_api import KiteAPI

metadata_file = os.path.join(os.path.expanduser('~'), '.nautilus', 'broker_metadata.json')
if not os.path.exists(metadata_file):
    print(json.dumps({{"success": False, "error": "Broker not configured"}}))
    sys.exit(0)

with open(metadata_file) as f:
    metadata = json.load(f)

api_key = metadata.get('api_key') or os.environ.get('KITE_API_KEY')
access_token = metadata.get('access_token') or os.environ.get('KITE_ACCESS_TOKEN')

if not api_key or not access_token:
    print(json.dumps({{"success": False, "error": "API key or access token not configured"}}))
    sys.exit(0)

try:
    kite = KiteAPI(api_key, access_token)

    params = {{"order_id": "{order_id}"}}
    if {quantity} > 0:
        params["quantity"] = {quantity}
    if {price} > 0:
        params["price"] = {price}
    if {trigger_price} > 0:
        params["trigger_price"] = {trigger_price}

    result = kite.modify_order(**params)
    print(json.dumps({{"success": True, "orderId": str(result.get("order_id", "{order_id}"))}}))

except Exception as e:
    print(json.dumps({{"success": False, "error": str(e)}}))
"#,
        scripts_dir = scripts_dir,
        order_id = escape_py(&req.order_id),
        quantity = req.quantity.unwrap_or(0),
        price = req.price.unwrap_or(0.0),
        trigger_price = req.trigger_price.unwrap_or(0.0),
    );

    let output = Command::new("python")
        .arg("-c")
        .arg(&python_code)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if let Ok(result) = serde_json::from_str::<serde_json::Value>(&stdout) {
                Json(result)
            } else {
                Json(serde_json::json!({
                    "success": false,
                    "message": "Failed to modify order"
                }))
            }
        }
        Err(e) => {
            Json(serde_json::json!({
                "success": false,
                "message": format!("Failed to modify order: {}", e)
            }))
        }
    }
}

async fn cancel_order(Json(req): Json<CancelOrderRequest>) -> impl IntoResponse {
    let scripts_dir = std::env::var("NAUTILUS_SCRIPTS_DIR")
        .unwrap_or_else(|_| "D:/Nautilus-Trader/scripts/zerodha".to_string());

    fn escape_py(s: &str) -> String {
        s.replace('\\', "\\\\").replace('\'', "\\'")
    }

    let python_code = format!(
        r#"
import sys
import json
import os
sys.path.insert(0, r'{scripts_dir}')
from kite_api import KiteAPI

metadata_file = os.path.join(os.path.expanduser('~'), '.nautilus', 'broker_metadata.json')
if not os.path.exists(metadata_file):
    print(json.dumps({{"success": False, "error": "Broker not configured"}}))
    sys.exit(0)

with open(metadata_file) as f:
    metadata = json.load(f)

api_key = metadata.get('api_key') or os.environ.get('KITE_API_KEY')
access_token = metadata.get('access_token') or os.environ.get('KITE_ACCESS_TOKEN')

if not api_key or not access_token:
    print(json.dumps({{"success": False, "error": "API key or access token not configured"}}))
    sys.exit(0)

try:
    kite = KiteAPI(api_key, access_token)
    result = kite.cancel_order("{order_id}")
    print(json.dumps({{"success": True, "orderId": "{order_id}"}}))

except Exception as e:
    print(json.dumps({{"success": False, "error": str(e)}}))
"#,
        scripts_dir = scripts_dir,
        order_id = escape_py(&req.order_id),
    );

    let output = Command::new("python")
        .arg("-c")
        .arg(&python_code)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if let Ok(result) = serde_json::from_str::<serde_json::Value>(&stdout) {
                Json(result)
            } else {
                Json(serde_json::json!({
                    "success": false,
                    "message": "Failed to cancel order"
                }))
            }
        }
        Err(e) => {
            Json(serde_json::json!({
                "success": false,
                "message": format!("Failed to cancel order: {}", e)
            }))
        }
    }
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

# Also save non-sensitive metadata for UI display and API calls
import json
from pathlib import Path
metadata_file = Path.home() / '.nautilus' / 'broker_metadata.json'
metadata_file.parent.mkdir(parents=True, exist_ok=True)
api_key = '{api_key}'
metadata = {{
    'broker_id': 'zerodha',
    'broker_name': '{broker_name}' or 'Zerodha',
    'api_key': api_key,  # Full key needed for API calls
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
    from pathlib import Path
    token_file = os.path.expanduser('~/.zerodha_token.json')
    with open(token_file) as f:
        data = json.load(f)

    # Update metadata file with full API key for subsequent API calls
    metadata_file = Path.home() / '.nautilus' / 'broker_metadata.json'
    if metadata_file.exists():
        with open(metadata_file) as f:
            metadata = json.load(f)
        # Add full api_key if not present
        if 'api_key' not in metadata or not metadata['api_key']:
            metadata['api_key'] = creds.api_key
            with open(metadata_file, 'w') as f:
                json.dump(metadata, f)

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
    #[serde(rename = "needsApiKeyUpdate")]
    needs_api_key_update: bool,
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
    # Check if full api_key is present (not just prefix)
    has_full_api_key = 'api_key' in data and data['api_key'] and not data['api_key'].endswith('...')
    print(json.dumps({{
        "exists": True,
        "brokerId": data.get("broker_id", "zerodha"),
        "brokerName": data.get("broker_name", "Zerodha"),
        "apiKeyPrefix": data.get("api_key_prefix", ""),
        "userId": data.get("user_id", ""),
        "needsApiKeyUpdate": not has_full_api_key
    }}))
else:
    # Check if encrypted credentials exist
    from credential_store import CredentialStore
    store = CredentialStore()
    if store.exists():
        print(json.dumps({{"exists": True, "brokerId": "zerodha", "brokerName": "Zerodha", "needsApiKeyUpdate": True}}))
    else:
        print(json.dumps({{"exists": False, "needsApiKeyUpdate": False}}))
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
                    needs_api_key_update: json.get("needsApiKeyUpdate").and_then(|v| v.as_bool()).unwrap_or(false),
                }),
                Err(_) => Json(BrokerConfigResponse {
                    exists: false,
                    broker_id: None,
                    broker_name: None,
                    api_key_prefix: None,
                    user_id: None,
                    needs_api_key_update: false,
                }),
            }
        }
        Err(_) => Json(BrokerConfigResponse {
            exists: false,
            broker_id: None,
            broker_name: None,
            api_key_prefix: None,
            user_id: None,
            needs_api_key_update: false,
        }),
    }
}

// Migrate API key from vault to metadata
#[derive(Debug, Deserialize)]
pub struct MigrateApiKeyRequest {
    pub master_password: String,
}

#[derive(Debug, Serialize)]
pub struct MigrateApiKeyResponse {
    pub success: bool,
    pub message: String,
}

// ============================================================================
// AI Service Types and Handlers
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ChatMessageDto {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct GenerateStrategyHttpRequest {
    pub prompt: String,
    #[serde(rename = "visualBuilderJson", default)]
    pub visual_builder_json: Option<String>,
    #[serde(rename = "inputMode", default)]
    pub input_mode: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub history: Option<Vec<ChatMessageDto>>,
}

#[derive(Debug, Serialize)]
pub struct GenerateStrategyHttpResponse {
    pub success: bool,
    #[serde(rename = "strategyCode")]
    pub strategy_code: String,
    #[serde(rename = "strategyName")]
    pub strategy_name: String,
    pub error: String,
    #[serde(rename = "providerUsed")]
    pub provider_used: String,
    #[serde(rename = "modelUsed")]
    pub model_used: String,
    #[serde(rename = "tokensUsed")]
    pub tokens_used: i32,
}

#[derive(Debug, Deserialize)]
pub struct RefineStrategyHttpRequest {
    #[serde(rename = "currentCode")]
    pub current_code: String,
    pub instructions: String,
    pub provider: Option<String>,
    pub history: Option<Vec<ChatMessageDto>>,
}

#[derive(Debug, Serialize)]
pub struct RefineStrategyHttpResponse {
    pub success: bool,
    #[serde(rename = "refinedCode")]
    pub refined_code: String,
    #[serde(rename = "changesSummary")]
    pub changes_summary: String,
    pub error: String,
    #[serde(rename = "providerUsed")]
    pub provider_used: String,
    #[serde(rename = "tokensUsed")]
    pub tokens_used: i32,
}

#[derive(Debug, Serialize)]
pub struct AiModelDto {
    pub id: String,
    pub name: String,
    pub recommended: bool,
    #[serde(rename = "contextLength")]
    pub context_length: i32,
}

#[derive(Debug, Serialize)]
pub struct AiProviderDto {
    pub id: String,
    pub name: String,
    pub configured: bool,
    pub available: bool,
    pub models: Vec<AiModelDto>,
}

#[derive(Debug, Serialize)]
pub struct AiStatusResponse {
    pub providers: Vec<AiProviderDto>,
    #[serde(rename = "defaultProvider")]
    pub default_provider: String,
}

#[derive(Debug, Deserialize)]
pub struct SaveAiConfigHttpRequest {
    pub provider: String,
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(rename = "defaultModel", default)]
    pub default_model: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SaveAiConfigHttpResponse {
    pub success: bool,
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct AiConfigResponse {
    #[serde(rename = "defaultProvider")]
    pub default_provider: String,
    #[serde(rename = "groqConfigured")]
    pub groq_configured: bool,
    #[serde(rename = "groqApiKeyMasked")]
    pub groq_api_key_masked: String,
    #[serde(rename = "groqDefaultModel")]
    pub groq_default_model: String,
    #[serde(rename = "claudeConfigured")]
    pub claude_configured: bool,
    #[serde(rename = "claudeApiKeyMasked")]
    pub claude_api_key_masked: String,
    #[serde(rename = "claudeDefaultModel")]
    pub claude_default_model: String,
}

#[derive(Debug, Deserialize)]
pub struct TestAiConnectionRequest {
    pub provider: String,
    #[serde(rename = "apiKey")]
    pub api_key: String,
}

#[derive(Debug, Serialize)]
pub struct TestAiConnectionResponse {
    pub success: bool,
    pub error: String,
    #[serde(rename = "availableModels")]
    pub available_models: Vec<String>,
}

async fn generate_strategy(
    State(state): State<AppState>,
    Json(req): Json<GenerateStrategyHttpRequest>,
) -> impl IntoResponse {
    use crate::proto::ai::{GenerateStrategyRequest, ChatMessage};
    use tonic::Request;

    let history: Vec<ChatMessage> = req.history.unwrap_or_default()
        .into_iter()
        .map(|m| ChatMessage { role: m.role, content: m.content })
        .collect();

    let grpc_req = GenerateStrategyRequest {
        prompt: req.prompt,
        visual_builder_json: req.visual_builder_json.unwrap_or_default(),
        input_mode: req.input_mode.unwrap_or_else(|| "natural_language".to_string()),
        provider: req.provider.unwrap_or_default(),
        model: req.model.unwrap_or_default(),
        history,
    };

    match state.ai_service.generate_strategy(Request::new(grpc_req)).await {
        Ok(resp) => {
            let resp = resp.into_inner();
            Json(GenerateStrategyHttpResponse {
                success: resp.success,
                strategy_code: resp.strategy_code,
                strategy_name: resp.strategy_name,
                error: resp.error,
                provider_used: resp.provider_used,
                model_used: resp.model_used,
                tokens_used: resp.tokens_used,
            })
        }
        Err(e) => {
            Json(GenerateStrategyHttpResponse {
                success: false,
                strategy_code: String::new(),
                strategy_name: String::new(),
                error: e.message().to_string(),
                provider_used: String::new(),
                model_used: String::new(),
                tokens_used: 0,
            })
        }
    }
}

async fn refine_strategy(
    State(state): State<AppState>,
    Json(req): Json<RefineStrategyHttpRequest>,
) -> impl IntoResponse {
    use crate::proto::ai::{RefineStrategyRequest, ChatMessage};
    use tonic::Request;

    let history: Vec<ChatMessage> = req.history.unwrap_or_default()
        .into_iter()
        .map(|m| ChatMessage { role: m.role, content: m.content })
        .collect();

    let grpc_req = RefineStrategyRequest {
        current_code: req.current_code,
        instructions: req.instructions,
        provider: req.provider.unwrap_or_default(),
        history,
    };

    match state.ai_service.refine_strategy(Request::new(grpc_req)).await {
        Ok(resp) => {
            let resp = resp.into_inner();
            Json(RefineStrategyHttpResponse {
                success: resp.success,
                refined_code: resp.refined_code,
                changes_summary: resp.changes_summary,
                error: resp.error,
                provider_used: resp.provider_used,
                tokens_used: resp.tokens_used,
            })
        }
        Err(e) => {
            Json(RefineStrategyHttpResponse {
                success: false,
                refined_code: String::new(),
                changes_summary: String::new(),
                error: e.message().to_string(),
                provider_used: String::new(),
                tokens_used: 0,
            })
        }
    }
}

async fn get_ai_status(State(state): State<AppState>) -> impl IntoResponse {
    use crate::proto::ai::GetAiStatusRequest;
    use tonic::Request;

    match state.ai_service.get_ai_status(Request::new(GetAiStatusRequest {})).await {
        Ok(resp) => {
            let resp = resp.into_inner();
            Json(AiStatusResponse {
                providers: resp.providers.into_iter().map(|p| AiProviderDto {
                    id: p.id,
                    name: p.name,
                    configured: p.configured,
                    available: p.available,
                    models: p.models.into_iter().map(|m| AiModelDto {
                        id: m.id,
                        name: m.name,
                        recommended: m.recommended,
                        context_length: m.context_length,
                    }).collect(),
                }).collect(),
                default_provider: resp.default_provider,
            })
        }
        Err(_) => {
            Json(AiStatusResponse {
                providers: vec![],
                default_provider: String::new(),
            })
        }
    }
}

async fn save_ai_config(
    State(state): State<AppState>,
    Json(req): Json<SaveAiConfigHttpRequest>,
) -> impl IntoResponse {
    use crate::proto::ai::SaveAiConfigRequest;
    use tonic::Request;

    let grpc_req = SaveAiConfigRequest {
        provider: req.provider,
        api_key: req.api_key,
        default_model: req.default_model.unwrap_or_default(),
    };

    match state.ai_service.save_ai_config(Request::new(grpc_req)).await {
        Ok(resp) => {
            let resp = resp.into_inner();
            Json(SaveAiConfigHttpResponse {
                success: resp.success,
                error: resp.error,
            })
        }
        Err(e) => {
            Json(SaveAiConfigHttpResponse {
                success: false,
                error: e.message().to_string(),
            })
        }
    }
}

async fn get_ai_config(State(state): State<AppState>) -> impl IntoResponse {
    use crate::proto::ai::GetAiConfigRequest;
    use tonic::Request;

    match state.ai_service.get_ai_config(Request::new(GetAiConfigRequest {})).await {
        Ok(resp) => {
            let resp = resp.into_inner();
            Json(AiConfigResponse {
                default_provider: resp.default_provider,
                groq_configured: resp.groq_configured,
                groq_api_key_masked: resp.groq_api_key_masked,
                groq_default_model: resp.groq_default_model,
                claude_configured: resp.claude_configured,
                claude_api_key_masked: resp.claude_api_key_masked,
                claude_default_model: resp.claude_default_model,
            })
        }
        Err(_) => {
            Json(AiConfigResponse {
                default_provider: String::new(),
                groq_configured: false,
                groq_api_key_masked: String::new(),
                groq_default_model: String::new(),
                claude_configured: false,
                claude_api_key_masked: String::new(),
                claude_default_model: String::new(),
            })
        }
    }
}

async fn test_ai_connection(
    State(state): State<AppState>,
    Json(req): Json<TestAiConnectionRequest>,
) -> impl IntoResponse {
    use crate::proto::ai::TestConnectionRequest;
    use tonic::Request;

    let grpc_req = TestConnectionRequest {
        provider: req.provider,
        api_key: req.api_key,
    };

    match state.ai_service.test_connection(Request::new(grpc_req)).await {
        Ok(resp) => {
            let resp = resp.into_inner();
            Json(TestAiConnectionResponse {
                success: resp.success,
                error: resp.error,
                available_models: resp.available_models,
            })
        }
        Err(e) => {
            Json(TestAiConnectionResponse {
                success: false,
                error: e.message().to_string(),
                available_models: vec![],
            })
        }
    }
}

// ============================================================================
// Legacy Handlers
// ============================================================================

async fn migrate_api_key(Json(req): Json<MigrateApiKeyRequest>) -> impl IntoResponse {
    fn escape_py(s: &str) -> String {
        s.replace('\\', "\\\\").replace('\'', "\\'")
    }

    let scripts_dir = std::env::var("NAUTILUS_SCRIPTS_DIR")
        .unwrap_or_else(|_| "D:/Nautilus-Trader/scripts/zerodha".to_string());

    let python_code = format!(
        r#"
import sys
import json
from pathlib import Path
sys.path.insert(0, r'{scripts_dir}')
from credential_store import CredentialStore

metadata_file = Path.home() / '.nautilus' / 'broker_metadata.json'
store = CredentialStore()

# Try to unlock vault
if not store.unlock('{master_password}'):
    print(json.dumps({{"success": False, "message": "Invalid master password"}}))
    sys.exit(0)

# Load credentials from vault
creds = store.load()
if not creds:
    print(json.dumps({{"success": False, "message": "No credentials found in vault"}}))
    sys.exit(0)

# Load and update metadata
if metadata_file.exists():
    with open(metadata_file) as f:
        metadata = json.load(f)
else:
    metadata = {{}}

metadata['api_key'] = creds.api_key
metadata['api_key_prefix'] = creds.api_key[:8] + '...' if len(creds.api_key) > 8 else creds.api_key

with open(metadata_file, 'w') as f:
    json.dump(metadata, f)

print(json.dumps({{"success": True, "message": "API key migrated successfully"}}))
"#,
        scripts_dir = scripts_dir,
        master_password = escape_py(&req.master_password),
    );

    let output = Command::new("python")
        .arg("-c")
        .arg(&python_code)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            match serde_json::from_str::<serde_json::Value>(&stdout) {
                Ok(json) => Json(MigrateApiKeyResponse {
                    success: json.get("success").and_then(|v| v.as_bool()).unwrap_or(false),
                    message: json.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                }),
                Err(e) => Json(MigrateApiKeyResponse {
                    success: false,
                    message: format!("Parse error: {}", e),
                }),
            }
        }
        Err(e) => Json(MigrateApiKeyResponse {
            success: false,
            message: format!("Failed to run migration: {}", e),
        }),
    }
}

// ============================================================================
// Backtest Service Types and Handlers
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct GetHistoricalDataHttpRequest {
    #[serde(rename = "brokerId")]
    pub broker_id: String,
    pub symbol: String,
    pub exchange: String,
    pub interval: String,
    #[serde(rename = "fromDate")]
    pub from_date: String,
    #[serde(rename = "toDate")]
    pub to_date: String,
}

#[derive(Debug, Serialize)]
pub struct BarDto {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
}

#[derive(Debug, Serialize)]
pub struct GetHistoricalDataHttpResponse {
    pub success: bool,
    pub error: String,
    pub bars: Vec<BarDto>,
    #[serde(rename = "totalBars")]
    pub total_bars: i32,
}

#[derive(Debug, Deserialize)]
pub struct RunBacktestHttpRequest {
    #[serde(rename = "strategyCode")]
    pub strategy_code: String,
    #[serde(rename = "strategyName")]
    pub strategy_name: String,
    #[serde(rename = "brokerId")]
    pub broker_id: String,
    pub symbols: Vec<String>,
    pub exchange: String,
    pub interval: String,
    #[serde(rename = "fromDate")]
    pub from_date: String,
    #[serde(rename = "toDate")]
    pub to_date: String,
    #[serde(rename = "initialCapital")]
    pub initial_capital: f64,
    #[serde(rename = "positionSize")]
    pub position_size: f64,
    #[serde(rename = "positionSizeType")]
    pub position_size_type: String,
    #[serde(rename = "commissionRate")]
    pub commission_rate: f64,
    #[serde(rename = "slippageRate")]
    pub slippage_rate: f64,
    #[serde(rename = "strategyParams", default)]
    pub strategy_params: String,
}

#[derive(Debug, Deserialize)]
pub struct GetBacktestResultsHttpRequest {
    #[serde(rename = "backtestId")]
    pub backtest_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ListBacktestsHttpRequest {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

async fn get_historical_data(
    State(state): State<AppState>,
    Json(req): Json<GetHistoricalDataHttpRequest>,
) -> impl IntoResponse {
    use crate::proto::backtest::GetHistoricalDataRequest;
    use tonic::Request;

    let grpc_req = GetHistoricalDataRequest {
        broker_id: req.broker_id,
        symbol: req.symbol,
        exchange: req.exchange,
        interval: req.interval,
        from_date: req.from_date,
        to_date: req.to_date,
    };

    match state.backtest_service.get_historical_data(Request::new(grpc_req)).await {
        Ok(resp) => {
            let resp = resp.into_inner();
            Json(GetHistoricalDataHttpResponse {
                success: resp.success,
                error: resp.error,
                bars: resp.bars.into_iter().map(|b| BarDto {
                    timestamp: b.timestamp,
                    open: b.open,
                    high: b.high,
                    low: b.low,
                    close: b.close,
                    volume: b.volume,
                }).collect(),
                total_bars: resp.total_bars,
            })
        }
        Err(e) => {
            Json(GetHistoricalDataHttpResponse {
                success: false,
                error: e.message().to_string(),
                bars: vec![],
                total_bars: 0,
            })
        }
    }
}

async fn run_backtest_sse(
    State(state): State<AppState>,
    Json(req): Json<RunBacktestHttpRequest>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    use crate::proto::backtest::RunBacktestRequest;
    use tonic::Request;
    use tokio_stream::StreamExt;

    let grpc_req = RunBacktestRequest {
        strategy_code: req.strategy_code,
        strategy_name: req.strategy_name,
        broker_id: req.broker_id,
        symbols: req.symbols,
        exchange: req.exchange,
        interval: req.interval,
        from_date: req.from_date,
        to_date: req.to_date,
        initial_capital: req.initial_capital,
        position_size: req.position_size,
        position_size_type: req.position_size_type,
        commission_rate: req.commission_rate,
        slippage_rate: req.slippage_rate,
        strategy_params: req.strategy_params,
    };

    let stream = async_stream::stream! {
        match state.backtest_service.run_backtest(Request::new(grpc_req)).await {
            Ok(resp) => {
                let mut stream = resp.into_inner();
                while let Some(progress_result) = stream.next().await {
                    match progress_result {
                        Ok(progress) => {
                            let json = serde_json::json!({
                                "status": progress.status,
                                "progressPercent": progress.progress_percent,
                                "message": progress.message,
                                "currentEquity": progress.current_equity,
                                "tradesCompleted": progress.trades_completed,
                                "results": progress.results.map(|r| serde_json::json!({
                                    "backtestId": r.backtest_id,
                                    "strategyName": r.strategy_name,
                                    "startTime": r.start_time,
                                    "endTime": r.end_time,
                                    "totalReturn": r.total_return,
                                    "totalReturnPercent": r.total_return_percent,
                                    "annualizedReturn": r.annualized_return,
                                    "sharpeRatio": r.sharpe_ratio,
                                    "sortinoRatio": r.sortino_ratio,
                                    "maxDrawdown": r.max_drawdown,
                                    "maxDrawdownPercent": r.max_drawdown_percent,
                                    "winRate": r.win_rate,
                                    "profitFactor": r.profit_factor,
                                    "avgWin": r.avg_win,
                                    "avgLoss": r.avg_loss,
                                    "totalTrades": r.total_trades,
                                    "winningTrades": r.winning_trades,
                                    "losingTrades": r.losing_trades,
                                    "equityCurve": r.equity_curve.iter().map(|e| serde_json::json!({
                                        "timestamp": e.timestamp,
                                        "equity": e.equity,
                                        "drawdown": e.drawdown,
                                    })).collect::<Vec<_>>(),
                                    "trades": r.trades.iter().map(|t| serde_json::json!({
                                        "tradeId": t.trade_id,
                                        "symbol": t.symbol,
                                        "side": t.side,
                                        "entryTime": t.entry_time,
                                        "exitTime": t.exit_time,
                                        "entryPrice": t.entry_price,
                                        "exitPrice": t.exit_price,
                                        "quantity": t.quantity,
                                        "pnl": t.pnl,
                                        "pnlPercent": t.pnl_percent,
                                        "commission": t.commission,
                                        "exitReason": t.exit_reason,
                                    })).collect::<Vec<_>>(),
                                    "signals": r.signals.iter().map(|s| serde_json::json!({
                                        "timestamp": s.timestamp,
                                        "symbol": s.symbol,
                                        "type": s.r#type,
                                        "price": s.price,
                                        "reason": s.reason,
                                    })).collect::<Vec<_>>(),
                                })),
                            });
                            yield Ok(Event::default().data(json.to_string()));
                        }
                        Err(e) => {
                            let error_json = serde_json::json!({
                                "status": "error",
                                "message": e.message(),
                            });
                            yield Ok(Event::default().data(error_json.to_string()));
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                let error_json = serde_json::json!({
                    "status": "error",
                    "message": e.message(),
                });
                yield Ok(Event::default().data(error_json.to_string()));
            }
        }
    };

    Sse::new(stream)
}

async fn get_backtest_results(
    State(state): State<AppState>,
    Json(req): Json<GetBacktestResultsHttpRequest>,
) -> impl IntoResponse {
    use crate::proto::backtest::GetBacktestResultsRequest;
    use tonic::Request;

    let grpc_req = GetBacktestResultsRequest {
        backtest_id: req.backtest_id,
    };

    match state.backtest_service.get_backtest_results(Request::new(grpc_req)).await {
        Ok(resp) => {
            let resp = resp.into_inner();
            Json(serde_json::json!({
                "success": resp.success,
                "error": resp.error,
                "results": resp.results.map(|r| serde_json::json!({
                    "backtestId": r.backtest_id,
                    "strategyName": r.strategy_name,
                    "startTime": r.start_time,
                    "endTime": r.end_time,
                    "totalReturn": r.total_return,
                    "totalReturnPercent": r.total_return_percent,
                    "annualizedReturn": r.annualized_return,
                    "sharpeRatio": r.sharpe_ratio,
                    "sortinoRatio": r.sortino_ratio,
                    "maxDrawdown": r.max_drawdown,
                    "maxDrawdownPercent": r.max_drawdown_percent,
                    "winRate": r.win_rate,
                    "profitFactor": r.profit_factor,
                    "avgWin": r.avg_win,
                    "avgLoss": r.avg_loss,
                    "totalTrades": r.total_trades,
                    "winningTrades": r.winning_trades,
                    "losingTrades": r.losing_trades,
                    "equityCurve": r.equity_curve.iter().map(|e| serde_json::json!({
                        "timestamp": e.timestamp,
                        "equity": e.equity,
                        "drawdown": e.drawdown,
                    })).collect::<Vec<_>>(),
                    "trades": r.trades.iter().map(|t| serde_json::json!({
                        "tradeId": t.trade_id,
                        "symbol": t.symbol,
                        "side": t.side,
                        "entryTime": t.entry_time,
                        "exitTime": t.exit_time,
                        "entryPrice": t.entry_price,
                        "exitPrice": t.exit_price,
                        "quantity": t.quantity,
                        "pnl": t.pnl,
                        "pnlPercent": t.pnl_percent,
                        "commission": t.commission,
                        "exitReason": t.exit_reason,
                    })).collect::<Vec<_>>(),
                    "signals": r.signals.iter().map(|s| serde_json::json!({
                        "timestamp": s.timestamp,
                        "symbol": s.symbol,
                        "type": s.r#type,
                        "price": s.price,
                        "reason": s.reason,
                    })).collect::<Vec<_>>(),
                })),
            }))
        }
        Err(e) => {
            Json(serde_json::json!({
                "success": false,
                "error": e.message(),
            }))
        }
    }
}

async fn list_backtests(
    State(state): State<AppState>,
    Json(req): Json<ListBacktestsHttpRequest>,
) -> impl IntoResponse {
    use crate::proto::backtest::ListBacktestsRequest;
    use tonic::Request;

    let grpc_req = ListBacktestsRequest {
        limit: req.limit.unwrap_or(10),
        offset: req.offset.unwrap_or(0),
    };

    match state.backtest_service.list_backtests(Request::new(grpc_req)).await {
        Ok(resp) => {
            let resp = resp.into_inner();
            Json(serde_json::json!({
                "backtests": resp.backtests.iter().map(|b| serde_json::json!({
                    "backtestId": b.backtest_id,
                    "strategyName": b.strategy_name,
                    "createdAt": b.created_at,
                    "totalReturnPercent": b.total_return_percent,
                    "sharpeRatio": b.sharpe_ratio,
                    "totalTrades": b.total_trades,
                })).collect::<Vec<_>>(),
                "total": resp.total,
            }))
        }
        Err(e) => {
            Json(serde_json::json!({
                "backtests": [],
                "total": 0,
                "error": e.message(),
            }))
        }
    }
}

pub fn create_http_router(
    registry: Arc<ClientRegistry>,
    clients: SharedClients,
    ai_service: Arc<AiServiceImpl>,
    backtest_service: Arc<BacktestServiceImpl>,
) -> Router {
    let state = AppState { registry, clients, ai_service, backtest_service };
    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any);

    Router::new()
        // Backtest Service routes
        .route("/backtest.BacktestService/GetHistoricalData", post(get_historical_data))
        .route("/backtest.BacktestService/RunBacktest", post(run_backtest_sse))
        .route("/backtest.BacktestService/GetBacktestResults", post(get_backtest_results))
        .route("/backtest.BacktestService/ListBacktests", post(list_backtests))
        // AI Service routes
        .route("/ai.AiService/GenerateStrategy", post(generate_strategy))
        .route("/ai.AiService/RefineStrategy", post(refine_strategy))
        .route("/ai.AiService/GetAiStatus", post(get_ai_status))
        .route("/ai.AiService/SaveAiConfig", post(save_ai_config))
        .route("/ai.AiService/GetAiConfig", post(get_ai_config))
        .route("/ai.AiService/TestConnection", post(test_ai_connection))
        // Broker Service routes
        .route("/broker.BrokerService/ListBrokers", post(list_brokers))
        .route("/broker.BrokerService/RegisterBroker", post(register_broker))
        .route("/broker.BrokerService/TestConnection", post(test_connection))
        // Zerodha token management
        .route("/broker.BrokerService/SaveCredentials", post(save_zerodha_credentials))
        .route("/broker.BrokerService/RefreshToken", post(refresh_zerodha_token))
        .route("/broker.BrokerService/GetTokenStatus", post(get_token_status))
        .route("/broker.BrokerService/GetBrokerConfig", post(get_broker_config))
        .route("/broker.BrokerService/MigrateApiKey", post(migrate_api_key))
        .route("/portfolio.PortfolioService/GetDaySummary", post(get_day_summary))
        .route("/portfolio.PortfolioService/GetHoldings", post(get_holdings))
        .route("/portfolio.PortfolioService/GetMargins", post(get_margins))
        .route("/portfolio.PortfolioService/GetPositions", post(get_positions))
        .route("/marketdata.MarketDataService/SearchInstruments", post(search_instruments))
        .route("/marketdata.MarketDataService/GetHistoricalBars", post(get_historical_bars))
        .route("/marketdata.MarketDataService/GetQuotes", post(get_quotes))
        .route("/order.OrderService/PlaceOrder", post(place_order))
        .route("/order.OrderService/ModifyOrder", post(modify_order))
        .route("/order.OrderService/CancelOrder", post(cancel_order))
        .layer(cors)
        .with_state(state)
}
