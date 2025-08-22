# NautilusTrader - Claude Documentation

## Overview
NautilusTrader is a high-performance, production-grade algorithmic trading platform designed for quantitative traders. It provides the ability to backtest portfolios of automated trading strategies on historical data with an event-driven engine, and deploy those same strategies live with no code changes.

## Key Features
- **Hybrid Architecture**: Core written in Rust with Python bindings via Cython and PyO3
- **Event-Driven**: Real-time backtesting and live trading with identical strategy implementations
- **Multi-Asset Support**: FX, Equities, Futures, Options, Crypto, DeFi, and Betting markets
- **High Performance**: Rust-powered type safety, thread safety, and async networking
- **Modular Design**: Custom components, adapters, indicators, and execution algorithms

## Entry Points

### Main Package Entry Points
- **Main Package**: `nautilus_trader/nautilus_trader/__init__.py` - Core package initialization
- **Backtest CLI**: `nautilus_trader/nautilus_trader/backtest/__main__.py` - Command-line backtesting interface
- **Live Trading CLI**: `nautilus_trader/nautilus_trader/live/__main__.py` - Command-line live trading interface

### Build System
- **Build Script**: `nautilus_trader/build.py` - Custom build system for Rust/Python hybrid
- **Python Config**: `nautilus_trader/pyproject.toml` - Python package configuration
- **Rust Config**: `nautilus_trader/Cargo.toml` - Rust workspace configuration
- **Make Automation**: `nautilus_trader/Makefile` - Development automation tasks

### Example Entry Points
- **Backtest Examples**: `nautilus_trader/examples/backtest/` - Various backtesting strategy examples
- **Live Examples**: `nautilus_trader/examples/live/` - Live trading examples for different exchanges
- **Sandbox Examples**: `nautilus_trader/examples/sandbox/` - Testing/development sandboxes

## Architecture Components

### Core Rust Crates (in `crates/`)
1. **Core Infrastructure**:
   - `core/` - Fundamental types, time handling, UUID, math utilities
   - `common/` - Common components, actors, messaging, runtime
   - `model/` - Trading domain models (instruments, orders, positions, etc.)
   
2. **Data & Execution**:
   - `data/` - Data aggregation, client interfaces, engine
   - `execution/` - Order execution, matching engine, algorithms
   - `backtest/` - Backtesting engine and simulation components
   - `live/` - Live trading infrastructure and node management
   
3. **Supporting Systems**:
   - `portfolio/` - Portfolio management and accounting
   - `risk/` - Risk management and position sizing
   - `persistence/` - Data storage and retrieval (Parquet, databases)
   - `indicators/` - Technical analysis indicators
   - `network/` - HTTP/WebSocket networking with async support
   - `cryptography/` - Signing, TLS, security providers

4. **Exchange Adapters** (in `crates/adapters/`):
   - `binance/`, `bybit/`, `coinbase_intx/`, `databento/`, `okx/`, etc.
   - Each adapter translates exchange-specific APIs to unified interfaces

### Python Components (in `nautilus_trader/`)
1. **Core Modules**:
   - `core/` - Python bindings to Rust core functionality
   - `common/` - Common components, actors, messaging
   - `model/` - Trading domain models with Cython extensions
   
2. **Engines & Systems**:
   - `backtest/` - Backtesting engine with configuration
   - `live/` - Live trading node and execution
   - `data/` - Data client, aggregation, message handling
   - `execution/` - Execution engines and order management
   
3. **Strategy & Analysis**:
   - `trading/` - Strategy base classes and trading logic
   - `indicators/` - Technical indicators (Cython-optimized)
   - `analysis/` - Performance analysis and statistics
   - `portfolio/` - Portfolio management
   - `risk/` - Risk management engines

## Usage Patterns

### 1. Backtesting Workflow
```python
# Create backtest engine
from nautilus_trader.backtest.engine import BacktestEngine
engine = BacktestEngine(config=config)

# Add venue, instruments, and data
engine.add_venue(venue, account_type, starting_balances)
engine.add_instrument(instrument)
engine.add_data(historical_data)

# Add strategy and run
engine.add_strategy(strategy)
engine.run()
```

### 2. Live Trading Setup
```python
# Create live trading node
from nautilus_trader.live.node import TradingNode
node = TradingNode(config=config)
node.build()
node.run()
```

### 3. Strategy Development
- Inherit from `Strategy` base class in `nautilus_trader.trading.strategy`
- Implement `on_start()`, `on_data()`, `on_event()` methods
- Use built-in indicators or create custom ones
- Handle orders through execution engine

### 4. Custom Indicators
- Inherit from indicator base classes in `nautilus_trader.indicators`
- Implement in Python or Cython for performance
- Use `update()` method for streaming data processing

### 5. Data Integration
- Use data wranglers for format conversion (CSV, Parquet, etc.)
- Connect to data providers via adapters
- Support for tick, bar, order book, and custom data types

## Development Commands

### Building
```bash
make install          # Install release version with all dependencies
make install-debug    # Install debug version for development
make build           # Build Rust/Cython extensions in release mode
make build-debug     # Build in debug mode
make clean           # Clean build artifacts
```

### Testing
```bash
make pytest          # Run Python tests
make cargo-test      # Run Rust tests with cargo-nextest
make test-performance # Run performance benchmarks
make test-examples   # Test all example scripts
```

### Development
```bash
make pre-commit      # Run pre-commit checks
make ruff           # Code formatting and linting
make docs           # Build documentation
```

## Configuration

### Precision Modes
- **High-precision** (128-bit): Up to 16 decimal places (Linux/macOS)
- **Standard-precision** (64-bit): Up to 9 decimal places (Windows default)

### Environment Variables
- `BUILD_MODE`: "release" or "debug" 
- `HIGH_PRECISION`: Enable 128-bit precision mode
- `RUSTUP_TOOLCHAIN`: Rust toolchain to use
- `PARALLEL_BUILD`: Enable parallel compilation

### Dependencies
- **Rust**: 1.89.0+ (for core components)
- **Python**: 3.11-3.13
- **Cython**: For Python/Rust FFI bindings
- **Optional**: Redis (for caching), PostgreSQL (for persistence)

## Testing Strategy
- **Unit Tests**: `tests/unit_tests/` - Component-level testing
- **Integration Tests**: `tests/integration_tests/` - Cross-component testing  
- **Performance Tests**: `tests/performance_tests/` - Benchmarking
- **Memory Leak Tests**: `tests/mem_leak_tests/` - Memory profiling
- **Acceptance Tests**: `tests/acceptance_tests/` - End-to-end validation

## Key Integrations
The platform supports numerous exchanges and data providers through modular adapters:
- **Crypto**: Binance, Bybit, Coinbase, OKX, etc.
- **Traditional**: Interactive Brokers
- **Data Providers**: Databento, Tardis
- **Betting**: Betfair, Polymarket
- **DeFi**: dYdX, Hyperliquid

Each integration follows the same adapter pattern, making the platform highly extensible for new venues and data sources.