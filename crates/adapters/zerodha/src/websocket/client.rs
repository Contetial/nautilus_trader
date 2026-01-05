// -------------------------------------------------------------------------------------------------
//  Copyright (C) 2015-2025 Nautech Systems Pty Ltd. All rights reserved.
//  https://nautechsystems.io
//
//  Licensed under the GNU Lesser General Public License Version 3.0 (the "License");
//  You may not use this file except in compliance with the License.
//  You may obtain a copy of the License at https://www.gnu.org/licenses/lgpl-3.0.en.html
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.
// -------------------------------------------------------------------------------------------------

//! WebSocket client implementation for Zerodha Kite Ticker.

use crate::{
    config::ZerodhaWebSocketConfig,
    enums::TickerMode,
    error::{ZerodhaError, ZerodhaResult},
    types::{ZerodhaTick, OHLC},
};
use rust_decimal::{Decimal, prelude::FromPrimitive};
use dashmap::DashMap;
use futures_util::{
    sink::SinkExt,
    stream::{SplitSink, SplitStream, StreamExt},
};
use serde_json::json;
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Instant,
};
use tokio::{
    net::TcpStream,
    sync::{mpsc, RwLock},
    time::{interval, sleep},
};
use tokio_tungstenite::{
    connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, error, info, warn};
use url::Url;

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsSink = SplitSink<WsStream, Message>;
type WsReceiver = SplitStream<WsStream>;

/// WebSocket client for Zerodha Kite Ticker
#[derive(Debug)]
pub struct ZerodhaWebSocketClient {
    config: ZerodhaWebSocketConfig,
    subscriptions: Arc<RwLock<HashSet<u32>>>,
    modes: Arc<DashMap<u32, TickerMode>>,
    is_connected: Arc<AtomicBool>,
    reconnect_count: Arc<AtomicU64>,
    last_heartbeat: Arc<RwLock<Instant>>,
    
    // Communication channels
    tick_tx: mpsc::UnboundedSender<ZerodhaTick>,
    command_tx: Option<mpsc::UnboundedSender<WsCommand>>,
}

#[derive(Debug, Clone)]
enum WsCommand {
    Subscribe { tokens: Vec<u32>, mode: TickerMode },
    Unsubscribe { tokens: Vec<u32> },
    SetMode { tokens: Vec<u32>, mode: TickerMode },
    Ping,
    Reconnect,
}

impl ZerodhaWebSocketClient {
    /// Create a new WebSocket client
    pub fn new(
        config: ZerodhaWebSocketConfig,
        tick_tx: mpsc::UnboundedSender<ZerodhaTick>,
    ) -> Self {
        Self {
            config,
            subscriptions: Arc::new(RwLock::new(HashSet::new())),
            modes: Arc::new(DashMap::new()),
            is_connected: Arc::new(AtomicBool::new(false)),
            reconnect_count: Arc::new(AtomicU64::new(0)),
            last_heartbeat: Arc::new(RwLock::new(Instant::now())),
            tick_tx,
            command_tx: None,
        }
    }
    
    /// Start the WebSocket client
    pub async fn start(&mut self) -> ZerodhaResult<()> {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        self.command_tx = Some(command_tx.clone());
        
        let config = self.config.clone();
        let subscriptions = Arc::clone(&self.subscriptions);
        let modes = Arc::clone(&self.modes);
        let is_connected = Arc::clone(&self.is_connected);
        let reconnect_count = Arc::clone(&self.reconnect_count);
        let last_heartbeat = Arc::clone(&self.last_heartbeat);
        let tick_tx = self.tick_tx.clone();
        
        // Spawn the main WebSocket task
        tokio::spawn(async move {
            let mut connection_handler = ConnectionHandler::new(
                config,
                subscriptions,
                modes,
                is_connected,
                reconnect_count,
                last_heartbeat,
                tick_tx,
                command_rx,
            );
            
            connection_handler.run().await;
        });
        
        Ok(())
    }
    
