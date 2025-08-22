# Indian Options Trading Implementation Guide

## Options Trading Specifics for Indian Markets

### Market Structure
- **Primary Exchange**: NSE (National Stock Exchange) - NFO segment
- **Secondary Exchange**: BSE (Bombay Stock Exchange) - BFO segment
- **Major Indices**: Nifty 50, Bank Nifty, Fin Nifty, Sensex
- **Trading Hours**: 9:15 AM to 3:30 PM IST (Monday to Friday)

### Contract Specifications

#### Index Options (Most Liquid)
1. **Nifty 50**
   - Lot Size: 50 units
   - Strike Interval: 50 points
   - Expiry: Weekly (Thursday), Monthly (last Thursday)
   - Tick Size: ₹0.05

2. **Bank Nifty**  
   - Lot Size: 25 units
   - Strike Interval: 100 points
   - Expiry: Weekly (Wednesday), Monthly
   - Tick Size: ₹0.05

3. **Fin Nifty**
   - Lot Size: 40 units  
   - Strike Interval: 50 points
   - Expiry: Weekly (Tuesday), Monthly
   - Tick Size: ₹0.05

#### Stock Options
- **Individual Stocks**: Top 100+ stocks have options
- **Lot Size**: Varies (typically 250-4000 shares)
- **Expiry**: Monthly (last Thursday)
- **Strike Intervals**: Based on stock price ranges

### Margin Requirements

#### SPAN Margins (Portfolio Risk)
```python
# Example SPAN calculation components
span_margin = {
    "span_value": calculated_span,
    "exposure_margin": min(3% of notional, span_value * 0.3),
    "additional_margin": volatility_based_addon,
    "delivery_margin": 20%_for_physical_delivery
}

total_margin_required = span_value + exposure_margin + additional_margin
```

#### Premium Collection (Options Selling)
- **Short Options**: Full SPAN + Exposure margin required
- **Covered Positions**: Reduced margin for hedge positions
- **Calendar Spreads**: Benefit from margin offsetting

## Key Trading Strategies for Implementation

### 1. Directional Strategies
```python
# Long Call/Put
class DirectionalOption(Strategy):
    def on_start(self):
        # Select ATM/OTM options based on view
        if self.market_view == "bullish":
            self.target_instrument = self.get_call_option()
        else:
            self.target_instrument = self.get_put_option()
    
    def on_bar(self, bar):
        # Entry/exit logic based on technical indicators
        if self.should_enter():
            self.buy_option()
        elif self.should_exit():
            self.close_position()
```

### 2. Volatility Strategies
```python
# Straddle/Strangle
class VolatilityStrategy(Strategy):
    def create_straddle(self, strike_price):
        # Buy both call and put at same strike
        call_order = self.order_factory.market(
            instrument_id=self.call_instrument,
            order_side=OrderSide.BUY,
            quantity=self.lot_size
        )
        put_order = self.order_factory.market(
            instrument_id=self.put_instrument,
            order_side=OrderSide.BUY,
            quantity=self.lot_size
        )
        return [call_order, put_order]
```

### 3. Income Generation (Theta Strategies)
```python
# Iron Condor
class IronCondor(Strategy):
    def setup_iron_condor(self):
        # Sell call spread and put spread
        positions = [
            ("short_call", self.otm_call_high),
            ("long_call", self.otm_call_higher),
            ("short_put", self.otm_put_low),
            ("long_put", self.otm_put_lower)
        ]
        return self.execute_spread(positions)
```

## NautilusTrader Integration Patterns

### Data Client Implementation
```python
class ZerodhaDataClient(LiveDataClient):
    """
    Provides live market data feed from Zerodha Kite Connect.
    """
    
    def __init__(self, config: ZerodhaConfig):
        self._config = config
        self._client = KiteConnect(api_key=config.api_key)
        self._ws = KiteTicker(api_key=config.api_key, access_token=config.access_token)
        
    async def subscribe_quote_ticks(self, instrument_id: InstrumentId):
        """Subscribe to real-time quote updates."""
        token = self._get_instrument_token(instrument_id)
        self._ws.subscribe([token])
        
    def _on_tick(self, ws, ticks):
        """Process incoming tick data."""
        for tick in ticks:
            quote_tick = self._parse_quote_tick(tick)
            self._handle_data(quote_tick)
```

### Execution Client Implementation  
```python
class ZerodhaExecutionClient(LiveExecClient):
    """
    Provides order execution via Zerodha Kite Connect.
    """
    
    async def submit_order(self, command: SubmitOrder):
        """Submit order to Zerodha."""
        try:
            order_params = self._build_order_params(command.order)
            order_id = await self._client.place_order(**order_params)
            
            # Generate and handle events
            event = OrderSubmitted(
                trader_id=command.trader_id,
                strategy_id=command.strategy_id,
                instrument_id=command.order.instrument_id,
                client_order_id=command.order.client_order_id,
                event_id=UUID4(),
                ts_event=self._clock.timestamp_ns()
            )
            self._send_order_event(event)
            
        except Exception as e:
            self._handle_order_error(command, e)
```

