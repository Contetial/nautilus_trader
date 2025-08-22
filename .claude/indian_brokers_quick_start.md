# Quick Start Guide: Indian Brokers Integration

## Getting Started Checklist

### Prerequisites
- [ ] NautilusTrader development environment set up
- [ ] Rust 1.89.0+ installed
- [ ] Python 3.11-3.13 with development dependencies
- [ ] Indian broker API credentials (Zerodha/Angel One/Upstox)
- [ ] Understanding of Indian options market structure

### Phase 1: Start with Zerodha (Recommended First Broker)

#### 1. Set up Zerodha Developer Account
```bash
# Register at https://developers.kite.trade/
# Get API key and API secret
# Generate access token (manual process initially)
```

#### 2. Create Initial Adapter Structure
```bash
# Create Rust crate
mkdir -p crates/adapters/zerodha
cd crates/adapters/zerodha

# Initialize Cargo.toml (copy from existing adapter like OKX)
cargo init --lib
```

#### 3. Minimal HTTP Client Implementation
```rust
// src/http/client.rs
use reqwest::Client;
use serde_json::Value;

pub struct ZerodhaHttpClient {
    client: Client,
    api_key: String,
    access_token: String,
    base_url: String,
}

impl ZerodhaHttpClient {
    pub fn new(api_key: String, access_token: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            access_token,
            base_url: "https://api.kite.trade".to_string(),
        }
    }
    
    pub async fn get_instruments(&self) -> Result<Value, reqwest::Error> {
        let url = format!("{}/instruments", self.base_url);
        let response = self.client
            .get(&url)
            .header("X-Kite-Version", "3")
            .header("Authorization", format!("token {}:{}", self.api_key, self.access_token))
            .send()
            .await?;
        
        response.json().await
    }
}
```

#### 4. Test with Simple Market Data
```rust
// bin/test-instruments.rs
use zerodha_adapter::http::ZerodhaHttpClient;

#[tokio::main]
async fn main() {
    let client = ZerodhaHttpClient::new(
        "your_api_key".to_string(),
        "your_access_token".to_string()
    );
    
    match client.get_instruments().await {
        Ok(instruments) => println!("Instruments: {:#?}", instruments),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

### Phase 2: Basic Integration Testing

#### 1. Environment Setup
```python
# Create test configuration
from nautilus_trader.adapters.zerodha.config import ZerodhaConfig

config = ZerodhaConfig(
    api_key="your_api_key",
    api_secret="your_api_secret", 
    access_token="your_access_token",
    sandbox=True  # Start with sandbox/testnet
)
```

#### 2. Test Data Connection
```python
# Test basic market data
from nautilus_trader.adapters.zerodha.data import ZerodhaDataClient

data_client = ZerodhaDataClient(config=config)
await data_client.connect()

# Subscribe to Nifty index data
nifty_id = InstrumentId.from_str("NIFTY50.NSE")
data_client.subscribe_quote_ticks(nifty_id)
```

#### 3. Test Options Chain Retrieval
```python
# Get Nifty options chain
from datetime import date, timedelta

expiry_date = date.today() + timedelta(days=7)  # Next weekly expiry
options_chain = await data_client.get_options_chain("NIFTY", expiry_date)

print(f"Available strikes: {len(options_chain)}")
for strike, data in options_chain.items():
    print(f"Strike {strike}: Call={data.call_price}, Put={data.put_price}")
```

### Common Development Patterns

#### 1. Error Handling Pattern
```rust
#[derive(thiserror::Error, Debug)]
pub enum ZerodhaError {
    #[error("API error: {0}")]
    ApiError(String),
    
    #[error("Authentication failed: {0}")]
    AuthError(String),
    
    #[error("Rate limit exceeded")]
    RateLimit,
    
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),
}
```

#### 2. Configuration Pattern
```python
@dataclass
class ZerodhaConfig(NautilusConfig):
    """Zerodha adapter configuration."""
    api_key: str
    api_secret: str
    access_token: str | None = None
    sandbox: bool = True
    base_url: str = "https://api.kite.trade"
    websocket_url: str = "wss://ws.kite.trade"
    rate_limit_per_second: int = 3
    request_timeout: float = 10.0
    max_retries: int = 3
```

#### 3. Instrument ID Mapping
```python
def create_instrument_id(zerodha_instrument) -> InstrumentId:
    """Convert Zerodha instrument to Nautilus InstrumentId."""
    if zerodha_instrument['segment'] == 'NFO':  # Options
        symbol = f"{zerodha_instrument['tradingsymbol']}"
        return InstrumentId.from_str(f"{symbol}.NSE")
    else:  # Equity
        return InstrumentId.from_str(f"{zerodha_instrument['tradingsymbol']}.NSE")
```

### Development Tools & Testing

#### 1. API Testing Tools
```bash
# Install HTTPie for API testing
pip install httpie

# Test Zerodha API endpoints
http GET https://api.kite.trade/instruments \
    "X-Kite-Version:3" \
    "Authorization:token API_KEY:ACCESS_TOKEN"
```

#### 2. WebSocket Testing
```python
# Simple WebSocket test
import asyncio
import websockets
import json

async def test_websocket():
    uri = "wss://ws.kite.trade?api_key=your_api_key&access_token=your_access_token"
    
    async with websockets.connect(uri) as websocket:
        # Subscribe to instruments
        subscribe_msg = {
            "a": "subscribe",
            "v": [256265, 408065]  # Nifty 50, Bank Nifty tokens
        }
        await websocket.send(json.dumps(subscribe_msg))
        
        # Listen for data
        while True:
            message = await websocket.recv()
            print(f"Received: {message}")

asyncio.run(test_websocket())
```

### Directory Structure Checklist
```
crates/adapters/zerodha/
├── ✅ Cargo.toml
├── ✅ src/lib.rs
├── ✅ src/config.rs
├── ✅ src/http/client.rs
├── ⏳ src/websocket/client.rs
├── ⏳ src/python/mod.rs
└── ✅ bin/test-instruments.rs

nautilus_trader/adapters/zerodha/
├── ✅ __init__.py
├── ✅ config.py
├── ⏳ data.py
├── ⏳ execution.py
└── ⏳ factories.py
```

### Immediate Next Steps (Week 1)
1. **Set up Zerodha developer account** and get API credentials
2. **Create basic Rust HTTP client** for instrument data
3. **Test API connectivity** with simple instrument fetching
4. **Implement basic Python bindings** for configuration
5. **Create simple integration test** to validate setup

### Resources
- **Zerodha Kite Connect Docs**: https://kite.trade/docs/connect/
- **NautilusTrader Adapter Examples**: Check `crates/adapters/okx/` for reference patterns
- **Indian Market Data**: NSE and BSE official documentation
- **Options Trading**: Understanding Indian F&O segment specifics

This quick start guide provides a practical path to begin implementation while following NautilusTrader's established patterns.