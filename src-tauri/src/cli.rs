//! CLI mode for port forwarding

use clap::{Parser, Subcommand};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::signal;
use tokio::sync::{Mutex, RwLock, mpsc};

use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Framed;

use crate::protocol::{Frame, FrameCodec, FrameType, OpenChannelPayload};

// ============== Shared Types ==============

/// Connection status
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

/// Forwarded port status
#[derive(Debug, Clone)]
pub struct ForwardedPort {
    pub port: u16,
    pub status: String,
    pub connections: u32,
    pub upload: u64,
    pub download: u64,
}

/// Stats state
pub struct StatsState {
    pub upload: Arc<RwLock<u64>>,
    pub download: Arc<RwLock<u64>>,
    pub connections: Arc<RwLock<u64>>,
}

impl StatsState {
    pub fn new() -> Self {
        Self {
            upload: Arc::new(RwLock::new(0)),
            download: Arc::new(RwLock::new(0)),
            connections: Arc::new(RwLock::new(0)),
        }
    }

    pub async fn add_upload(&self, bytes: u64) {
        let mut u = self.upload.write().await;
        *u += bytes;
    }

    pub async fn add_download(&self, bytes: u64) {
        let mut d = self.download.write().await;
        *d += bytes;
    }

    pub fn inc_connections(&self) {
        let conn = self.connections.clone();
        tokio::spawn(async move {
            let mut c = conn.write().await;
            *c += 1;
        });
    }

    pub fn dec_connections(&self) {
        let conn = self.connections.clone();
        tokio::spawn(async move {
            let mut c = conn.write().await;
            *c = c.saturating_sub(1);
        });
    }
}

/// Channel state for server mode
pub struct TunnelChannel {
    pub local_tx: mpsc::Sender<Bytes>,
    pub close_tx: Option<mpsc::Sender<()>>,
}

/// Tunnel state for server mode
pub struct TunnelState {
    pub tunnel_tx: mpsc::Sender<Frame>,
    pub channels: HashMap<u32, TunnelChannel>,
    pub next_channel_id: u32,
}

/// Channel state for client mode
pub struct ClientChannel {
    pub target_tx: mpsc::Sender<Bytes>,
    pub close_tx: Option<mpsc::Sender<()>>,
}

/// Runtime state
pub struct RuntimeState {
    pub status: Arc<RwLock<ConnectionStatus>>,
    pub is_server: Arc<RwLock<bool>>,
    pub error_message: Arc<RwLock<Option<String>>>,
    pub server_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    pub client_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    pub forward_handles: Arc<Mutex<HashMap<u16, tokio::task::JoinHandle<()>>>>,
    pub forward_stats: Arc<RwLock<HashMap<u16, ForwardedPort>>>,
    pub tunnel_state: Arc<Mutex<Option<Arc<Mutex<TunnelState>>>>>,
}

impl RuntimeState {
    pub fn new() -> Self {
        Self {
            status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            is_server: Arc::new(RwLock::new(false)),
            error_message: Arc::new(RwLock::new(None)),
            server_handle: Arc::new(Mutex::new(None)),
            client_handle: Arc::new(Mutex::new(None)),
            forward_handles: Arc::new(Mutex::new(HashMap::new())),
            forward_stats: Arc::new(RwLock::new(HashMap::new())),
            tunnel_state: Arc::new(Mutex::new(None)),
        }
    }
}

// ============== CLI Definition ==============

#[derive(Parser)]
#[command(name = "port-forward")]
#[command(about = "A small, fast, secure cross-platform port forwarding tool")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start in server mode (listen for tunnel clients)
    Server {
        /// Port to listen on for tunnel connections
        #[arg(short, long, default_value = "5173")]
        port: u16,

        /// Password for authentication
        #[arg(short = 'P', long)]
        password: String,

        /// Local ports to forward (comma separated, e.g., "1080,3389")
        #[arg(short = 'f', long, default_value = "")]
        forward: String,
    },

    /// Start in client mode (connect to tunnel server)
    Client {
        /// Server host to connect to
        #[arg(short = 'H', long)]
        host: String,

        /// Server port to connect to
        #[arg(short = 'p', long, default_value = "5173")]
        port: u16,

        /// Password for authentication
        #[arg(short = 'P', long)]
        password: String,
    },
}

