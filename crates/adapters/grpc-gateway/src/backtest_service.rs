//! Backtest Service Implementation
//!
//! Provides historical data fetching and backtest execution capabilities.

use crate::proto::backtest::backtest_service_server::BacktestService;
use crate::proto::backtest::*;
use anyhow::Result;
use chrono::Datelike;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

/// Stored backtest results
#[derive(Clone, Debug)]
pub struct StoredBacktest {
    pub results: BacktestResults,
    pub created_at: i64,
}

/// Backtest service implementation
pub struct BacktestServiceImpl {
    /// Stored backtest results
    backtests: Arc<RwLock<HashMap<String, StoredBacktest>>>,
    /// HTTP client for external API calls
    http_client: reqwest::Client,
}

impl BacktestServiceImpl {
    pub fn new() -> Self {
        Self {
            backtests: Arc::new(RwLock::new(HashMap::new())),
            http_client: reqwest::Client::new(),
        }
    }

    /// Fetch historical data from Zerodha Kite API
    async fn fetch_zerodha_historical(
        &self,
        instrument_token: &str,
        interval: &str,
        from_date: &str,
        to_date: &str,
        api_key: &str,
        access_token: &str,
    ) -> Result<Vec<Bar>> {
        let url = format!(
            "https://api.kite.trade/instruments/historical/{}/{}",
            instrument_token, interval
        );

        let response = self
            .http_client
            .get(&url)
            .header("X-Kite-Version", "3")
            .header("Authorization", format!("token {}:{}", api_key, access_token))
            .query(&[("from", from_date), ("to", to_date)])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Zerodha API error: {}", error_text);
        }

