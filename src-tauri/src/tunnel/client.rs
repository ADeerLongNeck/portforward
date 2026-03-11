use crate::config::ServerConfig;
use crate::crypto::{AesGcmCrypto, AuthManager, CryptoError};
use crate::protocol::{Frame, FrameCodec, FrameType, OpenChannelPayload};
use bytes::Bytes;
use dashmap::DashMap;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use super::{ConnectionInfo, HeartbeatManager, TunnelState};

#[derive(Debug, Error)]
pub enum TunnelError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Authentication failed")]
    AuthFailed,
    #[error("Channel error: {0}")]
    ChannelError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Protocol error: {0}")]
    ProtocolError(String),
    #[error("Already connected")]
    AlreadyConnected,
    #[error("Not connected")]
    NotConnected,
    #[error("Crypto error: {0}")]
    CryptoError(#[from] CryptoError),
}

/// Channel handle for bidirectional communication
pub struct ChannelHandle {
    pub sender: mpsc::Sender<Bytes>,
    pub created_at: std::time::Instant,
}

/// Tunnel client - connects to remote server
pub struct TunnelClient {
    config: ServerConfig,
    state: Arc<AtomicU8>,
    heartbeat: Arc<HeartbeatManager>,
    crypto: Option<AesGcmCrypto>,
    auth: AuthManager,
    channels: Arc<DashMap<u32, ChannelHandle>>,
    next_channel_id: AtomicU32,
    writer_tx: Option<mpsc::Sender<Frame>>,
}

impl TunnelClient {
    pub fn new(config: ServerConfig) -> Self {
        let crypto = if !config.auth.password.is_empty() {
            Some(AesGcmCrypto::new(&config.auth.password).ok()).flatten()
        } else {
            None
        };

        let auth = AuthManager::new(&config.auth.password);

        Self {
            config,
            state: Arc::new(AtomicU8::new(TunnelState::Disconnected as u8)),
            heartbeat: Arc::new(HeartbeatManager::new()),
            crypto,
            auth,
            channels: Arc::new(DashMap::new()),
            next_channel_id: AtomicU32::new(1),
            writer_tx: None,
        }
    }

    /// Get current connection state
    pub fn state(&self) -> TunnelState {
        match self.state.load(Ordering::SeqCst) {
            0 => TunnelState::Disconnected,
            1 => TunnelState::Connecting,
            2 => TunnelState::Authenticating,
            3 => TunnelState::Connected,
            4 => TunnelState::Reconnecting,
            _ => TunnelState::Error,
        }
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.state() == TunnelState::Connected
    }

    /// Get connection info
    pub fn connection_info(&self) -> ConnectionInfo {
        let state = self.state();
        ConnectionInfo {
            server_id: self.config.id.clone(),
            state,
            connected_at: None,
            latency_ms: None,
            error_message: None,
        }
    }

