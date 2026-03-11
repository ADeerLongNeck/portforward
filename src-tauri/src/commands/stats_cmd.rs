use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tauri::State;

/// Statistics data
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Stats {
    pub upload_speed: u64,
    pub download_speed: u64,
    pub total_upload: u64,
    pub total_download: u64,
    pub active_connections: u32,
}

/// Internal stats with speed calculation
struct InternalStats {
    total_upload: AtomicU64,
    total_download: AtomicU64,
    active_connections: AtomicU64,
    last_upload: AtomicU64,
    last_download: AtomicU64,
    last_update: RwLock<Instant>,
}

impl InternalStats {
    fn new() -> Self {
        Self {
            total_upload: AtomicU64::new(0),
            total_download: AtomicU64::new(0),
            active_connections: AtomicU64::new(0),
            last_upload: AtomicU64::new(0),
            last_download: AtomicU64::new(0),
            last_update: RwLock::new(Instant::now()),
        }
    }

    fn add_upload(&self, bytes: u64) {
        self.total_upload.fetch_add(bytes, Ordering::Relaxed);
    }

    fn add_download(&self, bytes: u64) {
        self.total_download.fetch_add(bytes, Ordering::Relaxed);
    }

    fn inc_connections(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    fn dec_connections(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    async fn get_snapshot(&self) -> Stats {
        let now = Instant::now();
        let mut last_update = self.last_update.write().await;
        let elapsed = now.duration_since(*last_update).as_secs_f64();

        let current_upload = self.total_upload.load(Ordering::Relaxed);
        let current_download = self.total_download.load(Ordering::Relaxed);

        let last_upload = self.last_upload.swap(current_upload, Ordering::Relaxed);
        let last_download = self.last_download.swap(current_download, Ordering::Relaxed);

        let (upload_speed, download_speed) = if elapsed > 0.0 {
            let upload_diff = current_upload.saturating_sub(last_upload);
            let download_diff = current_download.saturating_sub(last_download);
            ((upload_diff as f64 / elapsed) as u64, (download_diff as f64 / elapsed) as u64)
        } else {
            (0, 0)
        };

        *last_update = now;

        Stats {
            upload_speed,
            download_speed,
            total_upload: current_upload,
            total_download: current_download,
            active_connections: self.active_connections.load(Ordering::Relaxed) as u32,
        }
    }
}

/// Stats state - Cloneable wrapper around shared stats
#[derive(Clone)]
pub struct StatsState {
    stats: Arc<InternalStats>,
}

impl StatsState {
    pub fn new() -> Self {
        Self {
            stats: Arc::new(InternalStats::new()),
        }
    }

    pub fn add_upload(&self, bytes: u64) {
        self.stats.add_upload(bytes);
    }

    pub fn add_download(&self, bytes: u64) {
        self.stats.add_download(bytes);
    }

    pub fn inc_connections(&self) {
        self.stats.inc_connections();
    }

    pub fn dec_connections(&self) {
        self.stats.dec_connections();
    }

    pub async fn get_snapshot(&self) -> Stats {
        self.stats.get_snapshot().await
    }
}

/// Get stats
#[tauri::command]
pub async fn get_stats(state: State<'_, StatsState>) -> Result<Stats, String> {
    Ok(state.get_snapshot().await)
}

/// Update stats (deprecated - kept for compatibility)
#[tauri::command]
pub async fn update_stats(
    _upload_speed: u64,
    _download_speed: u64,
    _state: State<'_, StatsState>,
) -> Result<(), String> {
    // This function is deprecated - stats are now updated automatically
    Ok(())
}