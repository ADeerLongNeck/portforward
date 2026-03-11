use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Traffic sample for a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficSample {
    pub timestamp: DateTime<Utc>,
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub connections: u64,
}

/// Traffic summary for frontend display
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrafficSummary {
    pub total_upload: u64,
    pub total_download: u64,
    pub total_connections: u64,
    pub upload_speed_bps: u64,
    pub download_speed_bps: u64,
    pub active_connections: u64,
}

/// Per-server traffic statistics
pub struct ServerTrafficStats {
    upload: AtomicU64,
    download: AtomicU64,
    connections: AtomicU64,
    active_connections: AtomicU64,
    history: RwLock<VecDeque<TrafficSample>>,
    max_history_size: usize,
}

impl ServerTrafficStats {
    pub fn new() -> Self {
        Self {
            upload: AtomicU64::new(0),
            download: AtomicU64::new(0),
            connections: AtomicU64::new(0),
            active_connections: AtomicU64::new(0),
            history: RwLock::new(VecDeque::with_capacity(360)), // 1 hour at 10s intervals
            max_history_size: 360,
        }
    }

    pub fn add_upload(&self, bytes: u64) {
        self.upload.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn add_download(&self, bytes: u64) {
        self.download.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn inc_connections(&self) {
        self.connections.fetch_add(1, Ordering::Relaxed);
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dec_active_connections(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn total_upload(&self) -> u64 {
        self.upload.load(Ordering::Relaxed)
    }

    pub fn total_download(&self) -> u64 {
        self.download.load(Ordering::Relaxed)
    }

    pub fn total_connections(&self) -> u64 {
        self.connections.load(Ordering::Relaxed)
    }

    pub fn active_connections(&self) -> u64 {
        self.active_connections.load(Ordering::Relaxed)
    }

    /// Record a traffic sample
    pub async fn record_sample(&self) {
        let sample = TrafficSample {
            timestamp: Utc::now(),
            upload_bytes: self.total_upload(),
            download_bytes: self.total_download(),
            connections: self.total_connections(),
        };

        let mut history = self.history.write().await;
        if history.len() >= self.max_history_size {
            history.pop_front();
        }
        history.push_back(sample);
    }

    /// Get history samples
    pub async fn get_history(&self) -> Vec<TrafficSample> {
        self.history.read().await.iter().cloned().collect()
    }

    /// Calculate speed from recent samples
    pub async fn get_summary(&self) -> TrafficSummary {
        let history = self.history.read().await;

        let (upload_speed, download_speed) = if history.len() >= 2 {
            let recent = history.back().unwrap();
            let older = history.front().unwrap();
            let time_diff = (recent.timestamp - older.timestamp).num_seconds() as f64;

            if time_diff > 0.0 {
                let upload_diff = recent.upload_bytes.saturating_sub(older.upload_bytes);
                let download_diff = recent.download_bytes.saturating_sub(older.download_bytes);
                (
                    (upload_diff as f64 / time_diff) as u64,
                    (download_diff as f64 / time_diff) as u64,
                )
            } else {
                (0, 0)
            }
        } else {
            (0, 0)
        };

        TrafficSummary {
            total_upload: self.total_upload(),
            total_download: self.total_download(),
            total_connections: self.total_connections(),
            upload_speed_bps: upload_speed,
            download_speed_bps: download_speed,
            active_connections: self.active_connections(),
        }
    }

    /// Reset all counters
    pub fn reset(&self) {
        self.upload.store(0, Ordering::Relaxed);
        self.download.store(0, Ordering::Relaxed);
        self.connections.store(0, Ordering::Relaxed);
        self.active_connections.store(0, Ordering::Relaxed);
    }
}

impl Default for ServerTrafficStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Global traffic statistics manager
pub struct TrafficStats {
    servers: RwLock<std::collections::HashMap<String, Arc<ServerTrafficStats>>>,
}

impl TrafficStats {
    pub fn new() -> Self {
        Self {
            servers: RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Get or create stats for a server
    pub async fn get_server_stats(&self, server_id: &str) -> Arc<ServerTrafficStats> {
        let mut servers = self.servers.write().await;
        servers
            .entry(server_id.to_string())
            .or_insert_with(|| Arc::new(ServerTrafficStats::new()))
            .clone()
    }

    /// Get summary for a server
    pub async fn get_summary(&self, server_id: &str) -> TrafficSummary {
        let stats = self.get_server_stats(server_id).await;
        stats.get_summary().await
    }

    /// Get history for a server
    pub async fn get_history(&self, server_id: &str) -> Vec<TrafficSample> {
        let stats = self.get_server_stats(server_id).await;
        stats.get_history().await
    }

    /// Record samples for all servers
    pub async fn record_all_samples(&self) {
        let servers = self.servers.read().await;
        for stats in servers.values() {
            stats.record_sample().await;
        }
    }

    /// Remove stats for a server
    pub async fn remove_server(&self, server_id: &str) {
        let mut servers = self.servers.write().await;
        servers.remove(server_id);
    }
}

impl Default for TrafficStats {
    fn default() -> Self {
        Self::new()
    }
}
