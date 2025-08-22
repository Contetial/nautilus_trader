// -------------------------------------------------------------------------------------------------
//  Copyright (C) 2015-2025 Nautech Systems Pty Ltd. All rights reserved.
//  https://nautechsystems.io
//
//  Licensed under the GNU Lesser General Public License Version 3.0 (the "License");
//  You may not use this file except in compliance with the License.
//  You may not use this file except in compliance with the License.
//  You may obtain a copy of the License at https://www.gnu.org/licenses/lgpl-3.0.en.html
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.
// -------------------------------------------------------------------------------------------------

//! HTTP client for the Zerodha Kite Connect API.

use crate::{
    config::ZerodhaConfig,
    enums::*,
    error::{parse_api_error, ZerodhaError, ZerodhaResult},
    types::*,
};
use chrono::{DateTime, NaiveDate, Utc};
use reqwest::{header::HeaderMap, Client, Method, RequestBuilder, Response};
use rust_decimal::Decimal;
use serde_json::Value;
use std::{collections::HashMap, time::Duration};
use tokio::time::{sleep, Instant};
use tracing::{debug, error, info, warn};

/// Rate limiter to handle Zerodha's API limits (3 requests/second)
#[derive(Debug)]
struct RateLimiter {
    last_request: Option<Instant>,
    min_interval: Duration,
}

impl RateLimiter {
    fn new(requests_per_second: u32) -> Self {
        Self {
            last_request: None,
            min_interval: Duration::from_millis(1000 / requests_per_second as u64),
        }
    }
    
    async fn wait_if_needed(&mut self) {
        if let Some(last) = self.last_request {
            let elapsed = last.elapsed();
            if elapsed < self.min_interval {
                let wait_time = self.min_interval - elapsed;
                sleep(wait_time).await;
            }
        }
        self.last_request = Some(Instant::now());
    }
}

/// HTTP client for Zerodha Kite Connect API
#[derive(Debug)]
pub struct ZerodhaHttpClient {
    client: Client,
    config: ZerodhaConfig,
    rate_limiter: std::sync::Mutex<RateLimiter>,
}

impl ZerodhaHttpClient {
    /// Create a new HTTP client
    pub fn new(config: ZerodhaConfig) -> ZerodhaResult<Self> {
        config.validate().map_err(ZerodhaError::config_error)?;
        
        let client = Client::builder()
            .timeout(config.request_timeout)
            .build()
            .map_err(|e| ZerodhaError::Internal(format!("Failed to create HTTP client: {}", e)))?;
        
        let rate_limiter = std::sync::Mutex::new(RateLimiter::new(config.rate_limit_per_second));
        
        Ok(Self {
            client,
            config,
            rate_limiter,
        })
    }
    
    /// Build request with authentication headers
    fn build_request(&self, method: Method, path: &str) -> ZerodhaResult<RequestBuilder> {
        let access_token = self.config.access_token.as_ref()
            .ok_or_else(|| ZerodhaError::auth_error("Access token not configured"))?;
        
        let url = format!("{}/{}", self.config.base_url.trim_end_matches('/'), path);
        
        let mut headers = HeaderMap::new();
        headers.insert("X-Kite-Version", "3".parse().unwrap());
        headers.insert(
            "Authorization",
            format!("token {}:{}", self.config.api_key, access_token)
                .parse()
                .unwrap(),
        );
        
        let request = self.client.request(method, &url).headers(headers);
        
        if self.config.debug_mode {
            debug!("Building request: {} {}", method, url);
        }
        
        Ok(request)
    }
    
    /// Execute HTTP request with rate limiting and error handling
    async fn execute_request(&self, request: RequestBuilder) -> ZerodhaResult<Response> {
        // Apply rate limiting
        {
            let mut limiter = self.rate_limiter.lock().unwrap();
            limiter.wait_if_needed().await;
        }
        
        let response = request.send().await?;
        
        if self.config.debug_mode {
            debug!("Response status: {}", response.status());
        }
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            return Err(parse_api_error(status, &body));
        }
        