pub async fn run_cli() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let runtime_state = Arc::new(RuntimeState::new());
    let stats_state = Arc::new(StatsState::new());

    match cli.command {
        Commands::Server { port, password, forward } => {
            let forward_ports: Vec<u16> = forward
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();

            tracing::info!("Starting server on port {} with forward ports: {:?}", port, forward_ports);

            start_server_standalone(port, forward_ports.clone(), password, runtime_state.clone(), stats_state.clone()).await?;

            println!("Server started on port {}", port);
            println!("Forward ports: {:?}", forward_ports);
            println!("Press Ctrl+C to stop...");

            // Wait for shutdown signal
            match signal::ctrl_c().await {
                Ok(()) => {
                    tracing::info!("Received shutdown signal");
                }
                Err(err) => {
                    tracing::error!("Unable to listen for shutdown signal: {}", err);
                }
            }

            // Stop server
            stop_server_standalone(runtime_state).await?;
        }
        Commands::Client { host, port, password } => {
            tracing::info!("Connecting to server {}:{}", host, port);

            // Start client
            start_client_standalone(host.clone(), port, password, runtime_state.clone()).await?;

            println!("Connected to {}:{}", host, port);
            println!("Press Ctrl+C to stop...");

            // Wait for shutdown signal
            match signal::ctrl_c().await {
                Ok(()) => {
                    tracing::info!("Received shutdown signal");
                }
                Err(err) => {
                    tracing::error!("Unable to listen for shutdown signal: {}", err);
                }
            }

            // Stop client
            stop_client_standalone(runtime_state).await?;
        }
    }

    Ok(())
}

// ============== Server Mode ===============