    /// Subscribe to instruments
    pub async fn subscribe(&self, tokens: Vec<u32>, mode: TickerMode) -> ZerodhaResult<()> {
        if let Some(tx) = &self.command_tx {
            tx.send(WsCommand::Subscribe { tokens: tokens.clone(), mode })
                .map_err(|e| ZerodhaError::websocket_error(format!("Failed to send subscribe command: {}", e)))?;
            
            // Update local subscription tracking
            let mut subs = self.subscriptions.write().await;
            for token in tokens {
                subs.insert(token);
                self.modes.insert(token, mode);
            }
        } else {
            return Err(ZerodhaError::websocket_error("WebSocket client not started"));
        }
        
        Ok(())
    }
    
    /// Unsubscribe from instruments
    pub async fn unsubscribe(&self, tokens: Vec<u32>) -> ZerodhaResult<()> {
        if let Some(tx) = &self.command_tx {
            tx.send(WsCommand::Unsubscribe { tokens: tokens.clone() })
                .map_err(|e| ZerodhaError::websocket_error(format!("Failed to send unsubscribe command: {}", e)))?;
            
            // Update local subscription tracking
            let mut subs = self.subscriptions.write().await;
            for token in tokens {
                subs.remove(&token);
                self.modes.remove(&token);
            }
        } else {
            return Err(ZerodhaError::websocket_error("WebSocket client not started"));
        }
        
        Ok(())
    }
    
    /// Set mode for subscribed instruments
    pub async fn set_mode(&self, tokens: Vec<u32>, mode: TickerMode) -> ZerodhaResult<()> {
        if let Some(tx) = &self.command_tx {
            tx.send(WsCommand::SetMode { tokens: tokens.clone(), mode })
                .map_err(|e| ZerodhaError::websocket_error(format!("Failed to send set_mode command: {}", e)))?;
            
            // Update local mode tracking
            for token in tokens {
                self.modes.insert(token, mode);
            }
        } else {
            return Err(ZerodhaError::websocket_error("WebSocket client not started"));
        }
        
        Ok(())
    }
    
    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::Relaxed)
    }
    
    /// Get current subscriptions
    pub async fn get_subscriptions(&self) -> HashSet<u32> {
        self.subscriptions.read().await.clone()
    }
}

struct ConnectionHandler {
    config: ZerodhaWebSocketConfig,
    subscriptions: Arc<RwLock<HashSet<u32>>>,
    modes: Arc<DashMap<u32, TickerMode>>,
    is_connected: Arc<AtomicBool>,
    reconnect_count: Arc<AtomicU64>,
    last_heartbeat: Arc<RwLock<Instant>>,
    tick_tx: mpsc::UnboundedSender<ZerodhaTick>,
    command_rx: mpsc::UnboundedReceiver<WsCommand>,
}

impl ConnectionHandler {
    fn new(
        config: ZerodhaWebSocketConfig,
        subscriptions: Arc<RwLock<HashSet<u32>>>,
        modes: Arc<DashMap<u32, TickerMode>>,
        is_connected: Arc<AtomicBool>,
        reconnect_count: Arc<AtomicU64>,
        last_heartbeat: Arc<RwLock<Instant>>,
        tick_tx: mpsc::UnboundedSender<ZerodhaTick>,
        command_rx: mpsc::UnboundedReceiver<WsCommand>,
    ) -> Self {
        Self {
            config,
            subscriptions,
            modes,
            is_connected,
            reconnect_count,
            last_heartbeat,
            tick_tx,
            command_rx,
        }
    }
    
    async fn run(&mut self) {
        loop {
            match self.connect_and_handle().await {
                Ok(_) => {
                    info!("WebSocket connection closed normally");
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                }
            }
            
            // Check if we should reconnect
            if !self.config.auto_reconnect {
                break;
            }
            
            let reconnect_count = self.reconnect_count.fetch_add(1, Ordering::Relaxed);
            if reconnect_count >= self.config.max_reconnect_attempts as u64 {
                error!("Maximum reconnection attempts reached");
                break;
            }
            
            warn!("Reconnecting in {:?} (attempt {})", self.config.reconnect_delay, reconnect_count + 1);
            sleep(self.config.reconnect_delay).await;
        }
    }
    
