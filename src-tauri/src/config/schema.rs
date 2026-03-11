use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub servers: Vec<ServerConfig>,
    pub settings: SettingsConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            servers: Vec::new(),
            settings: SettingsConfig::default(),
        }
    }
}

/// Server connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub auth: AuthConfig,
    pub port_forward: PortForwardConfig,
    pub socks5: Option<Socks5Config>,
    pub auto_reconnect: bool,
    #[serde(default = "default_reconnect_interval")]
    pub reconnect_interval: u64,
}

fn default_reconnect_interval() -> u64 { 5 }

impl ServerConfig {
    pub fn new(name: String, host: String, port: u16, password: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            host,
            port,
            auth: AuthConfig { password },
            port_forward: PortForwardConfig::default(),
            socks5: None,
            auto_reconnect: true,
            reconnect_interval: 5,
        }
    }
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub password: String,
}

/// Port forward rules configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PortForwardConfig {
    pub local2remote: Vec<Local2RemoteRule>,
    pub remote2local: Vec<Remote2LocalRule>,
}

/// Local to remote forwarding rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Local2RemoteRule {
    pub id: String,
    pub name: Option<String>,
    pub local_port: u16,
    pub remote_ip: String,
    pub remote_port: u16,
    pub enabled: bool,
}

impl Local2RemoteRule {
    pub fn new(local_port: u16, remote_ip: String, remote_port: u16) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: None,
            local_port,
            remote_ip,
            remote_port,
            enabled: true,
        }
    }
}

/// Remote to local forwarding rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Remote2LocalRule {
    pub id: String,
    pub name: Option<String>,
    pub remote_port: u16,
    pub local_ip: String,
    pub local_port: u16,
    pub enabled: bool,
}

impl Remote2LocalRule {
    pub fn new(remote_port: u16, local_ip: String, local_port: u16) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: None,
            remote_port,
            local_ip,
            local_port,
            enabled: true,
        }
    }
}

/// SOCKS5 proxy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Socks5Config {
    pub enabled: bool,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default)]
    pub local_resolution: bool,
}

impl Default for Socks5Config {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 1080,
            username: None,
            password: None,
            local_resolution: false,
        }
    }
}

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsConfig {
    pub theme: ThemeConfig,
    pub log: LogConfig,
    pub tray: TrayConfig,
}

impl Default for SettingsConfig {
    fn default() -> Self {
        Self {
            theme: ThemeConfig::default(),
            log: LogConfig::default(),
            tray: TrayConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub mode: ThemeMode,
    pub accent_color: String,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            mode: ThemeMode::System,
            accent_color: "#DC2626".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    pub level: String,
    pub max_size_mb: u32,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            max_size_mb: 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayConfig {
    pub show_on_startup: bool,
    pub minimize_to_tray: bool,
}

impl Default for TrayConfig {
    fn default() -> Self {
        Self {
            show_on_startup: true,
            minimize_to_tray: true,
        }
    }
}

/// Configuration manager
pub struct ConfigManager {
    config: Arc<RwLock<AppConfig>>,
    config_path: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Self {
        let config_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("port-forward")
            .join("config.toml");

        let config = if config_path.exists() {
            match std::fs::read_to_string(&config_path) {
                Ok(content) => toml::from_str(&content).unwrap_or_default(),
                Err(_) => AppConfig::default(),
            }
        } else {
            AppConfig::default()
        };

        Self {
            config: Arc::new(RwLock::new(config)),
            config_path,
        }
    }

    pub async fn get_config(&self) -> AppConfig {
        self.config.read().await.clone()
    }

    pub async fn get_servers(&self) -> Vec<ServerConfig> {
        self.config.read().await.servers.clone()
    }

    pub async fn get_server(&self, id: &str) -> Option<ServerConfig> {
        self.config
            .read()
            .await
            .servers
            .iter()
            .find(|s| s.id == id)
            .cloned()
    }

    pub async fn add_server(&self, server: ServerConfig) -> Result<(), String> {
        let mut config = self.config.write().await;
        config.servers.push(server);
        self.save_config(&config).await
    }

    pub async fn update_server(&self, server: ServerConfig) -> Result<(), String> {
        let mut config = self.config.write().await;
        if let Some(pos) = config.servers.iter().position(|s| s.id == server.id) {
            config.servers[pos] = server;
            self.save_config(&config).await
        } else {
            Err("Server not found".to_string())
        }
    }

    pub async fn remove_server(&self, id: &str) -> Result<(), String> {
        let mut config = self.config.write().await;
        config.servers.retain(|s| s.id != id);
        self.save_config(&config).await
    }

    pub async fn get_settings(&self) -> SettingsConfig {
        self.config.read().await.settings.clone()
    }

    pub async fn update_settings(&self, settings: SettingsConfig) -> Result<(), String> {
        let mut config = self.config.write().await;
        config.settings = settings;
        self.save_config(&config).await
    }

    pub async fn export(&self) -> Result<String, String> {
        let config = self.config.read().await;
        serde_json::to_string_pretty(&*config).map_err(|e| e.to_string())
    }

    pub async fn import(&self, json: &str) -> Result<(), String> {
        let config: AppConfig = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let mut cfg = self.config.write().await;
        *cfg = config;
        self.save_config(&cfg).await
    }

    async fn save_config(&self, config: &AppConfig) -> Result<(), String> {
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let content = toml::to_string_pretty(config).map_err(|e| e.to_string())?;
        std::fs::write(&self.config_path, content).map_err(|e| e.to_string())
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}
