# Zerodha Adapter Configuration

This directory contains configuration files for the Zerodha adapter. The adapter supports secure credential management through TOML configuration files with environment variable fallbacks.

## 🚀 Quick Start

### 1. Create Configuration File

Copy the example configuration:
```bash
cp zerodha_credentials.toml.example zerodha_credentials.toml
```

### 2. Add Your Credentials

Edit `zerodha_credentials.toml` and replace the placeholder values:

```toml
[api]
api_key = "your_actual_api_key"
api_secret = "your_actual_api_secret"  
access_token = "your_actual_access_token"

[settings]
trading_mode = "paper"  # Start with paper trading!
sandbox = true
```

### 3. Test Configuration

```bash
# Rust test
cargo run --bin zerodha-config-test

# Python test  
python nautilus_trader/adapters/zerodha/config_loader.py
```

### 4. Run Tests

```bash
# Safe paper trading tests
python scripts/run_zerodha_tests.py --mode unit

# Live tests (requires real credentials)
python scripts/run_zerodha_tests.py --mode live
```

## 📁 Configuration File Locations

The adapter searches for configuration files in this order:

1. **Current directory**: `./zerodha_credentials.toml`
2. **Config directory**: `./config/zerodha_credentials.toml`
3. **Project root**: `../config/zerodha_credentials.toml`
4. **Home directory**: `~/.zerodha_credentials.toml`
5. **System config**: `~/.config/zerodha_credentials.toml`

## 🔧 Configuration Sections

### API Credentials
```toml
[api]
api_key = "your_api_key"        # From Kite Connect app
api_secret = "your_api_secret"  # From Kite Connect app
access_token = "your_token"     # Generated after login flow
```

### General Settings
```toml
[settings]
trading_mode = "paper"          # "paper" or "live"
sandbox = true                  # Use sandbox/test mode
default_product_type = "MIS"    # MIS, CNC, NRML
default_validity = "DAY"        # DAY, IOC, GTT
```

### Paper Trading
```toml
[paper_trading]
initial_balance = 1000000.0     # Starting balance (₹10 Lakh)
commission_per_trade = 20.0     # Commission per trade (₹20)
execution_delay_ms = 100        # Simulated execution delay
simulate_market_data = true     # Enable market data simulation
```

### Risk Management
```toml
[risk_management]
max_order_value = 100000.0      # Max order value (₹1 Lakh)
max_position_value = 1000000.0  # Max position value (₹10 Lakh)
max_orders_per_minute = 10      # Rate limiting
enable_validation = true        # Enable pre-trade validation
```

### Testing Configuration
```toml
[testing]
test_duration = 300             # Test duration (5 minutes)
test_instruments = [            # Instruments for testing
    "RELIANCE", "TCS", "HDFCBANK"
]
min_data_quality = 85.0         # Minimum acceptable data quality %
max_error_rate = 5.0           # Maximum acceptable error rate %
```

## 🔐 Security Best Practices

### 1. **Never Commit Credentials**
Add to your `.gitignore`:
```gitignore
# Zerodha credentials
zerodha_credentials.toml
**/zerodha_credentials.toml
```

### 2. **Use Paper Trading First**
Always start with paper trading mode:
```toml
[settings]
trading_mode = "paper"
sandbox = true
```

### 3. **Environment Variable Overrides**
You can override any setting with environment variables:
```bash
export ZERODHA_API_KEY="your_key"
export ZERODHA_API_SECRET="your_secret"
export ZERODHA_ACCESS_TOKEN="your_token"
export ZERODHA_TRADING_MODE="paper"
```

### 4. **File Permissions**
Secure your config file:
```bash
chmod 600 zerodha_credentials.toml  # Owner read/write only
```

## 🎮 Trading Modes

### Paper Trading (Recommended for Testing)
```toml
[settings]
trading_mode = "paper"
sandbox = true

[paper_trading]
initial_balance = 1000000.0  # ₹10 Lakh virtual money
```

**Features:**
- ✅ Zero risk - no real money
- ✅ Realistic order execution simulation
- ✅ P&L calculation and tracking
- ✅ Commission and fee simulation
- ✅ Position management

