//! Market data service implementation with Zerodha integration

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};
use tokio_stream::wrappers::ReceiverStream;
use rust_decimal::prelude::ToPrimitive;

use crate::proto::market_data::{
    market_data_service_server::MarketDataService,
    BarList, BarRequest, Bar, InstrumentFilter, InstrumentList, Instrument, MarketTick,
    OptionChain, OptionChainRequest, OptionStrike, OptionData, SubscriptionRequest,
};

use crate::registry::SharedClients;
use nautilus_zerodha::ZerodhaHttpClient;
use nautilus_zerodha::types::ZerodhaInstrument;
use nautilus_zerodha::enums::{Interval, InstrumentType};

/// Market data service implementation with Zerodha support
pub struct MarketDataServiceImpl {
    /// Shared Zerodha clients
    clients: SharedClients,
    /// Cached instruments
    instruments_cache: Arc<RwLock<Vec<ZerodhaInstrument>>>,
}

impl std::fmt::Debug for MarketDataServiceImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MarketDataServiceImpl").finish()
    }
}

impl MarketDataServiceImpl {
    pub fn new(clients: SharedClients) -> Self {
        Self {
            clients,
            instruments_cache: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get the first available client
    async fn get_client(&self) -> Result<Arc<ZerodhaHttpClient>, Status> {
        self.clients.get_any().await
            .ok_or_else(|| Status::unavailable("No broker connected"))
    }

    /// Check if instrument type is an option
    fn is_option(inst_type: &InstrumentType) -> bool {
        matches!(inst_type, InstrumentType::CE | InstrumentType::PE)
    }

    /// Convert Zerodha instrument to proto Instrument
    fn to_proto_instrument(inst: &ZerodhaInstrument) -> Instrument {
        let is_option = Self::is_option(&inst.instrument_type);
        Instrument {
            symbol: inst.tradingsymbol.clone(),
            name: inst.name.clone(),
            exchange: inst.exchange.to_string(),
            instrument_type: inst.instrument_type.to_string(),
            trading_symbol: inst.tradingsymbol.clone(),
            instrument_token: inst.instrument_token as i64,
            lot_size: inst.lot_size as i32,
            tick_size: inst.tick_size.to_f64().unwrap_or(0.0),
            expiry: inst.expiry.map(|d| d.to_string()).unwrap_or_default(),
            strike: inst.strike.and_then(|s| s.to_f64()).unwrap_or(0.0),
            option_type: if is_option { inst.instrument_type.to_string() } else { String::new() },
        }
    }
}

#[tonic::async_trait]
impl MarketDataService for MarketDataServiceImpl {
    async fn get_instruments(
        &self,
        request: Request<InstrumentFilter>,
    ) -> Result<Response<InstrumentList>, Status> {
        let filter = request.into_inner();
        tracing::info!("Searching instruments: query={}, exchange={}", filter.query, filter.exchange);

        let client = self.get_client().await?;

        // Refresh cache if needed
        {
            let mut cache = self.instruments_cache.write().await;
            if cache.is_empty() {
                tracing::info!("Fetching instruments from Zerodha...");
                match client.get_instruments().await {
                    Ok(instruments) => {
                        tracing::info!("Loaded {} instruments", instruments.len());
                        *cache = instruments;
                    }
                    Err(e) => {
                        return Err(Status::internal(format!("Failed to fetch instruments: {}", e)));
                    }
                }
            }
        }

        let cache = self.instruments_cache.read().await;

        // Filter instruments
        let filtered: Vec<Instrument> = cache.iter()
            .filter(|inst| {
                let matches_query = filter.query.is_empty() ||
                    inst.tradingsymbol.to_lowercase().contains(&filter.query.to_lowercase()) ||
                    inst.name.to_lowercase().contains(&filter.query.to_lowercase());

                let matches_exchange = filter.exchange.is_empty() ||
                    inst.exchange.to_string() == filter.exchange;

                let matches_type = filter.instrument_type.is_empty() ||
                    inst.instrument_type.to_string() == filter.instrument_type;

                matches_query && matches_exchange && matches_type
            })
            .take(if filter.limit > 0 { filter.limit as usize } else { 100 })
            .map(Self::to_proto_instrument)
            .collect();

        let total = filtered.len() as i32;

        Ok(Response::new(InstrumentList {
            instruments: filtered,
            total_count: total,
        }))
    }

    type SubscribeStream = ReceiverStream<Result<MarketTick, Status>>;

    async fn subscribe(
        &self,
        request: Request<SubscriptionRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let sub = request.into_inner();
        tracing::info!("Subscribing to {} symbols", sub.symbols.len());

        let _client = self.get_client().await?;
        let (tx, rx) = tokio::sync::mpsc::channel(1024);

        // TODO: Connect to Zerodha WebSocket for live data
        // For now, we'll use polling with get_quotes
        let symbols = sub.symbols;
        let exchange = sub.exchange;
        let clients = self.clients.clone();
        let instruments_cache = Arc::clone(&self.instruments_cache);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));

            // Get instrument tokens for symbols
            let cache = instruments_cache.read().await;
            let tokens: Vec<u32> = cache.iter()
                .filter(|inst| symbols.contains(&inst.tradingsymbol))
                .map(|inst| inst.instrument_token)
                .collect();
            drop(cache);

            if tokens.is_empty() {
                tracing::warn!("No instrument tokens found for symbols: {:?}", symbols);
                return;
            }

            loop {
                interval.tick().await;

                if let Some(client) = clients.get_any().await {
                    match client.get_quotes(&tokens).await {
                        Ok(quotes) => {
                            for (key, quote) in quotes {
                                // Get depth data safely
                                let (bid, ask, bid_qty, ask_qty) = if let Some(ref depth) = quote.depth {
                                    (
                                        depth.buy.first().map(|d| d.price.to_f64().unwrap_or(0.0)).unwrap_or(0.0),
                                        depth.sell.first().map(|d| d.price.to_f64().unwrap_or(0.0)).unwrap_or(0.0),
                                        depth.buy.first().map(|d| d.quantity as i64).unwrap_or(0),
                                        depth.sell.first().map(|d| d.quantity as i64).unwrap_or(0),
                                    )
                                } else {
                                    (0.0, 0.0, 0, 0)
                                };

                                let net_change = quote.net_change.unwrap_or_default();
                                let change_pct = if quote.last_price.is_zero() {
                                    0.0
                                } else {
                                    (net_change / quote.last_price * rust_decimal::Decimal::from(100))
                                        .to_f64()
                                        .unwrap_or(0.0)
                                };

                                let tick = MarketTick {
                                    symbol: key,
                                    exchange: exchange.clone(),
                                    ltp: quote.last_price.to_f64().unwrap_or(0.0),
                                    open: quote.ohlc.open.to_f64().unwrap_or(0.0),
                                    high: quote.ohlc.high.to_f64().unwrap_or(0.0),
                                    low: quote.ohlc.low.to_f64().unwrap_or(0.0),
                                    close: quote.ohlc.close.to_f64().unwrap_or(0.0),
                                    volume: quote.volume as i64,
                                    bid,
                                    ask,
                                    bid_qty,
                                    ask_qty,
                                    change: net_change.to_f64().unwrap_or(0.0),
                                    change_percent: change_pct,
                                    timestamp: quote.timestamp.map(|t| t.timestamp_nanos_opt().unwrap_or(0)).unwrap_or(0),
                                    oi: quote.oi.unwrap_or(0) as i64,
                                };

                                if tx.send(Ok(tick)).await.is_err() {
                                    return; // Client disconnected
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to get quotes: {}", e);
                        }
                    }
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn get_historical_bars(
        &self,
        request: Request<BarRequest>,
    ) -> Result<Response<BarList>, Status> {
        let req = request.into_inner();
        tracing::info!("Fetching historical bars for {} ({})", req.symbol, req.interval);

        let client = self.get_client().await?;

        // Find instrument token
        let cache = self.instruments_cache.read().await;
        let instrument = cache.iter()
            .find(|inst| inst.tradingsymbol == req.symbol)
            .ok_or_else(|| Status::not_found(format!("Instrument not found: {}", req.symbol)))?;

        let token = instrument.instrument_token;
        drop(cache);

        // Parse interval
        let interval = match req.interval.as_str() {
            "minute" | "1m" => Interval::Minute,
            "3minute" | "3m" => Interval::ThreeMinute,
            "5minute" | "5m" => Interval::FiveMinute,
            "10minute" | "10m" => Interval::TenMinute,
            "15minute" | "15m" => Interval::FifteenMinute,
            "30minute" | "30m" => Interval::ThirtyMinute,
            "60minute" | "hour" | "1h" => Interval::Hourly,
            "day" | "1d" => Interval::Daily,
            _ => return Err(Status::invalid_argument(format!("Invalid interval: {}", req.interval))),
        };

        // Parse timestamps
        let from = chrono::DateTime::from_timestamp(req.from_timestamp, 0)
            .ok_or_else(|| Status::invalid_argument("Invalid from_timestamp"))?;
        let to = chrono::DateTime::from_timestamp(req.to_timestamp, 0)
            .ok_or_else(|| Status::invalid_argument("Invalid to_timestamp"))?;

        // Fetch data
        match client.get_historical_data(token, interval, from, to, None, None).await {
            Ok(candles) => {
                let bars: Vec<Bar> = candles.iter()
                    .map(|c| Bar {
                        timestamp: c.timestamp.timestamp_millis(),
                        open: c.open.to_f64().unwrap_or(0.0),
                        high: c.high.to_f64().unwrap_or(0.0),
                        low: c.low.to_f64().unwrap_or(0.0),
                        close: c.close.to_f64().unwrap_or(0.0),
                        volume: c.volume as i64,
                    })
                    .collect();

                Ok(Response::new(BarList {
                    symbol: req.symbol,
                    interval: req.interval,
                    bars,
                }))
            }
            Err(e) => {
                Err(Status::internal(format!("Failed to fetch historical data: {}", e)))
            }
        }
    }

    async fn get_option_chain(
        &self,
        request: Request<OptionChainRequest>,
    ) -> Result<Response<OptionChain>, Status> {
        let req = request.into_inner();
        tracing::info!("Fetching option chain for {}", req.underlying);

        let client = self.get_client().await?;
        let cache = self.instruments_cache.read().await;

        // Find all options for the underlying
        let options: Vec<&ZerodhaInstrument> = cache.iter()
            .filter(|inst| {
                inst.name == req.underlying &&
                Self::is_option(&inst.instrument_type) &&
                (req.expiry.is_empty() || inst.expiry.map(|d| d.to_string()).unwrap_or_default() == req.expiry)
            })
            .collect();

        if options.is_empty() {
            return Err(Status::not_found(format!("No options found for {}", req.underlying)));
        }

        // Get quotes for options
        let tokens: Vec<u32> = options.iter().map(|o| o.instrument_token).collect();

        let quotes = client.get_quotes(&tokens).await
            .map_err(|e| Status::internal(format!("Failed to get quotes: {}", e)))?;

        // Group by strike
        let mut strikes_map: HashMap<String, OptionStrike> = HashMap::new();

        for opt in options {
            let strike_key = opt.strike.map(|s| s.to_string()).unwrap_or_default();
            let quote = quotes.get(&format!("{}:{}", opt.exchange, opt.tradingsymbol));

            let entry = strikes_map.entry(strike_key.clone()).or_insert_with(|| {
                OptionStrike {
                    strike_price: opt.strike.and_then(|s| s.to_f64()).unwrap_or(0.0),
                    call: None,
                    put: None,
                }
            });

            // Get depth data safely
            let (bid, ask) = if let Some(q) = quote {
                if let Some(ref depth) = q.depth {
                    (
                        depth.buy.first().map(|d| d.price.to_f64().unwrap_or(0.0)).unwrap_or(0.0),
                        depth.sell.first().map(|d| d.price.to_f64().unwrap_or(0.0)).unwrap_or(0.0),
                    )
                } else {
                    (0.0, 0.0)
                }
            } else {
                (0.0, 0.0)
            };

            let option_data = OptionData {
                symbol: opt.tradingsymbol.clone(),
                ltp: quote.map(|q| q.last_price.to_f64().unwrap_or(0.0)).unwrap_or(0.0),
                bid,
                ask,
                volume: quote.map(|q| q.volume as i64).unwrap_or(0),
                oi: quote.map(|q| q.oi.unwrap_or(0) as i64).unwrap_or(0),
                iv: 0.0, // TODO: Calculate IV
                delta: 0.0, // TODO: Calculate Greeks
                gamma: 0.0,
                theta: 0.0,
                vega: 0.0,
            };

            if matches!(opt.instrument_type, InstrumentType::CE) {
                entry.call = Some(option_data);
            } else {
                entry.put = Some(option_data);
            }
        }

        // Get spot price
        let underlying_inst = cache.iter()
            .find(|inst| inst.tradingsymbol == req.underlying || inst.name == req.underlying);

        let spot_price = if let Some(inst) = underlying_inst {
            client.get_quotes(&[inst.instrument_token]).await
                .ok()
                .and_then(|q| q.values().next().map(|v| v.last_price.to_f64().unwrap_or(0.0)))
                .unwrap_or(0.0)
        } else {
            0.0
        };

        Ok(Response::new(OptionChain {
            underlying: req.underlying,
            expiry: req.expiry,
            spot_price,
            strikes: strikes_map.into_values().collect(),
        }))
    }
}
