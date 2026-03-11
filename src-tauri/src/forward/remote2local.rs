use crate::config::Remote2LocalRule;
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

/// Remote to local port forwarder
pub struct Remote2LocalForwarder {
    rule: Remote2LocalRule,
}

impl Remote2LocalForwarder {
    pub fn new(rule: Remote2LocalRule) -> Self {
        Self { rule }
    }

    /// Start listening for connections from remote tunnel
    pub async fn start(self) -> Result<(), ForwardError> {
        tracing::info!(
            "R2L forwarder registered: {} -> {}:{}",
            self.rule.remote_port,
            self.rule.local_ip,
            self.rule.local_port
        );

        // This would be triggered by the tunnel when it receives a connection
        // on the remote port

        Ok(())
    }
}
