# Nautilus Zerodha Adapter

High-performance Rust adapter for Zerodha Kite Connect API integration with NautilusTrader.

## Features

- **REST API Client**: Full coverage of Zerodha Kite Connect v3 API
- **WebSocket Client**: Real-time market data and order updates via Kite Ticker
- **Options Trading**: Complete support for NSE/BSE options with Greeks and IV
- **Risk Management**: SPAN margin calculations and position monitoring
- **High Performance**: Async Rust implementation with low-latency data processing

## Zerodha API Coverage

### Market Data
- [x] Instruments list (equity, derivatives, commodities)
- [x] Quote data (LTP, OHLC, volume)
- [x] Order book depth (L2 data)
- [x] Historical candles (minute, daily)
- [ ] Options chain data
- [ ] Market status

### Trading
- [ ] Order placement (market, limit, SL, SLM)
- [ ] Order modification and cancellation
- [ ] Positions and holdings
- [ ] Portfolio margins
- [ ] Trade book and order book

### WebSocket Feeds
- [ ] Live quotes and ticks
- [ ] Order updates
- [ ] Position updates

## Configuration

Set up your Zerodha API credentials:

```toml
[zerodha]
api_key = "your_api_key"
api_secret = "your_api_secret"
access_token = "your_access_token"
sandbox = true
```

## Usage

```rust
use nautilus_zerodha::http::ZerodhaHttpClient;
use nautilus_zerodha::config::ZerodhaConfig;

#[tokio::main]
async fn main() {
    let config = ZerodhaConfig {
        api_key: "your_api_key".to_string(),
        api_secret: "your_api_secret".to_string(),
        access_token: Some("your_access_token".to_string()),
        sandbox: true,
        ..Default::default()
    };
    
    let client = ZerodhaHttpClient::new(config);
    let instruments = client.get_instruments().await?;
    println!("Loaded {} instruments", instruments.len());
}
```

## Testing

Run the test binaries to validate your setup:

```bash
# Test instrument fetching
cargo run --bin zerodha-http-instruments

# Test order placement (paper trading)
cargo run --bin zerodha-http-orders

# Test WebSocket data feed
cargo run --bin zerodha-ws-data
```

## References

- [Zerodha Kite Connect Documentation](https://kite.trade/docs/connect/)
- [NSE Options Trading Guide](https://www.nseindia.com/products-services/derivatives)
- [NautilusTrader Documentation](https://nautilustrader.io/docs/)