### Options-Specific Features
```python
class NSEOptionsProvider(InstrumentProvider):
    """
    Provides NSE options instruments with real-time chain data.
    """
    
    async def load_options_chain(self, underlying: str, expiry: date) -> list[Instrument]:
        """Load complete options chain for underlying."""
        contracts = await self._fetch_options_contracts(underlying, expiry)
        instruments = []
        
        for contract in contracts:
            instrument = self._create_options_instrument(contract)
            instruments.append(instrument)
            
        return instruments
    
    def _create_options_instrument(self, contract) -> OptionsContract:
        """Create NautilusTrader options instrument from NSE contract."""
        return OptionsContract(
            instrument_id=InstrumentId.from_str(f"{contract.symbol}.NSE"),
            raw_symbol=Symbol(contract.symbol),
            asset_class=AssetClass.INDEX if contract.is_index else AssetClass.EQUITY,
            instrument_class=InstrumentClass.OPTION,
            underlying=contract.underlying,
            option_kind=OptionKind.CALL if contract.option_type == "CE" else OptionKind.PUT,
            strike_price=Price(contract.strike_price, precision=2),
            expiry_date=contract.expiry_date,
            currency=INR,
            price_precision=2,
            price_increment=Price(0.05, precision=2),
            lot_size=Quantity(contract.lot_size, precision=0),
            max_quantity=None,
            min_quantity=Quantity(contract.lot_size, precision=0),
            ts_event=self._clock.timestamp_ns(),
            ts_init=self._clock.timestamp_ns()
        )
```

## Risk Management for Options

### Position Risk Monitoring
```python
class OptionsRiskManager:
    """Risk management specific to options trading."""
    
    def calculate_portfolio_greeks(self, portfolio):
        """Calculate aggregate Greeks for entire portfolio."""
        total_delta = sum(pos.delta * pos.quantity for pos in portfolio.positions)
        total_gamma = sum(pos.gamma * pos.quantity for pos in portfolio.positions)
        total_theta = sum(pos.theta * pos.quantity for pos in portfolio.positions)
        total_vega = sum(pos.vega * pos.quantity for pos in portfolio.positions)
        
        return {
            'delta': total_delta,
            'gamma': total_gamma, 
            'theta': total_theta,
            'vega': total_vega
        }
    
    def check_position_limits(self, order):
        """Validate against SEBI position limits."""
        if order.instrument_id.asset_class == AssetClass.INDEX:
            # Index options: Market-wide position limit
            return self._check_index_limits(order)
        else:
            # Stock options: Per-stock position limit
            return self._check_stock_limits(order)
```

### Margin Optimization
```python
class MarginOptimizer:
    """Optimize strategies for margin efficiency."""
    
    def suggest_hedge(self, naked_position):
        """Suggest hedge to reduce margin requirement."""
        if naked_position.side == PositionSide.SHORT:
            # For short options, suggest protective long
            hedge_strike = self._calculate_protective_strike(naked_position)
            return self._create_hedge_order(hedge_strike)
    
    def calculate_strategy_margin(self, legs):
        """Calculate total margin for multi-leg strategy."""
        # Use SPAN methodology for margin calculation
        return self._span_margin_calculator(legs)
```

## Market Data Enhancements

### Options Chain Data Structure
```python
@dataclass  
class OptionsChainData:
    """Complete options chain snapshot."""
    underlying_price: Price
    timestamp: int
    expiry_date: date
    calls: dict[Price, OptionQuote]  # Strike -> Quote
    puts: dict[Price, OptionQuote]   # Strike -> Quote
    
    def get_atm_strike(self) -> Price:
        """Get At-The-Money strike price."""
        strikes = list(self.calls.keys())
        return min(strikes, key=lambda x: abs(x - self.underlying_price))
    
    def get_option_by_delta(self, target_delta: float, option_type: OptionKind) -> InstrumentId:
        """Find option closest to target delta."""
        options = self.calls if option_type == OptionKind.CALL else self.puts
        return min(options.items(), key=lambda x: abs(x[1].delta - target_delta))
```

### Live Greeks Calculation
```python
class LiveGreeksCalculator:
    """Calculate real-time Greeks for options."""
    
    def calculate_greeks(self, option_data, underlying_price, risk_free_rate, implied_vol):
        """Calculate Black-Scholes Greeks."""
        # Implementation of Black-Scholes formulas
        # or integration with external libraries like QuantLib
        
        greeks = BlackScholesGreeks(
            underlying_price=underlying_price,
            strike_price=option_data.strike_price,
            time_to_expiry=option_data.time_to_expiry,
            risk_free_rate=risk_free_rate,
            volatility=implied_vol,
            option_type=option_data.option_kind
        )
        
        return {
            'delta': greeks.delta(),
            'gamma': greeks.gamma(),
            'theta': greeks.theta(),
            'vega': greeks.vega(),
            'rho': greeks.rho()
        }
```

## Backtesting Considerations

### Historical Options Data
```python
class NSEOptionsDataProvider:
    """Provider for historical NSE options data."""
    
    def load_historical_options_data(self, symbol: str, date_range: tuple) -> list[OptionsChainData]:
        """Load historical options chains for backtesting."""
        # Integration with data providers like:
        # - TrueData API
        # - Zerodha Historical API  
        # - NSE Historical Data
        pass
    
    def simulate_options_expiry(self, positions, expiry_date):
        """Simulate options expiry and settlement."""
        for position in positions:
            if position.expiry_date == expiry_date:
                settlement_value = self._calculate_settlement_value(position)
                self._process_expiry_settlement(position, settlement_value)
```

This implementation guide provides the foundation for building a comprehensive options trading system tailored to Indian markets within the NautilusTrader framework.