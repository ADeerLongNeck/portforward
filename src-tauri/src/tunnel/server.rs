use crate::config::ServerConfig;
use crate::crypto::{AesGcmCrypto, AuthManager};
use crate::protocol::{Frame, FrameCodec, FrameType, OpenChannelPayload};
use bytes::Bytes;
use dashmap::DashMap;
use futures::StreamExt;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_util::codec::FramedRead;

use super::{ConnectionInfo, HeartbeatManager, TunnelState};

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("Bind failed: {0}")]
    BindFailed(String),
    #[error("Accept failed: {0}")]
    AcceptFailed(String),
    #[error("Auth failed")]
    AuthFailed,
    #[error("Channel error: {0}")]
    ChannelError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Client session on server
pub struct ClientSession {
    pub addr: SocketAddr,
    pub crypto: Option<AesGcmCrypto>,
    pub channels: DashMap<u32, mpsc::Sender<Bytes>>,
}

/// Tunnel server - accepts connections from clients
pub struct TunnelServer {
    port: u16,
    password: String,
    state: Arc<AtomicU8>,
    auth: AuthManager,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl TunnelServer {
    pub fn new(port: u16, password: String) -> Self {
        let auth = AuthManager::new(&password);
        Self {
            port,
            password,
            state: Arc::new(AtomicU8::new(TunnelState::Disconnected as u8)),
            auth,
            shutdown_tx: None,
        }
    }

    /// Start the server
    pub async fn start(&mut self) -> Result<(), ServerError> {
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| ServerError::BindFailed(e.to_string()))?;

        self.state.store(TunnelState::Connected as u8, Ordering::SeqCst);
        tracing::info!("Tunnel server listening on {}", addr);

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        let password = self.password.clone();
        let auth = AuthManager::new(&password);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept_result = listener.accept() => {
                        match accept_result {
                            Ok((stream, addr)) => {
                                let password = password.clone();
                                let auth = AuthManager::new(&password);
                                tokio::spawn(async move {
                                    if let Err(e) = handle_client(stream, addr, auth).await {
                                        tracing::error!("Client {} error: {}", addr, e);
                                    }
                                });
                            }
                            Err(e) => {
                                tracing::error!("Accept error: {}", e);
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::info!("Server shutdown requested");
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop the server
    pub async fn stop(&mut self) -> Result<(), ServerError> {
        self.state.store(TunnelState::Disconnected as u8, Ordering::SeqCst);
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
        Ok(())
    }

    /// Check if server is running
    pub fn is_running(&self) -> bool {
        self.state() == TunnelState::Connected
    }

    fn state(&self) -> TunnelState {
        match self.state.load(Ordering::SeqCst) {
            0 => TunnelState::Disconnected,
            1 => TunnelState::Connecting,
            2 => TunnelState::Authenticating,
            3 => TunnelState::Connected,
            _ => TunnelState::Error,
        }
    }
}

/// Handle a client connection
async fn handle_client(
    mut stream: TcpStream,
    addr: SocketAddr,
    auth: AuthManager,
) -> Result<(), ServerError> {
    tracing::info!("New client connection from {}", addr);

    // Send auth challenge
    let nonce = auth.generate_nonce();
    stream.write_all(&nonce).await?;

    // Read auth response
    let mut response_buf = [0u8; 64];
    let n = stream.read(&mut response_buf).await?;

    // Verify response
    if !auth.verify_response(&nonce, &response_buf[..n]) {
        tracing::warn!("Auth failed for client {}", addr);
        return Err(ServerError::AuthFailed);
    }

    tracing::info!("Client {} authenticated successfully", addr);

    // Create crypto for this session
    let session_key = auth.generate_session_key(&nonce);
    let crypto = AesGcmCrypto::from_key(&session_key)
        .ok();

    // Main loop - handle frames
    let (reader, mut writer) = stream.into_split();
    let mut framed_reader = FramedRead::new(reader, FrameCodec::new());

    while let Some(frame_result) = framed_reader.next().await {
        match frame_result {
            Ok(frame) => {
                match frame.frame_type {
                    FrameType::Heartbeat => {
                        // Respond with heartbeat ack
                        let ack = Frame::heartbeat_ack();
                        // writer.write_all(&ack.encode()).await?;
                    }
                    FrameType::OpenChannel => {
                        // Parse target and open connection
                        if let Ok(payload) = OpenChannelPayload::decode(&frame.payload) {
                            tracing::debug!(
                                "Open channel {} -> {}:{}",
                                frame.channel_id,
                                payload.target_ip,
                                payload.target_port
                            );
                            // Handle channel open
                        }
                    }
                    FrameType::CloseChannel => {
                        tracing::debug!("Close channel {}", frame.channel_id);
                    }
                    FrameType::Data => {
                        // Forward data to target
                    }
                    _ => {}
                }
            }
            Err(e) => {
                tracing::error!("Frame error from {}: {}", addr, e);
                break;
            }
        }
    }

    tracing::info!("Client {} disconnected", addr);
    Ok(())
}