### Live Trading (Production)
```toml
[settings]
trading_mode = "live"
sandbox = false  # Use real API endpoints
```

**Requirements:**
- ⚠️ Real Kite Connect API credentials
- ⚠️ Valid access token
- ⚠️ Sufficient account balance
- ⚠️ Proper risk management settings

## 🛠️ Configuration Tools

### Rust Configuration Test
```bash
cd crates/adapters/zerodha
cargo run --bin zerodha-config-test
```

**Features:**
- 📁 Shows config file search paths
- ✅ Validates configuration
- 🧪 Tests basic functionality
- 📝 Creates example config if missing

### Python Configuration Loader
```bash
python nautilus_trader/adapters/zerodha/config_loader.py
```

**Features:**
- 🐍 Python-compatible config loading
- 🔧 Environment variable integration
- ✅ Configuration validation
- 📋 Display-safe credential showing

### Test Runner Integration
```bash
python scripts/run_zerodha_tests.py --mode unit
```

**Features:**
- 🔍 Automatic config detection
- 🧪 Mode-appropriate validation
- 📊 Comprehensive test reporting

## 🔍 Troubleshooting

### "Configuration not found"
1. Check file exists: `ls -la zerodha_credentials.toml`
2. Check file permissions: `ls -l zerodha_credentials.toml`
3. Validate TOML syntax: Run config test binary

### "Invalid credentials"
1. Verify API key/secret from Kite Connect dashboard
2. Check access token expiry
3. Ensure trading_mode matches your intent

### "Tests failing"
1. Start with paper trading mode
2. Check market hours for live data tests
3. Verify network connectivity

### "Permission denied"
```bash
chmod 600 zerodha_credentials.toml
chown $USER zerodha_credentials.toml
```

## 📋 Example Configurations

### Development Setup
```toml
[api]
api_key = "dev_api_key"
api_secret = "dev_api_secret"

[settings]
trading_mode = "paper"
sandbox = true

[paper_trading]
initial_balance = 100000.0  # ₹1 Lakh for testing

[risk_management]
max_order_value = 10000.0   # ₹10k max orders
enable_validation = true
```

### Production Setup
```toml
[api]
api_key = "prod_api_key"
api_secret = "prod_api_secret"
access_token = "valid_access_token"

[settings]
trading_mode = "live"
sandbox = false

[risk_management]
max_order_value = 500000.0    # ₹5 Lakh max orders
max_position_value = 2000000.0 # ₹20 Lakh max position
max_orders_per_minute = 5      # Conservative rate limit
enable_validation = true

[logging]
log_level = "INFO"
log_to_file = true
log_file = "logs/production.log"
```

## 🎯 Migration from Environment Variables

If you're currently using environment variables, you can migrate:

### Old Way (Environment Variables)
```bash
export ZERODHA_API_KEY="your_key"
export ZERODHA_API_SECRET="your_secret"
export ZERODHA_ACCESS_TOKEN="your_token"
```

### New Way (Configuration File)
1. Create `zerodha_credentials.toml`
2. Add your credentials to the file
3. Set appropriate trading mode
4. Configure risk management settings

The new system will still read environment variables as overrides, so you can migrate gradually.

## 🚀 Advanced Usage

### Programmatic Configuration (Rust)
```rust
use nautilus_zerodha::config::credentials::ZerodhaCredentialsBuilder;

let config = ZerodhaCredentialsBuilder::new()
    .with_api_credentials("key".to_string(), "secret".to_string(), None)
    .with_trading_mode("paper")
    .with_paper_balance(500_000.0)
    .build()?;
```

### Programmatic Configuration (Python)
```python
from nautilus_trader.adapters.zerodha.config_loader import ZerodhaCredentialsBuilder

config = ZerodhaCredentialsBuilder() \
    .with_api_credentials("key", "secret") \
    .with_trading_mode("paper") \
    .with_paper_balance(500_000.0) \
    .build()
```

---

**Remember:** Always start with paper trading mode to test your strategies safely! 🎮

For more help, run the configuration test tools or check the example files.