    async fn connect_and_handle(&mut self) -> ZerodhaResult<()> {
        // Build WebSocket URL with authentication
        let ws_url = format!(
            "{}?api_key={}&access_token={}",
            self.config.url, self.config.api_key, self.config.access_token
        );
        
        let url = Url::parse(&ws_url)
            .map_err(|e| ZerodhaError::websocket_error(format!("Invalid WebSocket URL: {}", e)))?;
        
        info!("Connecting to Zerodha WebSocket: {}", url.host_str().unwrap_or("unknown"));
        
        // Connect to WebSocket
        let (ws_stream, response) = connect_async(url.as_str()).await
            .map_err(|e| ZerodhaError::websocket_error(format!("Connection failed: {}", e)))?;
        
        info!("WebSocket connected with status: {}", response.status());
        self.is_connected.store(true, Ordering::Relaxed);
        
        let (ws_sink, ws_stream) = ws_stream.split();
        
        // Start message handling tasks
        let message_handler = self.spawn_message_handler(ws_stream);
        let command_handler = self.spawn_command_handler(ws_sink);
        let heartbeat_handler = self.spawn_heartbeat_handler();
        
        // Resubscribe to previous subscriptions
        self.resubscribe_all().await;
        
        // Wait for any handler to complete (error or disconnect)
        tokio::select! {
            result = message_handler => {
                warn!("Message handler finished: {:?}", result);
            }
            result = command_handler => {
                warn!("Command handler finished: {:?}", result);
            }
            _ = heartbeat_handler => {
                warn!("Heartbeat handler finished");
            }
        }
        
        self.is_connected.store(false, Ordering::Relaxed);
        Ok(())
    }
    