async fn start_server_standalone(
    port: u16,
    forward_ports: Vec<u16>,
    _password: String,
    state: Arc<RuntimeState>,
    stats_state: Arc<StatsState>,
) -> Result<(), String> {
    // Check if already running
    {
        let status = state.status.read().await;
        if *status == ConnectionStatus::Connected {
            return Err("服务已在运行".to_string());
        }
    }

    tracing::info!("Starting server on port {}, forward ports: {:?}", port, forward_ports);

    // Update status
    {
        let mut status = state.status.write().await;
        let mut is_server = state.is_server.write().await;
        let mut error = state.error_message.write().await;
        *status = ConnectionStatus::Connecting;
        *is_server = true;
        *error = None;
    }

    // Clear tunnel state
    {
        let mut tunnel_state = state.tunnel_state.lock().await;
        *tunnel_state = None;
    }

    let status_clone = state.status.clone();
    let error_clone = state.error_message.clone();
    let tunnel_state_clone = state.tunnel_state.clone();
    let forward_stats_clone = state.forward_stats.clone();
    let stats_for_tunnel = stats_state.clone();
    let tunnel_port = port;

    let server_handle = tokio::spawn(async move {
        let addr = format!("0.0.0.0:{}", tunnel_port);

        match TcpListener::bind(&addr).await {
            Ok(listener) => {
                tracing::info!("Tunnel server listening on {}", addr);

                {
                    let mut status = status_clone.write().await;
                    *status = ConnectionStatus::Connected;
                }

                loop {
                    match listener.accept().await {
                        Ok((stream, client_addr)) => {
                            tracing::info!("New tunnel client from {}", client_addr);

                            stats_for_tunnel.inc_connections();

                            let status_clone2 = status_clone.clone();
                            let tunnel_state_clone2 = tunnel_state_clone.clone();
                            let forward_stats_clone2 = forward_stats_clone.clone();
                            let stats_for_tunnel_clone = stats_for_tunnel.clone();

                            tokio::spawn(async move {
                                if let Err(e) = handle_tunnel_server_standalone(
                                    stream,
                                    tunnel_state_clone2,
                                    forward_stats_clone2,
                                    stats_for_tunnel_clone.clone(),
                                ).await {
                                    tracing::error!("Tunnel client {} error: {}", client_addr, e);
                                }

                                stats_for_tunnel_clone.dec_connections();

                                let mut status = status_clone2.write().await;
                                *status = ConnectionStatus::Disconnected;
                            });
                        }
                        Err(e) => {
                            tracing::error!("Accept error: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to bind port {}: {}", tunnel_port, e);
                let mut status = status_clone.write().await;
                let mut error = error_clone.write().await;
                *status = ConnectionStatus::Error;
                *error = Some(format!("无法绑定端口 {}: {}", tunnel_port, e));
            }
        }
    });

    {
        let mut handle = state.server_handle.lock().await;
        *handle = Some(server_handle);
    }

    // Start local forward ports
    let forward_handles = state.forward_handles.clone();
    let forward_stats = state.forward_stats.clone();
    let tunnel_state = state.tunnel_state.clone();
    let stats_for_forward = stats_state.clone();

    for &fwd_port in &forward_ports {
        start_local_forwarder_standalone(
            fwd_port,
            forward_handles.clone(),
            forward_stats.clone(),
            tunnel_state.clone(),
            stats_for_forward.clone(),
        ).await;
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let status = state.status.read().await;
    if *status == ConnectionStatus::Connected {
        tracing::info!("Server started successfully");
        Ok(())
    } else {
        let error = state.error_message.read().await;
        Err(error.clone().unwrap_or_else(|| "启动失败".to_string()))
    }
}

async fn handle_tunnel_server_standalone(
    stream: TcpStream,
    tunnel_state: Arc<Mutex<Option<Arc<Mutex<TunnelState>>>>>,
    forward_stats: Arc<RwLock<HashMap<u16, ForwardedPort>>>,
    stats_state: Arc<StatsState>,
) -> Result<(), std::io::Error> {
    let framed = Framed::new(stream, FrameCodec::new());
    let (mut sink, mut stream) = framed.split();

    let (tunnel_tx, mut tunnel_rx) = mpsc::channel::<Frame>(256);

    let state = Arc::new(Mutex::new(TunnelState {
        tunnel_tx: tunnel_tx.clone(),
        channels: HashMap::new(),
        next_channel_id: 1,
    }));

    {
        let mut ts = tunnel_state.lock().await;
        *ts = Some(state.clone());
        tracing::info!("✓ Tunnel state stored - forward ports can now accept connections");
    }

    tracing::info!("Tunnel server connection established, ready to forward traffic");

    let send_task = async move {
        while let Some(frame) = tunnel_rx.recv().await {
            if sink.send(frame).await.is_err() {
                break;
            }
        }
    };

    let state_clone = state.clone();
    let forward_stats_clone = forward_stats.clone();
    let stats_for_recv = stats_state.clone();
    let recv_task = async move {
        while let Some(frame_result) = stream.next().await {
            match frame_result {
                Ok(frame) => {
                    if let Err(e) = handle_tunnel_frame_standalone(&frame, &state_clone, &forward_stats_clone, &stats_for_recv).await {
                        tracing::error!("Error handling frame: {}", e);
                    }
                }
                Err(e) => {
                    tracing::error!("Frame decode error: {}", e);
                    break;
                }
            }
        }
    };

    // Wait for either task to complete
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    // Clear tunnel state
    {
        let mut ts = tunnel_state.lock().await;
        *ts = None;
    }

    tracing::info!("Tunnel server connection closed");
    Ok(())
}

async fn handle_tunnel_frame_standalone(
    frame: &Frame,
    tunnel_state: &Arc<Mutex<TunnelState>>,
    _forward_stats: &Arc<RwLock<HashMap<u16, ForwardedPort>>>,
    stats_state: &Arc<StatsState>,
) -> Result<(), String> {
    match frame.frame_type {
        FrameType::Data => {
            let state = tunnel_state.lock().await;
            if let Some(channel) = state.channels.get(&frame.channel_id) {
                let _ = channel.local_tx.send(frame.payload.clone()).await;
                let stats = stats_state.clone();
                let len = frame.payload.len() as u64;
                tokio::spawn(async move {
                    stats.add_download(len).await;
                });
            }
        }
        FrameType::CloseChannel => {
            let mut state = tunnel_state.lock().await;
            if let Some(channel) = state.channels.remove(&frame.channel_id) {
                if let Some(close_tx) = &channel.close_tx {
                    let _ = close_tx.send(()).await;
                }
            }
        }
        FrameType::Heartbeat => {
            let state = tunnel_state.lock().await;
            let _ = state.tunnel_tx.send(Frame::heartbeat_ack()).await;
        }
        FrameType::HeartbeatAck => {}
        _ => {}
    }
    Ok(())
}

async fn start_local_forwarder_standalone(
    port: u16,
    forward_handles: Arc<Mutex<HashMap<u16, tokio::task::JoinHandle<()>>>>,
    forward_stats: Arc<RwLock<HashMap<u16, ForwardedPort>>>,
    tunnel_state: Arc<Mutex<Option<Arc<Mutex<TunnelState>>>>>,
    stats_state: Arc<StatsState>,
) {
    tracing::info!("Starting local forwarder for port {}", port);

    // Initialize stats
    {
        let mut stats = forward_stats.write().await;
        stats.insert(port, ForwardedPort {
            port,
            status: "starting".to_string(),
            connections: 0,
            upload: 0,
            download: 0,
        });
    }

    let handle = tokio::spawn(async move {
        let addr = format!("127.0.0.1:{}", port);

        match TcpListener::bind(&addr).await {
            Ok(listener) => {
                tracing::info!("✓ Local forward port {} successfully listening on {}", port, addr);

                {
                    let mut stats = forward_stats.write().await;
                    if let Some(fp) = stats.get_mut(&port) {
                        fp.status = "listening".to_string();
                    }
                }

                loop {
                    match listener.accept().await {
                        Ok((local_stream, peer)) => {
                            tracing::info!("→ New connection on forward port {} from {}", port, peer);

                            {
                                let mut stats = forward_stats.write().await;
                                if let Some(fp) = stats.get_mut(&port) {
                                    fp.connections += 1;
                                }
                            }

                            stats_state.inc_connections();

                            let tunnel_state_clone = tunnel_state.clone();
                            let forward_stats_for_conn = forward_stats.clone();
                            let forward_stats_for_dec = forward_stats.clone();
                            let port_clone = port;
                            let stats_for_conn = stats_state.clone();

                            tokio::spawn(async move {
                                if let Err(e) = handle_local_forward_connection_standalone(
                                    local_stream,
                                    port_clone,
                                    tunnel_state_clone,
                                    forward_stats_for_conn,
                                    stats_for_conn.clone(),
                                ).await {
                                    tracing::error!("Forward connection error on port {}: {}", port_clone, e);
                                }

                                {
                                    let mut stats = forward_stats_for_dec.write().await;
                                    if let Some(fp) = stats.get_mut(&port_clone) {
                                        fp.connections = fp.connections.saturating_sub(1);
                                    }
                                }

                                stats_for_conn.dec_connections();
                            });
                        }
                        Err(e) => {
                            tracing::error!("Accept error on port {}: {}", port, e);
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to bind port {}: {}", port, e);

                {
                    let mut stats = forward_stats.write().await;
                    if let Some(fp) = stats.get_mut(&port) {
                        fp.status = format!("error: {}", e);
                    }
                }
            }
        }
    });

    {
        let mut handles = forward_handles.lock().await;
        handles.insert(port, handle);
    }
}

async fn handle_local_forward_connection_standalone(
    mut local_stream: TcpStream,
    forward_port: u16,
    tunnel_state: Arc<Mutex<Option<Arc<Mutex<TunnelState>>>>>,
    forward_stats: Arc<RwLock<HashMap<u16, ForwardedPort>>>,
    stats_state: Arc<StatsState>,
) -> Result<(), std::io::Error> {
    // Handle SOCKS5 handshake to get target address
    let (target_ip, target_port) = handle_socks5_handshake_standalone(&mut local_stream).await?;

    tracing::info!("SOCKS5 target: {}:{}", target_ip, target_port);

    // Wait for tunnel connection
    let tunnel = {
        let mut attempts = 0;
        loop {
            let ts = tunnel_state.lock().await;
            if let Some(t) = ts.clone() {
                tracing::info!("✓ Tunnel found for forward port {}", forward_port);
                break t;
            }
            drop(ts); // Release lock before sleeping

            attempts += 1;
            if attempts % 10 == 0 {
                tracing::info!("Waiting for tunnel connection... (attempt {}/50)", attempts);
            }

            if attempts > 50 {
                // 5 seconds timeout
                tracing::warn!("✗ No tunnel connection for forward port {} after 5s timeout", forward_port);
                send_socks5_response_standalone(&mut local_stream, false).await?;
                return Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "No tunnel connection - timeout"));
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    };

    // Allocate channel ID and create channel state
    let (local_tx, mut local_rx) = mpsc::channel::<Bytes>(256);
    let (close_tx, mut close_rx) = mpsc::channel::<()>(1);

    let channel_id = {
        let mut state = tunnel.lock().await;
        let id = state.next_channel_id;
        state.next_channel_id += 1;

        state.channels.insert(id, TunnelChannel {
            local_tx: local_tx.clone(),
            close_tx: Some(close_tx),
        });

        id
    };

    tracing::info!("Opened channel {} for {}:{} through tunnel", channel_id, target_ip, target_port);

    // Send OpenChannel frame to tunnel client with the actual target
    let open_payload = OpenChannelPayload::new(target_ip.clone(), target_port);

    {
        let state = tunnel.lock().await;
        if state.tunnel_tx.send(Frame::open_channel(
            channel_id,
            Bytes::from(open_payload.encode()),
        )).await.is_err() {
            send_socks5_response_standalone(&mut local_stream, false).await?;
            return Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Tunnel send failed"));
        }
    }

    // Wait a bit for the client to connect to the target
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Send SOCKS5 success response
    send_socks5_response_standalone(&mut local_stream, true).await?;

    tracing::info!("SOCKS5 connection established: channel {} -> {}:{}", channel_id, target_ip, target_port);

    // Split local stream for bidirectional forwarding
    let (mut local_reader, mut local_writer) = local_stream.split();

    // Task: Read from local, send Data frames to tunnel
    let tunnel_for_send = tunnel.clone();
    let forward_stats_send = forward_stats.clone();
    let stats_for_upload = stats_state.clone();
    let channel_id_for_task = channel_id;
    let forward_port_for_task = forward_port;
    let send_task = async move {
        let mut buf = [0u8; 32 * 1024];
        loop {
            match local_reader.read(&mut buf).await {
                Ok(0) => {
                    tracing::debug!("SOCKS5 client EOF for channel {}", channel_id_for_task);
                    break;
                }
                Ok(n) => {
                    let data = Bytes::copy_from_slice(&buf[..n]);

                    {
                        let mut stats = forward_stats_send.write().await;
                        if let Some(fp) = stats.get_mut(&forward_port_for_task) {
                            fp.upload += n as u64;
                        }
                    }

                    stats_for_upload.add_upload(n as u64);

                    let state = tunnel_for_send.lock().await;
                    if state.tunnel_tx.send(Frame::data(channel_id_for_task, data)).await.is_err() {
                        tracing::error!("Failed to send data frame for channel {}", channel_id_for_task);
                        break;
                    }
                    tracing::trace!("Sent {} bytes to tunnel for channel {}", n, channel_id_for_task);
                }
                Err(_) => break,
            }
        }

        // Send CloseChannel when local stream ends
        let state = tunnel_for_send.lock().await;
        let _ = state.tunnel_tx.send(Frame::close_channel(channel_id_for_task)).await;
    };

    // Task: Receive from tunnel, write to local
    let stats_for_download = stats_state.clone();
    let forward_port_for_recv = forward_port;
    let recv_task = async move {
        while let Some(data) = local_rx.recv().await {
            if local_writer.write_all(&data).await.is_err() {
                break;
            }

            {
                let mut stats = forward_stats.write().await;
                if let Some(fp) = stats.get_mut(&forward_port_for_recv) {
                    fp.download += data.len() as u64;
                }
            }

            stats_for_download.add_download(data.len() as u64);
        }
    };

    // Wait for completion or close signal
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
        _ = close_rx.recv() => {},
    }

    // Cleanup channel
    {
        let mut state = tunnel.lock().await;
        state.channels.remove(&channel_id);
    }

    tracing::info!("SOCKS5 channel {} closed", channel_id);
    Ok(())
}

// SOCKS5 helper functions
const SOCKS5_VERSION: u8 = 0x05;
const SOCKS5_NO_AUTH: u8 = 0x00;
const SOCKS5_CMD_CONNECT: u8 = 0x01;
const SOCKS5_ATYP_IPV4: u8 = 0x01;
const SOCKS5_ATYP_DOMAIN: u8 = 0x03;
const SOCKS5_ATYP_IPV6: u8 = 0x04;
const SOCKS5_REP_SUCCESS: u8 = 0x00;
const SOCKS5_REP_GENERAL_FAILURE: u8 = 0x01;

async fn handle_socks5_handshake_standalone(
    stream: &mut TcpStream,
) -> Result<(String, u16), std::io::Error> {
    let mut buf = [0u8; 257];
    stream.read_exact(&mut buf[..2]).await?;

    if buf[0] != SOCKS5_VERSION {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid SOCKS version"));
    }

    let nmethods = buf[1] as usize;
    stream.read_exact(&mut buf[..nmethods]).await?;

    let has_no_auth = buf[..nmethods].contains(&SOCKS5_NO_AUTH);

    if !has_no_auth {
        stream.write_all(&[SOCKS5_VERSION, 0xFF]).await?;
        return Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "No supported auth method"));
    }

    stream.write_all(&[SOCKS5_VERSION, SOCKS5_NO_AUTH]).await?;

    stream.read_exact(&mut buf[..4]).await?;

    if buf[0] != SOCKS5_VERSION || buf[1] != SOCKS5_CMD_CONNECT {
        return Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "Invalid request"));
    }

    let atyp = buf[3];
    let (target_ip, target_port) = match atyp {
        SOCKS5_ATYP_IPV4 => {
            stream.read_exact(&mut buf[..6]).await?;
            let ip = format!("{}.{}.{}.{}", buf[0], buf[1], buf[2], buf[3]);
            let port = u16::from_be_bytes([buf[4], buf[5]]);
            (ip, port)
        }
        SOCKS5_ATYP_DOMAIN => {
            stream.read_exact(&mut buf[..1]).await?;
            let domain_len = buf[0] as usize;
            stream.read_exact(&mut buf[..domain_len + 2]).await?;
            let domain = String::from_utf8_lossy(&buf[..domain_len]).to_string();
            let port = u16::from_be_bytes([buf[domain_len], buf[domain_len + 1]]);
            (domain, port)
        }
        SOCKS5_ATYP_IPV6 => {
            stream.read_exact(&mut buf[..18]).await?;
            let mut ip_parts = Vec::new();
            for i in (0..16).step_by(2) {
                ip_parts.push(format!("{:02x}{:02x}", buf[i], buf[i + 1]));
            }
            let ip = ip_parts.join(":");
            let port = u16::from_be_bytes([buf[16], buf[17]]);
            (ip, port)
        }
        _ => return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Unsupported address type")),
    };

    tracing::info!("SOCKS5 connect request: {}:{}", target_ip, target_port);
    Ok((target_ip, target_port))
}

async fn send_socks5_response_standalone(
    stream: &mut TcpStream,
    success: bool,
) -> Result<(), std::io::Error> {
    let reply = if success { SOCKS5_REP_SUCCESS } else { SOCKS5_REP_GENERAL_FAILURE };
    stream.write_all(&[
        SOCKS5_VERSION, reply, 0x00, SOCKS5_ATYP_IPV4,
        0, 0, 0, 0, 0, 0,
    ]).await?;
    Ok(())
}

// ============== Client Mode ===============
async fn start_client_standalone(
    host: String,
    port: u16,
    _password: String,
    state: Arc<RuntimeState>,
) -> Result<(), String> {
    {
        let status = state.status.read().await;
        if *status == ConnectionStatus::Connected {
            return Err("客户端已在运行".to_string());
        }
    }

    tracing::info!("Starting client, connecting to {}:{}", host, port);

    {
        let mut status = state.status.write().await;
        let mut is_server = state.is_server.write().await;
        let mut error = state.error_message.write().await;
        *status = ConnectionStatus::Connecting;
        *is_server = false;
        *error = None;
    }

    let addr = format!("{}:{}", host, port);
    let status_clone = state.status.clone();

    let handle = tokio::spawn(async move {
        loop {
            match TcpStream::connect(&addr).await {
                Ok(stream) => {
                    tracing::info!("Connected to server {}", addr);

                    {
                        let mut status = status_clone.write().await;
                        *status = ConnectionStatus::Connected;
                    }

                    if let Err(e) = handle_client_tunnel_standalone(stream).await {
                        tracing::error!("Connection error: {}", e);
                    }

                    tracing::info!("Disconnected from server, will retry...");
                }
                Err(e) => {
                    tracing::error!("Failed to connect to {}: {}", addr, e);
                }
            }

            // Reset status on disconnect
            {
                let mut status = status_clone.write().await;
                *status = ConnectionStatus::Connecting;
            }

            // Wait before reconnecting
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    });

    {
        let mut client_handle = state.client_handle.lock().await;
        *client_handle = Some(handle);
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let status = state.status.read().await;
    if *status == ConnectionStatus::Connected {
        tracing::info!("Client connected successfully");
        Ok(())
    } else {
        Err("连接失败".to_string())
    }
}

async fn handle_client_tunnel_standalone(stream: TcpStream) -> Result<(), std::io::Error> {
    let framed = Framed::new(stream, FrameCodec::new());
    let (mut sink, mut stream) = framed.split();

    let (tunnel_tx, mut tunnel_rx) = mpsc::channel::<Frame>(256);
    let channels: Arc<Mutex<HashMap<u32, ClientChannel>>> = Arc::new(Mutex::new(HashMap::new()));

    // Heartbeat task
    let tunnel_tx_heartbeat = tunnel_tx.clone();
    let heartbeat_task = async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            if tunnel_tx_heartbeat.send(Frame::heartbeat()).await.is_err() {
                break;
            }
        }
    };

    // Send task
    let send_task = async move {
        while let Some(frame) = tunnel_rx.recv().await {
            if sink.send(frame).await.is_err() {
                break;
            }
        }
    };

    // Receive task
    let tunnel_tx_recv = tunnel_tx.clone();
    let channels_recv = channels.clone();
    let recv_task = async move {
        while let Some(frame_result) = stream.next().await {
            match frame_result {
                Ok(frame) => {
                    match frame.frame_type {
                        FrameType::OpenChannel => {
                            match OpenChannelPayload::decode(&frame.payload) {
                                Ok(payload) => {
                                    let tunnel_tx_clone = tunnel_tx_recv.clone();
                                    let channels_clone = channels_recv.clone();
                                    let channel_id = frame.channel_id;
                                    let target_ip = payload.target_ip;
                                    let target_port = payload.target_port;

                                    tokio::spawn(async move {
                                        handle_client_open_channel_standalone(
                                            channel_id,
                                            target_ip,
                                            target_port,
                                            tunnel_tx_clone,
                                            channels_clone,
                                        ).await;
                                    });
                                }
                                Err(e) => {
                                    tracing::error!("Failed to decode OpenChannel payload: {}", e);
                                    let _ = tunnel_tx_recv.send(Frame::close_channel(frame.channel_id)).await;
                                }
                            }
                        }
                        FrameType::Data => {
                            let channels = channels_recv.lock().await;
                            if let Some(channel) = channels.get(&frame.channel_id) {
                                let _ = channel.target_tx.send(frame.payload.clone()).await;
                            }
                        }
                        FrameType::CloseChannel => {
                            let mut channels = channels_recv.lock().await;
                            if let Some(channel) = channels.remove(&frame.channel_id) {
                                if let Some(close_tx) = &channel.close_tx {
                                    let _ = close_tx.send(()).await;
                                }
                            }
                        }
                        FrameType::Heartbeat => {
                            let _ = tunnel_tx_recv.send(Frame::heartbeat_ack()).await;
                        }
                        FrameType::HeartbeatAck => {}
                        _ => {}
                    }
                }
                Err(e) => {
                    tracing::error!("Frame decode error: {}", e);
                    break;
                }
            }
        }
    };

    tokio::select! {
        _ = heartbeat_task => {},
        _ = send_task => {},
        _ = recv_task => {},
    }

    // Close all channels
    let mut channels = channels.lock().await;
    for (_, channel) in channels.drain() {
        if let Some(close_tx) = &channel.close_tx {
            let _ = close_tx.send(()).await;
        }
    }

    Ok(())
}

async fn handle_client_open_channel_standalone(
    channel_id: u32,
    target_ip: String,
    target_port: u16,
    tunnel_tx: mpsc::Sender<Frame>,
    channels: Arc<Mutex<HashMap<u32, ClientChannel>>>,
) {
    let target_addr = format!("{}:{}", target_ip, target_port);

    tracing::info!("Connecting to target {} for channel {}", target_addr, channel_id);

    let target_stream = match TcpStream::connect(&target_addr).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to connect to target {}: {}", target_addr, e);
            let _ = tunnel_tx.send(Frame::close_channel(channel_id)).await;
            return;
        }
    };

    tracing::info!("Connected to target {} for channel {}", target_addr, channel_id);

    let (target_tx, mut target_rx) = mpsc::channel::<Bytes>(256);
    let (close_tx, mut close_rx) = mpsc::channel::<()>(1);

    // Register channel
    {
        let mut ch = channels.lock().await;
        ch.insert(channel_id, ClientChannel {
            target_tx: target_tx.clone(),
            close_tx: Some(close_tx),
        });
    }

    let (mut target_reader, mut target_writer) = target_stream.into_split();

    // Read from target, send to tunnel
    let tunnel_tx_read = tunnel_tx.clone();
    let read_task = async move {
        let mut buf = [0u8; 32 * 1024];
        loop {
            match target_reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    let data = Bytes::copy_from_slice(&buf[..n]);
                    if tunnel_tx_read.send(Frame::data(channel_id, data)).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = tunnel_tx_read.send(Frame::close_channel(channel_id)).await;
    };

    // Write to target from tunnel
    let write_task = async move {
        while let Some(data) = target_rx.recv().await {
            if target_writer.write_all(&data).await.is_err() {
                break;
            }
        }
    };

    tokio::select! {
        _ = read_task => {},
        _ = write_task => {},
        _ = close_rx.recv() => {},
    }

    // Remove channel
    {
        let mut ch = channels.lock().await;
        ch.remove(&channel_id);
    }

    tracing::info!("Channel {} to target {} closed", channel_id, target_addr);
}

async fn stop_server_standalone(state: Arc<RuntimeState>) -> Result<(), String> {
    tracing::info!("Stopping server");

    {
        let mut handle = state.server_handle.lock().await;
        if let Some(h) = handle.take() {
            h.abort();
        }
    }

    {
        let mut handles = state.forward_handles.lock().await;
        for (_, h) in handles.drain() {
            h.abort();
        }
    }

    {
        let mut tunnel_state = state.tunnel_state.lock().await;
        *tunnel_state = None;
    }

    {
        let mut stats = state.forward_stats.write().await;
        stats.clear();
    }

    {
        let mut status = state.status.write().await;
        *status = ConnectionStatus::Disconnected;
    }

    tracing::info!("Server stopped");
    Ok(())
}

async fn stop_client_standalone(state: Arc<RuntimeState>) -> Result<(), String> {
    tracing::info!("Stopping client");

    {
        let mut handle = state.client_handle.lock().await;
        if let Some(h) = handle.take() {
            h.abort();
        }
    }

    {
        let mut status = state.status.write().await;
        *status = ConnectionStatus::Disconnected;
    }

    tracing::info!("Client stopped");
    Ok(())
}
