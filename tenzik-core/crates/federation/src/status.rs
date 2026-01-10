use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;

/// Status information for a running Tenzik node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStatus {
    /// Node name (if configured)
    pub name: Option<String>,
    /// Listen address
    pub listen_addr: SocketAddr,
    /// Public key (hex-encoded)
    pub public_key: String,
    /// Node uptime
    pub uptime: Duration,
    /// Number of connected peers
    pub peer_count: usize,
    /// Total events in DAG
    pub event_count: usize,
    /// Total receipts stored
    pub receipt_count: usize,
    /// Database path
    pub db_path: String,
    /// Node version
    pub version: String,
    /// Health status
    pub health: HealthStatus,
    /// Network stats
    pub network: NetworkStats,
}

/// Health status of the node
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HealthStatus {
    /// Node is healthy and operating normally
    Healthy,
    /// Node is operating but has warnings
    Warning { message: String },
    /// Node has critical issues
    Critical { message: String },
}

/// Network statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    /// Total events sent
    pub events_sent: usize,
    /// Total events received
    pub events_received: usize,
    /// Total sync operations
    pub sync_count: usize,
    /// Active sync operations
    pub active_syncs: usize,
}

/// Detailed peer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerStatus {
    /// Peer socket address
    pub address: SocketAddr,
    /// Peer public key
    pub public_key: Option<String>,
    /// Peer name (if known)
    pub name: Option<String>,
    /// Connection status
    pub status: ConnectionStatus,
    /// Last seen timestamp
    pub last_seen: Option<String>,
    /// Events received from this peer
    pub events_received: usize,
    /// Events sent to this peer
    pub events_sent: usize,
}

/// Connection status for a peer
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// Connected and healthy
    Connected,
    /// Connected but experiencing issues
    Degraded,
    /// Disconnected
    Disconnected,
    /// Never connected
    Unknown,
}

impl NodeStatus {
    /// Check if the node is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self.health, HealthStatus::Healthy)
    }

    /// Get uptime as a human-readable string
    pub fn uptime_string(&self) -> String {
        let total_secs = self.uptime.as_secs();
        let days = total_secs / 86400;
        let hours = (total_secs % 86400) / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;

        if days > 0 {
            format!("{}d {}h {}m {}s", days, hours, minutes, seconds)
        } else if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, seconds)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    }

    /// Get a summary status string
    pub fn summary(&self) -> String {
        match &self.health {
            HealthStatus::Healthy => "✅ Healthy".to_string(),
            HealthStatus::Warning { message } => format!("⚠️  Warning: {}", message),
            HealthStatus::Critical { message } => format!("❌ Critical: {}", message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uptime_formatting() {
        let status = NodeStatus {
            name: None,
            listen_addr: "127.0.0.1:9000".parse().unwrap(),
            public_key: "test_key".to_string(),
            uptime: Duration::from_secs(3661), // 1h 1m 1s
            peer_count: 0,
            event_count: 0,
            receipt_count: 0,
            db_path: "/tmp/test".to_string(),
            version: "0.1.0".to_string(),
            health: HealthStatus::Healthy,
            network: NetworkStats {
                events_sent: 0,
                events_received: 0,
                sync_count: 0,
                active_syncs: 0,
            },
        };

        assert_eq!(status.uptime_string(), "1h 1m 1s");
    }

    #[test]
    fn test_health_check() {
        let healthy = NodeStatus {
            name: None,
            listen_addr: "127.0.0.1:9000".parse().unwrap(),
            public_key: "test_key".to_string(),
            uptime: Duration::from_secs(100),
            peer_count: 0,
            event_count: 0,
            receipt_count: 0,
            db_path: "/tmp/test".to_string(),
            version: "0.1.0".to_string(),
            health: HealthStatus::Healthy,
            network: NetworkStats {
                events_sent: 0,
                events_received: 0,
                sync_count: 0,
                active_syncs: 0,
            },
        };

        assert!(healthy.is_healthy());
        assert_eq!(healthy.summary(), "✅ Healthy");
    }
}