    /// Connect to the server
    pub async fn connect(&mut self) -> Result<ConnectionInfo, TunnelError> {
        if self.is_connected() {
            return Err(TunnelError::AlreadyConnected);
        }

        self.state.store(TunnelState::Connecting as u8, Ordering::SeqCst);

        // Connect to server
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let stream = TcpStream::connect(&addr)
            .await
            .map_err(|e| TunnelError::ConnectionFailed(e.to_string()))?;

        let (reader, mut writer) = tokio::io::split(stream);

        // Perform authentication
        self.state.store(TunnelState::Authenticating as u8, Ordering::SeqCst);

        // Read auth challenge
        let mut challenge_buf = [0u8; 1024];
        let mut reader = reader;
        let n = reader.read(&mut challenge_buf).await
            .map_err(|_| TunnelError::AuthFailed)?;

        // For now, simplified auth - just send password hash
        let response = self.auth.generate_response(&challenge_buf[..n])
            .map_err(|_| TunnelError::AuthFailed)?;
        writer.write_all(&response).await
            .map_err(|_| TunnelError::AuthFailed)?;

        // Auth success
        self.state.store(TunnelState::Connected as u8, Ordering::SeqCst);
        self.heartbeat.reset();

        // Create writer channel
        let (tx, mut rx) = mpsc::channel::<Frame>(128);
        self.writer_tx = Some(tx.clone());

        // Spawn heartbeat task
        let heartbeat = self.heartbeat.clone();
        let writer_tx = tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(heartbeat.interval());
            loop {
                interval.tick().await;
                if writer_tx.send(Frame::heartbeat()).await.is_err() {
                    break;
                }
                heartbeat.on_heartbeat_sent();
            }
        });

        Ok(self.connection_info())
    }

    /// Disconnect from server
    pub async fn disconnect(&mut self) -> Result<(), TunnelError> {
        self.state.store(TunnelState::Disconnected as u8, Ordering::SeqCst);
        self.writer_tx = None;
        self.channels.clear();
        Ok(())
    }

    /// Open a new channel to target
    pub async fn open_channel(&self, target_ip: &str, target_port: u16) -> Result<u32, TunnelError> {
        if !self.is_connected() {
            return Err(TunnelError::NotConnected);
        }

        let channel_id = self.next_channel_id.fetch_add(1, Ordering::SeqCst);
        let payload = OpenChannelPayload::new(target_ip.to_string(), target_port);

        let frame = Frame::open_channel(channel_id, Bytes::from(payload.encode()));

        if let Some(tx) = &self.writer_tx {
            tx.send(frame).await.map_err(|e| TunnelError::ChannelError(e.to_string()))?;
        }

        // Create channel handle
        let (ch_tx, _ch_rx) = mpsc::channel(64);
        self.channels.insert(channel_id, ChannelHandle {
            sender: ch_tx,
            created_at: std::time::Instant::now(),
        });

        Ok(channel_id)
    }

    /// Close a channel
    pub async fn close_channel(&self, channel_id: u32) -> Result<(), TunnelError> {
        self.channels.remove(&channel_id);

        if let Some(tx) = &self.writer_tx {
            let frame = Frame::close_channel(channel_id);
            tx.send(frame).await.map_err(|e| TunnelError::ChannelError(e.to_string()))?;
        }

        Ok(())
    }

    /// Send data through a channel
    pub async fn send_data(&self, channel_id: u32, data: &[u8]) -> Result<(), TunnelError> {
        if !self.is_connected() {
            return Err(TunnelError::NotConnected);
        }

        let data = if let Some(crypto) = &self.crypto {
            crypto.encrypt(data)?
        } else {
            data.to_vec()
        };

        let frame = Frame::data(channel_id, Bytes::from(data));

        if let Some(tx) = &self.writer_tx {
            tx.send(frame).await.map_err(|e| TunnelError::ChannelError(e.to_string()))?;
        }

        Ok(())
    }
}

/// Tunnel manager - manages multiple tunnel connections
pub struct TunnelManager {
    clients: DashMap<String, TunnelClient>,
}

impl TunnelManager {
    pub fn new() -> Self {
        Self {
            clients: DashMap::new(),
        }
    }

    pub async fn connect(&self, config: ServerConfig) -> Result<ConnectionInfo, TunnelError> {
        let server_id = config.id.clone();
        let mut client = TunnelClient::new(config);
        let info = client.connect().await?;
        self.clients.insert(server_id, client);
        Ok(info)
    }

    pub async fn disconnect(&self, server_id: &str) -> Result<(), TunnelError> {
        if let Some((_, mut client)) = self.clients.remove(server_id) {
            client.disconnect().await?;
        }
        Ok(())
    }

    pub fn get_client(&self, server_id: &str) -> Option<TunnelClient> {
        self.clients.get(server_id).map(|c| c.clone())
    }

    pub fn is_connected(&self, server_id: &str) -> bool {
        self.clients.get(server_id).map(|c| c.is_connected()).unwrap_or(false)
    }

    pub fn get_all_connections(&self) -> Vec<ConnectionInfo> {
        self.clients.iter().map(|c| c.connection_info()).collect()
    }
}

impl Default for TunnelManager {
    fn default() -> Self {
        Self::new()
    }
}

// Implement Clone for TunnelClient (needed for DashMap)
impl Clone for TunnelClient {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            state: self.state.clone(),
            heartbeat: self.heartbeat.clone(),
            crypto: self.crypto.clone(),
            auth: AuthManager::new(&self.config.auth.password),
            channels: self.channels.clone(),
            next_channel_id: AtomicU32::new(self.next_channel_id.load(Ordering::SeqCst)),
            writer_tx: None, // Don't clone the writer channel
        }
    }
}
