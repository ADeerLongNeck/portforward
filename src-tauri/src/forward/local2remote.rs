use crate::config::Local2RemoteRule;
use crate::tunnel::TunnelClient;
use std::sync::Arc;
use thiserror::Error;
use tokio::net::TcpListener;

#[derive(Debug, Error)]
pub enum ForwardError {
    #[error("Bind failed: {0}")]
    BindFailed(String),
    #[error("Tunnel error: {0}")]
    TunnelError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Local to remote port forwarder
pub struct Local2RemoteForwarder {
    rule: Local2RemoteRule,
    tunnel: Arc<TunnelClient>,
}

impl Local2RemoteForwarder {
    pub fn new(rule: Local2RemoteRule, tunnel: Arc<TunnelClient>) -> Self {
        Self { rule, tunnel }
    }

    /// Start listening and forwarding
    pub async fn start(self) -> Result<(), ForwardError> {
        let addr = format!("0.0.0.0:{}", self.rule.local_port);
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| ForwardError::BindFailed(e.to_string()))?;

        tracing::info!(
            "L2R forwarder started: {} -> {}:{}",
            self.rule.local_port,
            self.rule.remote_ip,
            self.rule.remote_port
        );

        loop {
            let (socket, peer_addr) = listener.accept().await?;
            let tunnel = self.tunnel.clone();
            let remote_ip = self.rule.remote_ip.clone();
            let remote_port = self.rule.remote_port;

            tracing::debug!("New connection from {}", peer_addr);

            tokio::spawn(async move {
                if let Err(e) = handle_connection(socket, tunnel, &remote_ip, remote_port).await {
                    tracing::error!("Connection error from {}: {}", peer_addr, e);
                }
            });
        }
    }
}

async fn handle_connection(
    mut socket: tokio::net::TcpStream,
    tunnel: Arc<TunnelClient>,
    remote_ip: &str,
    remote_port: u16,
) -> Result<(), ForwardError> {
    // Open channel through tunnel
    let channel_id = tunnel
        .open_channel(remote_ip, remote_port)
        .await
        .map_err(|e| ForwardError::TunnelError(e.to_string()))?;

    tracing::debug!("Opened channel {} -> {}:{}", channel_id, remote_ip, remote_port);

    // TODO: Implement bidirectional forwarding

    Ok(())
}
