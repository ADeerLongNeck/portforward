use crate::config::Socks5Config;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Debug, Error)]
pub enum Socks5Error {
    #[error("Bind failed: {0}")]
    BindFailed(String),
    #[error("Invalid SOCKS5 version")]
    InvalidVersion,
    #[error("No acceptable auth method")]
    NoAcceptableMethod,
    #[error("Auth failed")]
    AuthFailed,
    #[error("Unsupported command")]
    UnsupportedCommand,
    #[error("Unsupported address type")]
    UnsupportedAddressType,
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// SOCKS5 proxy server
pub struct Socks5Proxy {
    config: Socks5Config,
}

impl Socks5Proxy {
    pub fn new(config: Socks5Config) -> Self {
        Self { config }
    }

    /// Start the SOCKS5 proxy server
    pub async fn start(self) -> Result<(), Socks5Error> {
        if !self.config.enabled {
            tracing::info!("SOCKS5 proxy is disabled");
            return Ok(());
        }

        let addr = format!("0.0.0.0:{}", self.config.port);
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| Socks5Error::BindFailed(e.to_string()))?;

        tracing::info!("SOCKS5 proxy listening on {}", addr);

        loop {
            let (socket, peer_addr) = listener.accept().await?;
            let config = self.config.clone();

            tracing::debug!("SOCKS5 connection from {}", peer_addr);

            tokio::spawn(async move {
                if let Err(e) = handle_socks5_connection(socket, &config).await {
                    tracing::error!("SOCKS5 error from {}: {}", peer_addr, e);
                }
            });
        }
    }
}

async fn handle_socks5_connection(
    mut socket: tokio::net::TcpStream,
    config: &Socks5Config,
) -> Result<(), Socks5Error> {
    let mut buf = [0u8; 257];

    // Read client greeting
    let n = socket.read(&mut buf).await?;
    if n < 2 || buf[0] != 0x05 {
        return Err(Socks5Error::InvalidVersion);
    }

    let num_methods = buf[1] as usize;
    if n < 2 + num_methods {
        return Err(Socks5Error::NoAcceptableMethod);
    }

    // Check if auth is required
    let requires_auth = config.username.is_some() && config.password.is_some();

    let selected_method = if requires_auth {
        let methods = &buf[2..2 + num_methods];
        if methods.contains(&0x02) { 0x02 } else { 0xFF }
    } else {
        0x00
    };

    // Send method selection
    socket.write_all(&[0x05, selected_method]).await?;

    if selected_method == 0xFF {
        return Err(Socks5Error::NoAcceptableMethod);
    }

    // Handle authentication if required
    if selected_method == 0x02 {
        let n = socket.read(&mut buf).await?;
        if n < 2 || buf[0] != 0x01 {
            return Err(Socks5Error::AuthFailed);
        }

        let ulen = buf[1] as usize;
        if n < 2 + ulen {
            return Err(Socks5Error::AuthFailed);
        }

        let username = String::from_utf8_lossy(&buf[2..2 + ulen]).to_string();
        let plen = buf[2 + ulen] as usize;
        let password = String::from_utf8_lossy(&buf[3 + ulen..3 + ulen + plen]).to_string();

        let expected_user = config.username.as_deref().unwrap_or("");
        let expected_pass = config.password.as_deref().unwrap_or("");

        if username != expected_user || password != expected_pass {
            socket.write_all(&[0x01, 0x01]).await?;
            return Err(Socks5Error::AuthFailed);
        }

        socket.write_all(&[0x01, 0x00]).await?;
    }

    // Read CONNECT request
    let n = socket.read(&mut buf).await?;
    if n < 4 || buf[0] != 0x05 {
        return Err(Socks5Error::InvalidVersion);
    }

    let cmd = buf[1];
    if cmd != 0x01 {
        // Only CONNECT supported
        socket.write_all(&[0x05, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0]).await?;
        return Err(Socks5Error::UnsupportedCommand);
    }

    let atyp = buf[3];
    let (target_ip, target_port) = match atyp {
        0x01 => {
            // IPv4
            if n < 10 {
                return Err(Socks5Error::UnsupportedAddressType);
            }
            let ip = format!("{}.{}.{}.{}", buf[4], buf[5], buf[6], buf[7]);
            let port = ((buf[8] as u16) << 8) | (buf[9] as u16);
            (ip, port)
        }
        0x03 => {
            // Domain name
            if n < 5 {
                return Err(Socks5Error::UnsupportedAddressType);
            }
            let domain_len = buf[4] as usize;
            if n < 5 + domain_len + 2 {
                return Err(Socks5Error::UnsupportedAddressType);
            }
            let domain = String::from_utf8_lossy(&buf[5..5 + domain_len]).to_string();
            let port = ((buf[5 + domain_len] as u16) << 8) | (buf[5 + domain_len + 1] as u16);
            (domain, port)
        }
        0x04 => {
            // IPv6 - not supported
            socket.write_all(&[0x05, 0x08, 0x00, 0x01, 0, 0, 0, 0, 0, 0]).await?;
            return Err(Socks5Error::UnsupportedAddressType);
        }
        _ => {
            return Err(Socks5Error::UnsupportedAddressType);
        }
    };

    tracing::debug!("SOCKS5 CONNECT {}:{}", target_ip, target_port);

    // Send success response
    socket.write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0]).await?;

    // TODO: Open tunnel channel and forward traffic

    Ok(())
}
