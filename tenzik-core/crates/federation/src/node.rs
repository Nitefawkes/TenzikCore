//! Node Management Module
//!
//! This module implements Tenzik node management, including identity,
//! peer discovery, and basic networking.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::{error, info, warn};

use crate::storage::EventDAG;
use tenzik_protocol::{Event, EventContent, EventType, NodeInfo};

/// Configuration for a Tenzik node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Node's listen address
    pub listen_addr: SocketAddr,
    /// Database path for local storage
    pub db_path: String,
    /// Node's human-readable name
    pub name: String,
    /// Initial peers to connect to
    pub initial_peers: Vec<SocketAddr>,
    /// Signing key (Ed25519) for this node
    pub signing_key: Option<ed25519_dalek::SigningKey>,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:9000".parse().unwrap(),
            db_path: ".tenzik".to_string(),
            name: "tenzik-node".to_string(),
            initial_peers: Vec::new(),
            signing_key: None,
        }
    }
}

/// Information about a connected peer
#[derive(Debug, Clone)]
pub struct ConnectedPeer {
    /// Peer's address
    pub address: SocketAddr,
    /// Peer's node information
    pub node_info: NodeInfo,
    /// When the connection was established
    pub connected_at: chrono::DateTime<chrono::Utc>,
    /// Last seen timestamp
    pub last_seen: chrono::DateTime<chrono::Utc>,
}

/// A Tenzik federation node
pub struct TenzikNode {
    /// Node configuration
    config: NodeConfig,
    /// Local event DAG
    dag: EventDAG,
    /// Node's signing key
    signing_key: ed25519_dalek::SigningKey,
    /// Connected peers
    peers: HashMap<SocketAddr, ConnectedPeer>,
    /// Local sequence counter
    sequence: u64,
    /// Node start time
    start_time: chrono::DateTime<chrono::Utc>,
}

impl TenzikNode {
    /// Create a new Tenzik node
    pub fn new(config: NodeConfig) -> Result<Self> {
        // Generate or use provided signing key
        let signing_key = config.signing_key.clone().unwrap_or_else(|| {
            use rand::rngs::OsRng;
            ed25519_dalek::SigningKey::generate(&mut OsRng)
        });

        // Open local DAG storage
        let dag = EventDAG::new(&config.db_path)?;

        Ok(TenzikNode {
            config,
            dag,
            signing_key,
            peers: HashMap::new(),
            sequence: 1,
            start_time: chrono::Utc::now(),
        })
    }

    /// Start the node (bind to listen address)
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting Tenzik node on {}", self.config.listen_addr);

        // Bind to listen address
        let listener = TcpListener::bind(self.config.listen_addr).await?;
        info!("Node listening on {}", self.config.listen_addr);

        // Announce ourselves to the network
        self.announce_self().await?;

        // Connect to initial peers
        for peer_addr in &self.config.initial_peers {
            if let Err(e) = self.connect_to_peer(*peer_addr).await {
                warn!("Failed to connect to initial peer {}: {}", peer_addr, e);
            }
        }

        // TODO: Accept incoming connections
        // TODO: Start gossip protocol

        Ok(())
    }

    /// Announce this node to the network
    async fn announce_self(&mut self) -> Result<()> {
        let node_info = NodeInfo {
            public_key: hex::encode(self.signing_key.verifying_key().as_bytes()),
            address: self.config.listen_addr.to_string(),
            name: self.config.name.clone(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        };

        // Get current tips as parents for this announcement
        let tips = self.dag.get_tips()?;
        let parents: Vec<String> = tips.into_iter().map(|e| e.id).collect();

        let event = Event::new_node_announce(
            node_info,
            vec!["receipt".to_string(), "federation".to_string()], // capabilities
            parents,
            self.sequence,
            hex::encode(self.signing_key.verifying_key().as_bytes()),
            &self.signing_key,
        )?;

        self.sequence += 1;
        self.dag.add_event(event)?;

        info!("Announced node to network");
        Ok(())
    }

    /// Connect to a peer
    async fn connect_to_peer(&mut self, peer_addr: SocketAddr) -> Result<()> {
        info!("Connecting to peer: {}", peer_addr);

        // TODO: Implement actual TCP connection and handshake
        // For now, just simulate a successful connection

        let peer_info = ConnectedPeer {
            address: peer_addr,
            node_info: NodeInfo {
                public_key: "simulated_peer_key".to_string(),
                address: peer_addr.to_string(),
                name: format!("peer-{}", peer_addr.port()),
                version: "0.1.0".to_string(),
            },
            connected_at: chrono::Utc::now(),
            last_seen: chrono::Utc::now(),
        };

        self.peers.insert(peer_addr, peer_info);
        info!("Connected to peer: {}", peer_addr);

        Ok(())
    }

    /// Get connected peers
    pub fn get_connected_peers(&self) -> Vec<&ConnectedPeer> {
        self.peers.values().collect()
    }

    /// Get node's public key
    pub fn public_key(&self) -> ed25519_dalek::VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Get node's address
    pub fn listen_address(&self) -> SocketAddr {
        self.config.listen_addr
    }

    /// Get DAG statistics
    pub fn get_dag_stats(&self) -> Result<crate::storage::DAGStats> {
        self.dag.get_stats()
    }

    /// Add an event to the local DAG (e.g., from execution)
    pub fn add_event(&mut self, event: Event) -> Result<()> {
        self.dag.add_event(event)?;
        // TODO: Trigger gossip to peers
        Ok(())
    }

    /// Shutdown the node gracefully
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down Tenzik node");

        // Send leave announcement
        let tips = self.dag.get_tips()?;
        let parents: Vec<String> = tips.into_iter().map(|e| e.id).collect();

        // Create node leave event directly
        let content = EventContent::NodeLeave {
            reason: "Graceful shutdown".to_string(),
        };
        let timestamp = chrono::Utc::now().to_rfc3339();
        let node_id = hex::encode(self.signing_key.verifying_key().as_bytes());

        let leave_event = Event::new_event(
            EventType::NodeLeave,
            content,
            parents,
            self.sequence,
            node_id,
            &self.signing_key,
            timestamp,
        )?;

        self.sequence += 1;
        self.dag.add_event(leave_event)?;

        // TODO: Send leave event to all peers
        // TODO: Close all connections

        info!("Node shutdown complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_node_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = NodeConfig {
            db_path: temp_dir.path().to_string_lossy().to_string(),
            ..Default::default()
        };

        let node = TenzikNode::new(config).unwrap();
        assert_eq!(node.get_connected_peers().len(), 0);
    }

    #[test]
    fn test_node_config() {
        let config = NodeConfig::default();
        assert_eq!(config.name, "tenzik-node");
        assert_eq!(config.initial_peers.len(), 0);
    }
}