        Ok(response)
    }
    
    /// Execute request and parse JSON response
    async fn execute_json<T>(&self, request: RequestBuilder) -> ZerodhaResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let response = self.execute_request(request).await?;
        let text = response.text().await?;
        
        if self.config.debug_mode {
            debug!("Response body: {}", text);
        }
        
        let parsed: ZerodhaResponse<T> = serde_json::from_str(&text)?;
        parsed.into_result()
    }
    
    // =========================================================================
    // Market Data Methods
    // =========================================================================
    
    /// Get all instruments
    pub async fn get_instruments(&self) -> ZerodhaResult<Vec<ZerodhaInstrument>> {
        let request = self.build_request(Method::GET, "instruments")?;
        
        // Zerodha instruments endpoint returns CSV data
        let response = self.execute_request(request).await?;
        let text = response.text().await?;
        
        self.parse_instruments_csv(&text)
    }
    
    /// Get instruments for a specific exchange
    pub async fn get_instruments_for_exchange(&self, exchange: Exchange) -> ZerodhaResult<Vec<ZerodhaInstrument>> {
        let path = format!("instruments/{}", exchange.to_string());
        let request = self.build_request(Method::GET, &path)?;
        
        let response = self.execute_request(request).await?;
        let text = response.text().await?;
        
        self.parse_instruments_csv(&text)
    }
    
    /// Get quote for instruments
    pub async fn get_quotes(&self, instruments: &[u32]) -> ZerodhaResult<HashMap<String, ZerodhaQuote>> {
        if instruments.is_empty() {
            return Ok(HashMap::new());
        }
        
        let instrument_list = instruments
            .iter()
            .map(|token| token.to_string())
            .collect::<Vec<_>>()
            .join(",");
        
        let path = format!("quote?i={}", instrument_list);
        let request = self.build_request(Method::GET, &path)?;
        
        self.execute_json(request).await
    }
    
    /// Get historical candle data
    pub async fn get_historical_data(
        &self,
        instrument_token: u32,
        interval: Interval,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        continuous: Option<bool>,
        oi: Option<bool>,
    ) -> ZerodhaResult<Vec<ZerodhaCandle>> {
        let path = format!(
            "instruments/historical/{}/{}",
            instrument_token,
            interval.to_string()
        );
        
        let mut request = self.build_request(Method::GET, &path)?
            .query(&[
                ("from", from.format("%Y-%m-%d %H:%M:%S").to_string()),
                ("to", to.format("%Y-%m-%d %H:%M:%S").to_string()),
            ]);
        
        if let Some(continuous) = continuous {
            request = request.query(&[("continuous", continuous.to_string())]);
        }
        
        if let Some(oi) = oi {
            request = request.query(&[("oi", oi.to_string())]);
        }
        
        let response: ZerodhaResponse<HashMap<String, Vec<Value>>> = self.execute_json(request).await?;
        
        // Parse candle data from the nested structure
        if let Some(candles_data) = response.data {
            if let Some(candles) = candles_data.get("candles") {
                return self.parse_candles(candles);
            }
        }
        
        Ok(vec![])
    }
    
    // =========================================================================
    // Trading Methods
    // =========================================================================
    
    /// Place an order
    pub async fn place_order(
        &self,
        exchange: Exchange,
        tradingsymbol: &str,
        transaction_type: TransactionType,
        quantity: u32,
        product: Product,
        order_type: OrderType,
        price: Option<Decimal>,
        trigger_price: Option<Decimal>,
        validity: Option<Validity>,
        disclosed_quantity: Option<u32>,
        tag: Option<&str>,
    ) -> ZerodhaResult<String> {
        let mut params = vec![
            ("exchange", exchange.to_string()),
            ("tradingsymbol", tradingsymbol.to_string()),
            ("transaction_type", transaction_type.to_string()),
            ("quantity", quantity.to_string()),
            ("product", product.to_string()),
            ("order_type", order_type.to_string()),
            ("validity", validity.unwrap_or_default().to_string()),
        ];
        
        if let Some(price) = price {
            params.push(("price", price.to_string()));
        }
        
        if let Some(trigger_price) = trigger_price {
            params.push(("trigger_price", trigger_price.to_string()));
        }
        
        if let Some(disclosed_quantity) = disclosed_quantity {
            params.push(("disclosed_quantity", disclosed_quantity.to_string()));
        }
        
        if let Some(tag) = tag {
            params.push(("tag", tag.to_string()));
        }
        
        let request = self.build_request(Method::POST, "orders/regular")?
            .form(&params);
        
        let response: HashMap<String, String> = self.execute_json(request).await?;
        
        response.get("order_id")
            .cloned()
            .ok_or_else(|| ZerodhaError::Internal("No order_id in response".to_string()))
    }
    
    /// Get orders
    pub async fn get_orders(&self) -> ZerodhaResult<Vec<ZerodhaOrder>> {
        let request = self.build_request(Method::GET, "orders")?;
        self.execute_json(request).await
    }
    
    /// Get positions
    pub async fn get_positions(&self) -> ZerodhaResult<HashMap<String, Vec<ZerodhaPosition>>> {
        let request = self.build_request(Method::GET, "portfolio/positions")?;
        self.execute_json(request).await
    }
    
    /// Place order using form parameters
    pub async fn place_order(&self, params: HashMap<String, String>) -> ZerodhaResult<crate::types::ZerodhaOrderResponse> {
        let form_params: Vec<(String, String)> = params.into_iter().collect();
        
        let request = self.build_request(Method::POST, "orders/regular")?
            .form(&form_params);
        
        let response: HashMap<String, serde_json::Value> = self.execute_json(request).await?;
        
        let order_id = response.get("order_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZerodhaError::parse_error("No order_id in response"))?;
        
        Ok(crate::types::ZerodhaOrderResponse {
            order_id: order_id.to_string(),
        })
    }
    
    /// Modify existing order
    pub async fn modify_order(&self, params: HashMap<String, String>) -> ZerodhaResult<crate::types::ZerodhaOrderResponse> {
        let order_id = params.get("order_id")
            .ok_or_else(|| ZerodhaError::validation_error("order_id required for modification"))?;
        
        let form_params: Vec<(String, String)> = params.into_iter()
            .filter(|(k, _)| k != "order_id")
            .collect();
        
        let path = format!("orders/regular/{}", order_id);
        let request = self.build_request(Method::PUT, &path)?
            .form(&form_params);
        
        let response: HashMap<String, serde_json::Value> = self.execute_json(request).await?;
        
        let order_id = response.get("order_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZerodhaError::parse_error("No order_id in response"))?;
        
        Ok(crate::types::ZerodhaOrderResponse {
            order_id: order_id.to_string(),
        })
    }
    
    /// Cancel existing order
    pub async fn cancel_order(&self, order_id: &str) -> ZerodhaResult<crate::types::ZerodhaOrderResponse> {
        let path = format!("orders/regular/{}", order_id);
        let request = self.build_request(Method::DELETE, &path)?;
        
        let response: HashMap<String, serde_json::Value> = self.execute_json(request).await?;
        
        let order_id = response.get("order_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZerodhaError::parse_error("No order_id in response"))?;
        
        Ok(crate::types::ZerodhaOrderResponse {
            order_id: order_id.to_string(),
        })
    }
    
    /// Get order details by ID
    pub async fn get_order_details(&self, order_id: &str) -> ZerodhaResult<crate::types::ZerodhaOrder> {
        let path = format!("orders/{}", order_id);
        let request = self.build_request(Method::GET, &path)?;
        self.execute_json(request).await
    }
    
    /// Get order history/trades for an order
    pub async fn get_order_history(&self, order_id: &str) -> ZerodhaResult<Vec<crate::types::ZerodhaOrder>> {
        let path = format!("orders/{}/trades", order_id);
        let request = self.build_request(Method::GET, &path)?;
        self.execute_json(request).await
    }
    
    /// Get account margins for all segments
    pub async fn get_margins(&self) -> ZerodhaResult<crate::types::ZerodhaMarginResponse> {
        let request = self.build_request(Method::GET, "user/margins")?;
        self.execute_json(request).await
    }
    
    /// Get user profile information
    pub async fn get_user_profile(&self) -> ZerodhaResult<crate::types::ZerodhaUserProfile> {
        let request = self.build_request(Method::GET, "user/profile")?;
        self.execute_json(request).await
    }
    
    /// Get account summary (profile + margins)
    pub async fn get_account_summary(&self) -> ZerodhaResult<crate::types::ZerodhaAccountSummary> {
        let profile_fut = self.get_user_profile();
        let margins_fut = self.get_margins();
        
        let (profile, margins) = tokio::try_join!(profile_fut, margins_fut)?;
        
        Ok(crate::types::ZerodhaAccountSummary {
            profile,
            margins,
            last_updated: chrono::Utc::now(),
        })
    }
    
    /// Get simplified account balance
    pub async fn get_account_balance(&self) -> ZerodhaResult<crate::types::ZerodhaAccountBalance> {
        let margins = self.get_margins().await?;
        
        Ok(crate::types::ZerodhaAccountBalance {
            available_cash: margins.available_cash,
            used_margin: margins.utilised.total,
            net_balance: margins.available.net,
            opening_balance: margins.available.opening_balance,
            live_balance: margins.available.live_balance,
            last_updated: chrono::Utc::now(),
        })
    }
    
    // =========================================================================
    // Helper Methods
    // =========================================================================
    
    /// Parse instruments CSV data
    fn parse_instruments_csv(&self, csv_data: &str) -> ZerodhaResult<Vec<ZerodhaInstrument>> {
        let mut instruments = Vec::new();
        let lines: Vec<&str> = csv_data.lines().collect();
        
        if lines.is_empty() {
            return Ok(instruments);
        }
        
        // Skip header line
        for line in lines.iter().skip(1) {
            if let Ok(instrument) = self.parse_instrument_line(line) {
                instruments.push(instrument);
            }
        }
        
        info!("Parsed {} instruments from CSV", instruments.len());
        Ok(instruments)
    }
    
    /// Parse individual instrument line from CSV
    fn parse_instrument_line(&self, line: &str) -> ZerodhaResult<ZerodhaInstrument> {
        let fields: Vec<&str> = line.split(',').collect();
        
        if fields.len() < 12 {
            return Err(ZerodhaError::invalid_input("Invalid CSV line format"));
        }
        
        Ok(ZerodhaInstrument {
            instrument_token: fields[0].parse().map_err(|_| {
                ZerodhaError::invalid_input("Invalid instrument_token")
            })?,
            exchange_token: fields[1].parse().map_err(|_| {
                ZerodhaError::invalid_input("Invalid exchange_token")
            })?,
            tradingsymbol: fields[2].to_string(),
            name: fields[3].to_string(),
            last_price: fields[4].parse().map_err(|_| {
                ZerodhaError::invalid_input("Invalid last_price")
            })?,
            expiry: if fields[5].is_empty() {
                None
            } else {
                Some(NaiveDate::parse_from_str(fields[5], "%Y-%m-%d").map_err(|_| {
                    ZerodhaError::invalid_input("Invalid expiry date format")
                })?)
            },
            strike: if fields[6].is_empty() {
                None
            } else {
                Some(fields[6].parse().map_err(|_| {
                    ZerodhaError::invalid_input("Invalid strike price")
                })?)
            },
            tick_size: fields[7].parse().map_err(|_| {
                ZerodhaError::invalid_input("Invalid tick_size")
            })?,
            lot_size: fields[8].parse().map_err(|_| {
                ZerodhaError::invalid_input("Invalid lot_size")
            })?,
            instrument_type: fields[9].parse().map_err(|_| {
                ZerodhaError::invalid_input("Invalid instrument_type")
            })?,
            segment: fields[10].parse().map_err(|_| {
                ZerodhaError::invalid_input("Invalid segment")
            })?,
            exchange: fields[11].parse().map_err(|_| {
                ZerodhaError::invalid_input("Invalid exchange")
            })?,
        })
    }
    
    /// Parse historical candle data
    fn parse_candles(&self, candles: &[Value]) -> ZerodhaResult<Vec<ZerodhaCandle>> {
        let mut result = Vec::new();
        
        for candle in candles {
            if let Some(array) = candle.as_array() {
                if array.len() >= 6 {
                    let timestamp = array[0]
                        .as_str()
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .ok_or_else(|| ZerodhaError::invalid_input("Invalid timestamp"))?;
                    
                    result.push(ZerodhaCandle {
                        timestamp,
                        open: array[1].as_f64()
                            .ok_or_else(|| ZerodhaError::invalid_input("Invalid open price"))?
                            .into(),
                        high: array[2].as_f64()
                            .ok_or_else(|| ZerodhaError::invalid_input("Invalid high price"))?
                            .into(),
                        low: array[3].as_f64()
                            .ok_or_else(|| ZerodhaError::invalid_input("Invalid low price"))?
                            .into(),
                        close: array[4].as_f64()
                            .ok_or_else(|| ZerodhaError::invalid_input("Invalid close price"))?
                            .into(),
                        volume: array[5].as_u64()
                            .ok_or_else(|| ZerodhaError::invalid_input("Invalid volume"))?
                            as u32,
                        oi: if array.len() > 6 {
                            array[6].as_u64().map(|v| v as u32)
                        } else {
                            None
                        },
                    });
                }
            }
        }
        
        Ok(result)
    }
    
    /// Generate access token from request token (for initial setup)
    pub async fn generate_session(&self, request_token: &str, api_secret: &str) -> ZerodhaResult<String> {
        use sha2::{Digest, Sha256};
        
        let checksum_data = format!("{}{}{}", self.config.api_key, request_token, api_secret);
        let mut hasher = Sha256::new();
        hasher.update(checksum_data.as_bytes());
        let checksum = hex::encode(hasher.finalize());
        
        let params = vec![
            ("api_key", self.config.api_key.as_str()),
            ("request_token", request_token),
            ("checksum", &checksum),
        ];
        
        let url = format!("{}/session/token", self.config.base_url);
        let request = self.client.post(&url).form(&params);
        
        let response = self.execute_request(request).await?;
        let text = response.text().await?;
        
        let parsed: ZerodhaResponse<HashMap<String, Value>> = serde_json::from_str(&text)?;
        let data = parsed.into_result()?;
        
        data.get("access_token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ZerodhaError::Internal("No access_token in response".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter() {
        let mut limiter = RateLimiter::new(3); // 3 requests per second
        assert_eq!(limiter.min_interval, Duration::from_millis(333));
    }

    #[test]
    fn test_instrument_parsing() {
        let csv_line = "256265,1,NIFTY 50,NIFTY 50,19500.0,,0,0.05,50,INDEX,INDICES,NSE";
        let client = ZerodhaHttpClient::new(ZerodhaConfig::default()).unwrap();
        
        let instrument = client.parse_instrument_line(csv_line).unwrap();
        assert_eq!(instrument.instrument_token, 256265);
        assert_eq!(instrument.tradingsymbol, "NIFTY 50");
        assert_eq!(instrument.instrument_type, InstrumentType::INDEX);
        assert_eq!(instrument.exchange, Exchange::NSE);
    }
}