use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use tauri::State;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub mode: String,
    // Server mode
    #[serde(default = "default_listen_port")]
    pub listen_port: u16,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub forward_ports: Vec<u16>,
    // Client mode
    #[serde(default)]
    pub server_host: String,
    #[serde(default = "default_server_port")]
    pub server_port: u16,
    #[serde(default = "default_reconnect_interval")]
    pub reconnect_interval: u32,
}

fn default_listen_port() -> u16 { 5173 }
fn default_server_port() -> u16 { 5173 }
fn default_reconnect_interval() -> u32 { 5 }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            mode: "client".to_string(),
            listen_port: 5173,
            password: String::new(),
            forward_ports: vec![],
            server_host: String::new(),
            server_port: 5173,
            reconnect_interval: 5,
        }
    }
}

/// Configuration state
pub struct ConfigState {
    pub config: Arc<RwLock<AppConfig>>,
    config_path: PathBuf,
}

impl ConfigState {
    pub fn new() -> Self {
        // Use standard config directory for cross-platform compatibility
        let config_path = dirs::config_dir()
            .map(|config_dir| {
                let app_dir = config_dir.join("port-forward");
                // Create directory if it doesn't exist
                let _ = std::fs::create_dir_all(&app_dir);
                app_dir.join("config.json")
            })
            .unwrap_or_else(|| PathBuf::from("config.json"));

        tracing::info!("Config path: {:?}", config_path);

        Self {
            config: Arc::new(RwLock::new(AppConfig::default())),
            config_path,
        }
    }

    pub async fn load(&self) -> Result<AppConfig, String> {
        if !self.config_path.exists() {
            return Ok(AppConfig::default());
        }

        let content = fs::read_to_string(&self.config_path)
            .await
            .map_err(|e| format!("读取配置失败: {}", e))?;

        let config: AppConfig = serde_json::from_str(&content)
            .unwrap_or_else(|_| AppConfig::default());

        Ok(config)
    }

    pub async fn save(&self, config: &AppConfig) -> Result<(), String> {
        let content = serde_json::to_string_pretty(config)
            .map_err(|e| format!("序列化配置失败: {}", e))?;

        fs::write(&self.config_path, content)
            .await
            .map_err(|e| format!("写入配置失败: {}", e))?;

        Ok(())
    }
}

/// Get current configuration
#[tauri::command]
pub async fn get_config(state: State<'_, ConfigState>) -> Result<AppConfig, String> {
    let config = state.load().await?;
    let mut current = state.config.write().await;
    *current = config.clone();
    Ok(config)
}

/// Save configuration
#[tauri::command]
pub async fn save_config(
    config: AppConfig,
    state: State<'_, ConfigState>,
) -> Result<(), String> {
    state.save(&config).await?;
    let mut current = state.config.write().await;
    *current = config;
    tracing::info!("Configuration saved to {:?}", state.config_path);
    Ok(())
}