    fn spawn_message_handler(&self, mut ws_stream: WsReceiver) -> tokio::task::JoinHandle<ZerodhaResult<()>> {
        let tick_tx = self.tick_tx.clone();
        let last_heartbeat = Arc::clone(&self.last_heartbeat);
        
        tokio::spawn(async move {
            while let Some(message) = ws_stream.next().await {
                match message {
                    Ok(Message::Binary(data)) => {
                        // Update heartbeat
                        *last_heartbeat.write().await = Instant::now();
                        
                        // Parse binary tick data
                        match Self::parse_tick_data(&data) {
                            Ok(ticks) => {
                                for tick in ticks {
                                    if let Err(e) = tick_tx.send(tick) {
                                        error!("Failed to send tick data: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse tick data: {}", e);
                            }
                        }
                    }
                    Ok(Message::Text(text)) => {
                        debug!("Received text message: {}", text);
                        // Handle text messages (usually status or error messages)
                    }
                    Ok(Message::Ping(data)) => {
                        debug!("Received ping: {:?}", data);
                    }
                    Ok(Message::Pong(data)) => {
                        debug!("Received pong: {:?}", data);
                        *last_heartbeat.write().await = Instant::now();
                    }
                    Ok(Message::Close(frame)) => {
                        info!("WebSocket closed: {:?}", frame);
                        break;
                    }
                    Ok(Message::Frame(_)) => {
                        // Raw frames are handled by the underlying WebSocket implementation
                        debug!("Received raw frame");
                    }
                    Err(e) => {
                        error!("WebSocket message error: {}", e);
                        return Err(ZerodhaError::websocket_error(format!("Message error: {}", e)));
                    }
                }
            }
            Ok(())
        })
    }
    
    fn spawn_command_handler(&mut self, mut ws_sink: WsSink) -> tokio::task::JoinHandle<ZerodhaResult<()>> {
        let mut command_rx = std::mem::replace(&mut self.command_rx, mpsc::unbounded_channel().1);
        
        tokio::spawn(async move {
            while let Some(command) = command_rx.recv().await {
                let message = match Self::build_command_message(command) {
                    Ok(msg) => msg,
                    Err(e) => {
                        error!("Failed to build command message: {}", e);
                        continue;
                    }
                };
                
                if let Err(e) = ws_sink.send(Message::Text(message.into())).await {
                    error!("Failed to send WebSocket message: {}", e);
                    return Err(ZerodhaError::websocket_error(format!("Send error: {}", e)));
                }
            }
            Ok(())
        })
    }
    
    fn spawn_heartbeat_handler(&self) -> tokio::task::JoinHandle<()> {
        let last_heartbeat = Arc::clone(&self.last_heartbeat);
        let ping_interval = self.config.ping_interval;
        let is_connected = Arc::clone(&self.is_connected);
        
        tokio::spawn(async move {
            let mut ping_timer = interval(ping_interval);
            
            loop {
                ping_timer.tick().await;
                
                if !is_connected.load(Ordering::Relaxed) {
                    break;
                }
                
                let last_heartbeat_time = *last_heartbeat.read().await;
                let elapsed = last_heartbeat_time.elapsed();
                
                if elapsed > ping_interval * 2 {
                    warn!("No heartbeat for {:?}, connection may be stale", elapsed);
                    // Could trigger reconnection here
                }
            }
        })
    }
    
    async fn resubscribe_all(&self) {
        let subscriptions = self.subscriptions.read().await.clone();
        if subscriptions.is_empty() {
            return;
        }
        
        // Group subscriptions by mode
        let mut mode_groups: std::collections::HashMap<TickerMode, Vec<u32>> = std::collections::HashMap::new();
        
        for token in subscriptions {
            let mode = self.modes.get(&token).map(|entry| *entry.value()).unwrap_or(TickerMode::LTP);
            mode_groups.entry(mode).or_default().push(token);
        }
        
        // Send subscription commands
        for (mode, tokens) in mode_groups {
            info!("Resubscribing {} instruments with mode {:?}", tokens.len(), mode);
            // This would normally send via command channel, but we're in the handler
            // so we'd need to send the WebSocket message directly
        }
    }
    
    fn build_command_message(command: WsCommand) -> ZerodhaResult<String> {
        let message = match command {
            WsCommand::Subscribe { tokens, mode: _ } => {
                json!({
                    "a": "subscribe",
                    "v": tokens
                })
            }
            WsCommand::Unsubscribe { tokens } => {
                json!({
                    "a": "unsubscribe",
                    "v": tokens
                })
            }
            WsCommand::SetMode { tokens, mode } => {
                json!({
                    "a": "mode",
                    "v": [mode.to_string().to_lowercase(), tokens]
                })
            }
            WsCommand::Ping => {
                json!({
                    "a": "ping"
                })
            }
            WsCommand::Reconnect => {
                return Err(ZerodhaError::websocket_error("Reconnect command not supported"));
            }
        };
        
        serde_json::to_string(&message)
            .map_err(|e| ZerodhaError::websocket_error(format!("JSON serialization error: {}", e)))
    }
    
    fn parse_tick_data(data: &[u8]) -> ZerodhaResult<Vec<ZerodhaTick>> {
        let mut ticks = Vec::new();
        let mut cursor = 0;
        
        while cursor + 4 <= data.len() {
            // Read packet length (first 2 bytes)
            let packet_length = u16::from_be_bytes([data[cursor], data[cursor + 1]]) as usize;
            cursor += 2;
            
            if cursor + packet_length > data.len() {
                break;
            }
            
            // Read number of packets (next 2 bytes)  
            let num_packets = u16::from_be_bytes([data[cursor], data[cursor + 1]]);
            cursor += 2;
            
            for _ in 0..num_packets {
                if let Ok(tick) = Self::parse_single_tick(&data[cursor..]) {
                    let tick_size = Self::get_tick_size(&tick);
                    ticks.push(tick);
                    cursor += tick_size;
                } else {
                    break;
                }
            }
        }
        
        Ok(ticks)
    }
    
    fn parse_single_tick(data: &[u8]) -> ZerodhaResult<ZerodhaTick> {
        if data.len() < 8 {
            return Err(ZerodhaError::parse_error("Insufficient data for tick"));
        }
        
        // Read instrument token (4 bytes)
        let instrument_token = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        
        // Determine tick mode based on data length
        let mode = match data.len() {
            8 => TickerMode::LTP,     // LTP mode: 8 bytes
            28 => TickerMode::Quote,   // Quote mode: 28 bytes  
            _ => TickerMode::Full,     // Full mode: 164+ bytes
        };
        
        let mut tick = ZerodhaTick {
            instrument_token,
            exchange_timestamp: None,
            last_price: Decimal::ZERO,
            last_quantity: None,
            average_price: None,
            volume_traded: None,
            total_buy_quantity: None,
            total_sell_quantity: None,
            ohlc: None,
            change: None,
            oi: None,
            oi_change: None,
            depth: None,
            mode,
        };
        
        // Parse based on mode
        match mode {
            TickerMode::LTP => {
                // LTP mode: instrument_token(4) + last_price(4)
                tick.last_price = Decimal::from_f64(f32::from_be_bytes([data[4], data[5], data[6], data[7]]) as f64 / 100.0).unwrap_or_default();
            }
            
            TickerMode::Quote => {
                // Quote mode: LTP + volume + bid/ask prices
                tick.last_price = Decimal::from_f64(f32::from_be_bytes([data[4], data[5], data[6], data[7]]) as f64 / 100.0).unwrap_or_default();
                
                if data.len() >= 12 {
                    tick.last_quantity = Some(u32::from_be_bytes([data[8], data[9], data[10], data[11]]));
                }
                
                if data.len() >= 16 {
                    tick.average_price = Some(Decimal::from_f64(f32::from_be_bytes([data[12], data[13], data[14], data[15]]) as f64 / 100.0).unwrap_or_default());
                }
                
                if data.len() >= 20 {
                    tick.volume_traded = Some(u32::from_be_bytes([data[16], data[17], data[18], data[19]]));
                }
                
                if data.len() >= 24 {
                    tick.total_buy_quantity = Some(u32::from_be_bytes([data[20], data[21], data[22], data[23]]));
                }
                
                if data.len() >= 28 {
                    tick.total_sell_quantity = Some(u32::from_be_bytes([data[24], data[25], data[26], data[27]]));
                }
            }
            
            TickerMode::Full => {
                // Full mode: All data including OHLC, depth, OI
                tick.last_price = Decimal::from_f64(f32::from_be_bytes([data[4], data[5], data[6], data[7]]) as f64 / 100.0).unwrap_or_default();
                
                if data.len() >= 12 {
                    tick.last_quantity = Some(u32::from_be_bytes([data[8], data[9], data[10], data[11]]));
                }
                
                if data.len() >= 16 {
                    tick.average_price = Some(Decimal::from_f64(f32::from_be_bytes([data[12], data[13], data[14], data[15]]) as f64 / 100.0).unwrap_or_default());
                }
                
                if data.len() >= 20 {
                    tick.volume_traded = Some(u32::from_be_bytes([data[16], data[17], data[18], data[19]]));
                }
                
                if data.len() >= 24 {
                    tick.total_buy_quantity = Some(u32::from_be_bytes([data[20], data[21], data[22], data[23]]));
                }
                
                if data.len() >= 28 {
                    tick.total_sell_quantity = Some(u32::from_be_bytes([data[24], data[25], data[26], data[27]]));
                }
                
                // Parse OHLC (32-48 bytes)
                if data.len() >= 48 {
                    tick.ohlc = Some(OHLC {
                        open: Decimal::from_f64(f32::from_be_bytes([data[28], data[29], data[30], data[31]]) as f64 / 100.0).unwrap_or_default(),
                        high: Decimal::from_f64(f32::from_be_bytes([data[32], data[33], data[34], data[35]]) as f64 / 100.0).unwrap_or_default(),
                        low: Decimal::from_f64(f32::from_be_bytes([data[36], data[37], data[38], data[39]]) as f64 / 100.0).unwrap_or_default(),
                        close: Decimal::from_f64(f32::from_be_bytes([data[40], data[41], data[42], data[43]]) as f64 / 100.0).unwrap_or_default(),
                    });
                }
                
                // Parse change (48-52 bytes)
                if data.len() >= 52 {
                    tick.change = Some(Decimal::from_f64(f32::from_be_bytes([data[44], data[45], data[46], data[47]]) as f64 / 100.0).unwrap_or_default());
                }
                
                // Parse OI data for derivatives (52-64 bytes)
                if data.len() >= 64 {
                    tick.oi = Some(u32::from_be_bytes([data[48], data[49], data[50], data[51]]));
                    // Note: oi_day_high and oi_day_low not available in ZerodhaTick struct
                }
                
                // Parse timestamp (64-68 bytes) - Note: timestamp field not available in ZerodhaTick
                if data.len() >= 68 {
                    let _timestamp_secs = u32::from_be_bytes([data[60], data[61], data[62], data[63]]);
                    // tick.timestamp not available in ZerodhaTick struct
                }
                
                // Parse market depth (68+ bytes) - Note: depth field not available in ZerodhaTick
                if data.len() >= 164 {
                    let _depth_data = &data[64..164];
                    // tick.depth not available in ZerodhaTick struct
                }
            }
        }
        
        Ok(tick)
    }
    
    fn parse_market_depth(data: &[u8]) -> ZerodhaResult<Option<crate::types::ZerodhaDepth>> {
        use crate::types::{ZerodhaDepth, ZerodhaDepthItem};
        
        if data.len() < 100 {
            return Ok(None);
        }
        
        let mut buy_orders = Vec::new();
        let mut sell_orders = Vec::new();
        
        // Parse 5 buy orders (0-50 bytes)
        for i in 0..5 {
            let offset = i * 10;
            if offset + 10 <= data.len() {
                let quantity = u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
                let price = Decimal::from_f64(f32::from_be_bytes([data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]]) as f64 / 100.0).unwrap_or_default();
                let orders = u16::from_be_bytes([data[offset + 8], data[offset + 9]]) as u32;
                
                if quantity > 0 && price > Decimal::ZERO {
                    buy_orders.push(ZerodhaDepthItem {
                        quantity,
                        price,
                        orders,
                    });
                }
            }
        }
        
        // Parse 5 sell orders (50-100 bytes)
        for i in 0..5 {
            let offset = 50 + (i * 10);
            if offset + 10 <= data.len() {
                let quantity = u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
                let price = Decimal::from_f64(f32::from_be_bytes([data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]]) as f64 / 100.0).unwrap_or_default();
                let orders = u16::from_be_bytes([data[offset + 8], data[offset + 9]]) as u32;
                
                if quantity > 0 && price > Decimal::ZERO {
                    sell_orders.push(ZerodhaDepthItem {
                        quantity,
                        price,
                        orders,
                    });
                }
            }
        }
        
        Ok(Some(ZerodhaDepth {
            buy: buy_orders,
            sell: sell_orders,
        }))
    }
    
    fn get_tick_size(tick: &ZerodhaTick) -> usize {
        match tick.mode {
            TickerMode::LTP => 8,
            TickerMode::Quote => 28,
            TickerMode::Full => 164,
        }
    }
}

impl Drop for ZerodhaWebSocketClient {
    fn drop(&mut self) {
        self.is_connected.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_websocket_client_creation() {
        let config = ZerodhaWebSocketConfig::default();
        let (tick_tx, _tick_rx) = mpsc::unbounded_channel();
        
        let client = ZerodhaWebSocketClient::new(config, tick_tx);
        assert!(!client.is_connected());
    }
    
    #[test]
    fn test_command_message_building() {
        let command = WsCommand::Subscribe {
            tokens: vec![256265, 408065],
            mode: TickerMode::Full,
        };
        
        let message = ConnectionHandler::build_command_message(command).unwrap();
        assert!(message.contains("subscribe"));
        assert!(message.contains("256265"));
    }
}