        let data: serde_json::Value = response.json().await?;
        let candles = data["data"]["candles"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Invalid response format"))?;

        let bars: Vec<Bar> = candles
            .iter()
            .filter_map(|candle| {
                let arr = candle.as_array()?;
                if arr.len() >= 6 {
                    // Parse timestamp from ISO string
                    let timestamp_str = arr[0].as_str()?;
                    let timestamp = chrono::DateTime::parse_from_rfc3339(timestamp_str)
                        .ok()?
                        .timestamp_millis();

                    Some(Bar {
                        timestamp,
                        open: arr[1].as_f64()?,
                        high: arr[2].as_f64()?,
                        low: arr[3].as_f64()?,
                        close: arr[4].as_f64()?,
                        volume: arr[5].as_i64()?,
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(bars)
    }

    /// Generate mock historical data for testing
    fn generate_mock_data(&self, symbol: &str, from_date: &str, to_date: &str) -> Vec<Bar> {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        // Parse dates
        let from = chrono::NaiveDate::parse_from_str(from_date, "%Y-%m-%d")
            .unwrap_or(chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
        let to = chrono::NaiveDate::parse_from_str(to_date, "%Y-%m-%d")
            .unwrap_or(chrono::NaiveDate::from_ymd_opt(2024, 12, 31).unwrap());

        let mut bars = Vec::new();
        let mut current_date = from;
        let base_price = match symbol.to_uppercase().as_str() {
            "RELIANCE" => 2500.0,
            "TCS" => 3800.0,
            "INFY" => 1500.0,
            "HDFCBANK" => 1600.0,
            "ICICIBANK" => 1100.0,
            _ => 1000.0,
        };

        let mut price = base_price;

        while current_date <= to {
            // Skip weekends
            if current_date.weekday().number_from_monday() <= 5 {
                let change_percent: f64 = rng.random_range(-0.03..0.03);
                let open: f64 = price;
                let close: f64 = price * (1.0 + change_percent);
                let high: f64 = open.max(close) * (1.0 + rng.random_range(0.0..0.02));
                let low: f64 = open.min(close) * (1.0 - rng.random_range(0.0..0.02));
                let volume: i64 = rng.random_range(100000..1000000);

                let timestamp = current_date
                    .and_hms_opt(9, 15, 0)
                    .unwrap()
                    .and_utc()
                    .timestamp_millis();

                bars.push(Bar {
                    timestamp,
                    open,
                    high,
                    low,
                    close,
                    volume,
                });

                price = close;
            }

            current_date = current_date.succ_opt().unwrap_or(current_date);
        }

        bars
    }

    /// Run a backtest - currently uses mock implementation
    /// TODO: Add Python subprocess integration for real strategy execution
    async fn run_python_backtest(
        &self,
        request: &RunBacktestRequest,
        bars: &[Bar],
        tx: tokio::sync::mpsc::Sender<Result<BacktestProgress, Status>>,
    ) -> Result<BacktestResults> {
        let backtest_id = Uuid::new_v4().to_string();

        // Send initializing status
        let _ = tx
            .send(Ok(BacktestProgress {
                status: "initializing".to_string(),
                progress_percent: 0,
                message: "Preparing backtest environment...".to_string(),
                current_equity: request.initial_capital,
                trades_completed: 0,
                results: None,
            }))
            .await;

        // Send loading data status
        let _ = tx
            .send(Ok(BacktestProgress {
                status: "loading_data".to_string(),
                progress_percent: 10,
                message: format!("Loading {} bars of historical data...", bars.len()),
                current_equity: request.initial_capital,
                trades_completed: 0,
                results: None,
            }))
            .await;

        // Run mock backtest (simulates a moving average crossover strategy)
        self.run_mock_backtest(request, bars, &backtest_id, tx).await
    }

    /// Run a mock backtest for testing when Python script is not available
    async fn run_mock_backtest(
        &self,
        request: &RunBacktestRequest,
        bars: &[Bar],
        backtest_id: &str,
        tx: tokio::sync::mpsc::Sender<Result<BacktestProgress, Status>>,
    ) -> Result<BacktestResults> {
        let total_bars = bars.len();
        let mut equity = request.initial_capital;
        let mut trades: Vec<Trade> = Vec::new();
        let mut signals: Vec<Signal> = Vec::new();
        let mut equity_curve: Vec<EquityPoint> = Vec::new();
        let mut max_equity = equity;
        let mut max_drawdown = 0.0;

        // Simple moving average crossover simulation
        let short_period = 10;
        let long_period = 20;
        let mut position: Option<(f64, i64, i32)> = None; // (entry_price, entry_time, quantity)

        for (i, bar) in bars.iter().enumerate() {
            // Calculate progress
            let progress = ((i as f64 / total_bars as f64) * 80.0) as i32 + 10;

            // Update progress every 10%
            if i % (total_bars / 10).max(1) == 0 {
                let _ = tx
                    .send(Ok(BacktestProgress {
                        status: "running".to_string(),
                        progress_percent: progress,
                        message: format!("Processing bar {}/{}", i + 1, total_bars),
                        current_equity: equity,
                        trades_completed: trades.len() as i32,
                        results: None,
                    }))
                    .await;
            }

            // Skip if not enough bars for long MA (need long_period + 1 for prev_long_ma calculation)
            if i <= long_period {
                equity_curve.push(EquityPoint {
                    timestamp: bar.timestamp,
                    equity,
                    drawdown: 0.0,
                });
                continue;
            }

            // Calculate simple moving averages
            let short_ma: f64 = bars[i - short_period..i].iter().map(|b| b.close).sum::<f64>() / short_period as f64;
            let long_ma: f64 = bars[i - long_period..i].iter().map(|b| b.close).sum::<f64>() / long_period as f64;
            let prev_short_ma: f64 = bars[i - short_period - 1..i - 1].iter().map(|b| b.close).sum::<f64>() / short_period as f64;
            let prev_long_ma: f64 = bars[i - long_period - 1..i - 1].iter().map(|b| b.close).sum::<f64>() / long_period as f64;

            // Crossover detection
            let bullish_cross = short_ma > long_ma && prev_short_ma <= prev_long_ma;
            let bearish_cross = short_ma < long_ma && prev_short_ma >= prev_long_ma;

            // Trading logic
            if bullish_cross && position.is_none() {
                // Buy signal
                let quantity = ((equity * request.position_size / 100.0) / bar.close) as i32;
                if quantity > 0 {
                    position = Some((bar.close, bar.timestamp, quantity));
                    signals.push(Signal {
                        timestamp: bar.timestamp,
                        symbol: request.symbols.first().unwrap_or(&"UNKNOWN".to_string()).clone(),
                        r#type: "BUY".to_string(),
                        price: bar.close,
                        reason: "SMA Bullish Crossover".to_string(),
                    });
                }
            } else if bearish_cross && position.is_some() {
                // Sell signal - close position
                if let Some((entry_price, entry_time, quantity)) = position.take() {
                    let exit_price = bar.close;
                    let commission = (entry_price + exit_price) * quantity as f64 * request.commission_rate / 100.0;
                    let slippage = (entry_price + exit_price) * quantity as f64 * request.slippage_rate / 100.0;
                    let pnl = (exit_price - entry_price) * quantity as f64 - commission - slippage;
                    let pnl_percent = (pnl / (entry_price * quantity as f64)) * 100.0;

                    equity += pnl;

                    trades.push(Trade {
                        trade_id: Uuid::new_v4().to_string(),
                        symbol: request.symbols.first().unwrap_or(&"UNKNOWN".to_string()).clone(),
                        side: "BUY".to_string(),
                        entry_time,
                        exit_time: bar.timestamp,
                        entry_price,
                        exit_price,
                        quantity,
                        pnl,
                        pnl_percent,
                        commission,
                        exit_reason: "signal".to_string(),
                    });

                    signals.push(Signal {
                        timestamp: bar.timestamp,
                        symbol: request.symbols.first().unwrap_or(&"UNKNOWN".to_string()).clone(),
                        r#type: "SELL".to_string(),
                        price: bar.close,
                        reason: "SMA Bearish Crossover".to_string(),
                    });
                }
            }

            // Update max equity and drawdown
            if equity > max_equity {
                max_equity = equity;
            }
            let drawdown = (max_equity - equity) / max_equity * 100.0;
            if drawdown > max_drawdown {
                max_drawdown = drawdown;
            }

            equity_curve.push(EquityPoint {
                timestamp: bar.timestamp,
                equity,
                drawdown,
            });
        }

        // Close any open position at the end
        if let Some((entry_price, entry_time, quantity)) = position {
            let exit_price = bars.last().map(|b| b.close).unwrap_or(entry_price);
            let commission = (entry_price + exit_price) * quantity as f64 * request.commission_rate / 100.0;
            let slippage = (entry_price + exit_price) * quantity as f64 * request.slippage_rate / 100.0;
            let pnl = (exit_price - entry_price) * quantity as f64 - commission - slippage;
            let pnl_percent = (pnl / (entry_price * quantity as f64)) * 100.0;

            equity += pnl;

            trades.push(Trade {
                trade_id: Uuid::new_v4().to_string(),
                symbol: request.symbols.first().unwrap_or(&"UNKNOWN".to_string()).clone(),
                side: "BUY".to_string(),
                entry_time,
                exit_time: bars.last().map(|b| b.timestamp).unwrap_or(entry_time),
                entry_price,
                exit_price,
                quantity,
                pnl,
                pnl_percent,
                commission,
                exit_reason: "end_of_data".to_string(),
            });
        }

        // Calculate performance metrics
        let total_return = equity - request.initial_capital;
        let total_return_percent = (total_return / request.initial_capital) * 100.0;
        let winning_trades = trades.iter().filter(|t| t.pnl > 0.0).count() as i32;
        let losing_trades = trades.iter().filter(|t| t.pnl <= 0.0).count() as i32;
        let total_trades = trades.len() as i32;
        let win_rate = if total_trades > 0 {
            winning_trades as f64 / total_trades as f64 * 100.0
        } else {
            0.0
        };

        let avg_win = if winning_trades > 0 {
            trades.iter().filter(|t| t.pnl > 0.0).map(|t| t.pnl).sum::<f64>() / winning_trades as f64
        } else {
            0.0
        };

        let avg_loss = if losing_trades > 0 {
            trades.iter().filter(|t| t.pnl <= 0.0).map(|t| t.pnl.abs()).sum::<f64>() / losing_trades as f64
        } else {
            0.0
        };

        let gross_profit: f64 = trades.iter().filter(|t| t.pnl > 0.0).map(|t| t.pnl).sum();
        let gross_loss: f64 = trades.iter().filter(|t| t.pnl <= 0.0).map(|t| t.pnl.abs()).sum();
        let profit_factor = if gross_loss > 0.0 {
            gross_profit / gross_loss
        } else if gross_profit > 0.0 {
            f64::INFINITY
        } else {
            0.0
        };

        // Calculate Sharpe ratio (simplified)
        let returns: Vec<f64> = equity_curve
            .windows(2)
            .map(|w| (w[1].equity - w[0].equity) / w[0].equity)
            .collect();
        let avg_return = if !returns.is_empty() {
            returns.iter().sum::<f64>() / returns.len() as f64
        } else {
            0.0
        };
        let std_return = if returns.len() > 1 {
            let variance: f64 = returns.iter().map(|r| (r - avg_return).powi(2)).sum::<f64>() / (returns.len() - 1) as f64;
            variance.sqrt()
        } else {
            0.0
        };
        let sharpe_ratio = if std_return > 0.0 {
            (avg_return * 252.0_f64.sqrt()) / std_return
        } else {
            0.0
        };

        // Calculate Sortino ratio (downside deviation)
        let downside_returns: Vec<f64> = returns.iter().filter(|&&r| r < 0.0).copied().collect();
        let downside_std = if downside_returns.len() > 1 {
            let variance: f64 = downside_returns.iter().map(|r| r.powi(2)).sum::<f64>() / downside_returns.len() as f64;
            variance.sqrt()
        } else {
            0.0
        };
        let sortino_ratio = if downside_std > 0.0 {
            (avg_return * 252.0_f64.sqrt()) / downside_std
        } else {
            0.0
        };

        // Calculate annualized return
        let days = if !bars.is_empty() {
            (bars.last().unwrap().timestamp - bars.first().unwrap().timestamp) as f64 / (1000.0 * 60.0 * 60.0 * 24.0)
        } else {
            1.0
        };
        let annualized_return = ((1.0 + total_return_percent / 100.0).powf(365.0 / days) - 1.0) * 100.0;

        let start_time = bars.first().map(|b| b.timestamp).unwrap_or(0);
        let end_time = bars.last().map(|b| b.timestamp).unwrap_or(0);

        let results = BacktestResults {
            backtest_id: backtest_id.to_string(),
            strategy_name: request.strategy_name.clone(),
            start_time,
            end_time,
            total_return,
            total_return_percent,
            annualized_return,
            sharpe_ratio,
            sortino_ratio,
            max_drawdown: max_drawdown * request.initial_capital / 100.0,
            max_drawdown_percent: max_drawdown,
            win_rate,
            profit_factor,
            avg_win,
            avg_loss,
            total_trades,
            winning_trades,
            losing_trades,
            equity_curve,
            trades,
            signals,
        };

        Ok(results)
    }

    /// Store backtest results
    pub async fn store_results(&self, results: BacktestResults) {
        let mut backtests = self.backtests.write().await;
        let id = results.backtest_id.clone();
        backtests.insert(
            id,
            StoredBacktest {
                results,
                created_at: chrono::Utc::now().timestamp_millis(),
            },
        );
    }
}

impl Default for BacktestServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl BacktestService for BacktestServiceImpl {
    async fn get_historical_data(
        &self,
        request: Request<GetHistoricalDataRequest>,
    ) -> Result<Response<GetHistoricalDataResponse>, Status> {
        let req = request.into_inner();

        // For now, use mock data since we don't have Zerodha credentials
        let bars = self.generate_mock_data(&req.symbol, &req.from_date, &req.to_date);

        Ok(Response::new(GetHistoricalDataResponse {
            success: true,
            error: String::new(),
            bars: bars.clone(),
            total_bars: bars.len() as i32,
        }))
    }

    type RunBacktestStream = ReceiverStream<Result<BacktestProgress, Status>>;

    async fn run_backtest(
        &self,
        request: Request<RunBacktestRequest>,
    ) -> Result<Response<Self::RunBacktestStream>, Status> {
        let req = request.into_inner();

        // Create channel for streaming progress
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Clone what we need for the async task
        let this = self.clone();

        // Spawn backtest task
        tokio::spawn(async move {
            // Get historical data
            let bars = this.generate_mock_data(
                req.symbols.first().unwrap_or(&"RELIANCE".to_string()),
                &req.from_date,
                &req.to_date,
            );

            // Run backtest
            match this.run_python_backtest(&req, &bars, tx.clone()).await {
                Ok(results) => {
                    // Store results
                    this.store_results(results.clone()).await;

                    // Send completed status with results
                    let _ = tx
                        .send(Ok(BacktestProgress {
                            status: "completed".to_string(),
                            progress_percent: 100,
                            message: "Backtest completed successfully".to_string(),
                            current_equity: results.total_return + req.initial_capital,
                            trades_completed: results.total_trades,
                            results: Some(results),
                        }))
                        .await;
                }
                Err(e) => {
                    // Send error status
                    let _ = tx
                        .send(Ok(BacktestProgress {
                            status: "error".to_string(),
                            progress_percent: 0,
                            message: format!("Backtest failed: {}", e),
                            current_equity: req.initial_capital,
                            trades_completed: 0,
                            results: None,
                        }))
                        .await;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn get_backtest_results(
        &self,
        request: Request<GetBacktestResultsRequest>,
    ) -> Result<Response<BacktestResultsResponse>, Status> {
        let req = request.into_inner();
        let backtests = self.backtests.read().await;

        match backtests.get(&req.backtest_id) {
            Some(stored) => Ok(Response::new(BacktestResultsResponse {
                success: true,
                error: String::new(),
                results: Some(stored.results.clone()),
            })),
            None => Ok(Response::new(BacktestResultsResponse {
                success: false,
                error: format!("Backtest {} not found", req.backtest_id),
                results: None,
            })),
        }
    }

    async fn list_backtests(
        &self,
        request: Request<ListBacktestsRequest>,
    ) -> Result<Response<ListBacktestsResponse>, Status> {
        let req = request.into_inner();
        let backtests = self.backtests.read().await;

        let mut summaries: Vec<_> = backtests
            .iter()
            .map(|(_, stored)| BacktestSummary {
                backtest_id: stored.results.backtest_id.clone(),
                strategy_name: stored.results.strategy_name.clone(),
                created_at: stored.created_at,
                total_return_percent: stored.results.total_return_percent,
                sharpe_ratio: stored.results.sharpe_ratio,
                total_trades: stored.results.total_trades,
            })
            .collect();

        // Sort by created_at descending
        summaries.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let total = summaries.len() as i32;
        let offset = req.offset as usize;
        let limit = if req.limit > 0 { req.limit as usize } else { 10 };

        let paginated: Vec<BacktestSummary> = summaries
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect();

        Ok(Response::new(ListBacktestsResponse {
            backtests: paginated,
            total,
        }))
    }
}

impl Clone for BacktestServiceImpl {
    fn clone(&self) -> Self {
        Self {
            backtests: Arc::clone(&self.backtests),
            http_client: self.http_client.clone(),
        }
    }
}
