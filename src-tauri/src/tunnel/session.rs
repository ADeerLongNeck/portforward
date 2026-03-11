use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Tunnel connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TunnelState {
    Disconnected,
    Connecting,
    Authenticating,
    Connected,
    Reconnecting,
    Error,
}

impl std::fmt::Display for TunnelState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TunnelState::Disconnected => write!(f, "Disconnected"),
            TunnelState::Connecting => write!(f, "Connecting"),
            TunnelState::Authenticating => write!(f, "Authenticating"),
            TunnelState::Connected => write!(f, "Connected"),
            TunnelState::Reconnecting => write!(f, "Reconnecting"),
            TunnelState::Error => write!(f, "Error"),
        }
    }
}

/// Connection information exposed to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub server_id: String,
    pub state: TunnelState,
    pub connected_at: Option<DateTime<Utc>>,
    pub latency_ms: Option<u64>,
    pub error_message: Option<String>,
}

impl ConnectionInfo {
    pub fn disconnected(server_id: &str) -> Self {
        Self {
            server_id: server_id.to_string(),
            state: TunnelState::Disconnected,
            connected_at: None,
            latency_ms: None,
            error_message: None,
        }
    }

    pub fn connecting(server_id: &str) -> Self {
        Self {
            server_id: server_id.to_string(),
            state: TunnelState::Connecting,
            connected_at: None,
            latency_ms: None,
            error_message: None,
        }
    }

    pub fn connected(server_id: &str) -> Self {
        Self {
            server_id: server_id.to_string(),
            state: TunnelState::Connected,
            connected_at: Some(Utc::now()),
            latency_ms: None,
            error_message: None,
        }
    }

    pub fn error(server_id: &str, message: &str) -> Self {
        Self {
            server_id: server_id.to_string(),
            state: TunnelState::Error,
            connected_at: None,
            latency_ms: None,
            error_message: Some(message.to_string()),
        }
    }
}

/// Active channel information
#[derive(Debug, Clone)]
pub struct ChannelInfo {
    pub channel_id: u32,
    pub target_ip: String,
    pub target_port: u16,
    pub created_at: DateTime<Utc>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

impl ChannelInfo {
    pub fn new(channel_id: u32, target_ip: String, target_port: u16) -> Self {
        Self {
            channel_id,
            target_ip,
            target_port,
            created_at: Utc::now(),
            bytes_sent: 0,
            bytes_received: 0,
        }
    }
}
