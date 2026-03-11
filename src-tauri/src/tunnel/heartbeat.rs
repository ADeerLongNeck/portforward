use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Heartbeat manager for connection health monitoring
pub struct HeartbeatManager {
    /// Interval between heartbeats
    interval: Duration,
    /// Maximum number of consecutive failures before disconnect
    max_failures: u8,
    /// Current failure count
    failure_count: AtomicU32,
    /// Last successful heartbeat time (as Unix timestamp ms)
    last_pong: AtomicU64,
}

impl HeartbeatManager {
    /// Create a new heartbeat manager
    /// Default: 10 second interval, 3 max failures
    pub fn new() -> Self {
        Self {
            interval: Duration::from_secs(10),
            max_failures: 3,
            failure_count: AtomicU32::new(0),
            last_pong: AtomicU64::new(0),
        }
    }

    /// Create with custom settings
    pub fn with_settings(interval_secs: u64, max_failures: u8) -> Self {
        Self {
            interval: Duration::from_secs(interval_secs),
            max_failures,
            failure_count: AtomicU32::new(0),
            last_pong: AtomicU64::new(0),
        }
    }

    /// Get the heartbeat interval
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Called when a heartbeat is sent
    pub fn on_heartbeat_sent(&self) {
        // Increment failure count
        self.failure_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Called when a heartbeat response is received
    pub fn on_pong_received(&self) {
        // Reset failure count
        self.failure_count.store(0, Ordering::SeqCst);

        // Update last pong time
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.last_pong.store(now, Ordering::SeqCst);
    }

    /// Check if connection should be considered dead
    pub fn is_connection_dead(&self) -> bool {
        self.failure_count.load(Ordering::SeqCst) >= self.max_failures as u32
    }

    /// Get current failure count
    pub fn failure_count(&self) -> u32 {
        self.failure_count.load(Ordering::SeqCst)
    }

    /// Get time since last successful heartbeat
    pub fn time_since_last_pong(&self) -> Duration {
        let last = self.last_pong.load(Ordering::SeqCst);
        if last == 0 {
            return Duration::from_secs(0);
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Duration::from_millis(now.saturating_sub(last))
    }

    /// Reset all state
    pub fn reset(&self) {
        self.failure_count.store(0, Ordering::SeqCst);
        self.last_pong.store(0, Ordering::SeqCst);
    }
}

impl Default for HeartbeatManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heartbeat_flow() {
        let hb = HeartbeatManager::new();

        // Initial state
        assert!(!hb.is_connection_dead());

        // Send heartbeats
        hb.on_heartbeat_sent();
        assert!(!hb.is_connection_dead());
        assert_eq!(hb.failure_count(), 1);

        hb.on_heartbeat_sent();
        assert!(!hb.is_connection_dead());
        assert_eq!(hb.failure_count(), 2);

        hb.on_heartbeat_sent();
        assert!(hb.is_connection_dead());
        assert_eq!(hb.failure_count(), 3);

        // Receive pong
        hb.on_pong_received();
        assert!(!hb.is_connection_dead());
        assert_eq!(hb.failure_count(), 0);
    }
}
