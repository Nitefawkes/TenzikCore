//! Node Management Module
//!
//! This module implements Tenzik node management, including identity,
//! peer discovery, and basic networking.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::gossip::{GossipConfig, GossipProtocol};
use crate::storage::EventDAG;
use crate::transport::{accept_connection, connect_to_peer, PeerConnection};
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
    #[serde(skip)]
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

/// Message for internal node communication
enum NodeMessage {
    /// New event to add to DAG and gossip
    NewEvent(Event),
    /// Peer connected
    PeerConnected(SocketAddr, NodeInfo),
    /// Peer disconnected
    PeerDisconnected(SocketAddr),
    /// Shutdown signal
    Shutdown,
}

/// A Tenzik federation node
pub struct TenzikNode {
    /// Node configuration
    config: NodeConfig,
    /// Local event DAG (wrapped in Arc<RwLock> for shared access)
    dag: Arc<RwLock<EventDAG>>,
    /// Node's signing key
    signing_key: ed25519_dalek::SigningKey,
    /// Connected peers
    peers: Arc<RwLock<HashMap<SocketAddr, ConnectedPeer>>>,
    /// Local sequence counter
    sequence: Arc<RwLock<u64>>,
    /// Node start time
    start_time: chrono::DateTime<chrono::Utc>,
    /// Gossip protocol
    gossip: Option<Arc<RwLock<GossipProtocol>>>,
    /// Channel for node messages
    message_tx: mpsc::UnboundedSender<NodeMessage>,
    message_rx: Option<mpsc::UnboundedReceiver<NodeMessage>>,
    /// Background task handles
    tasks: Vec<JoinHandle<()>>,
}

impl TenzikNode {
    /// Create a new Tenzik node
    pub fn new(config: NodeConfig) -> Result<Self> {
        // Generate or use provided signing key
        let signing_key = config.signing_key.clone().unwrap_or_else(|| {
            use rand::RngCore;
            let mut csprng = rand::rngs::OsRng;
            let mut secret_bytes = [0u8; 32];
            csprng.fill_bytes(&mut secret_bytes);
            ed25519_dalek::SigningKey::from_bytes(&secret_bytes)
        });

        // Open local DAG storage
        let dag = Arc::new(RwLock::new(EventDAG::new(&config.db_path)?));

        // Create message channel
        let (message_tx, message_rx) = mpsc::unbounded_channel();

        Ok(TenzikNode {
            config,
            dag,
            signing_key,
            peers: Arc::new(RwLock::new(HashMap::new())),
            sequence: Arc::new(RwLock::new(1)),
            start_time: chrono::Utc::now(),
            gossip: None,
            message_tx,
            message_rx: Some(message_rx),
            tasks: Vec::new(),
        })
    }

    /// Start the node (bind to listen address)
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting Tenzik node on {}", self.config.listen_addr);

        // Bind to listen address
        let listener = TcpListener::bind(self.config.listen_addr).await?;
        info!("Node listening on {}", self.config.listen_addr);

        // Initialize gossip protocol
        let dag_clone = self.dag.read().await;
        // We need to create a new EventDAG for the gossip protocol (it needs owned access)
        // For now, we'll initialize without it and add events manually
        drop(dag_clone);

        let gossip_config = GossipConfig::default();
        let temp_dag = EventDAG::new(format!("{}_gossip", &self.config.db_path))?;
        let gossip = Arc::new(RwLock::new(GossipProtocol::new(gossip_config, temp_dag)));
        self.gossip = Some(gossip.clone());

        // Announce ourselves to the network
        self.announce_self().await?;

        // Start message processor
        let mut message_rx = self.message_rx.take().unwrap();
        let dag = self.dag.clone();
        let peers = self.peers.clone();

        let message_processor = tokio::spawn(async move {
            while let Some(msg) = message_rx.recv().await {
                match msg {
                    NodeMessage::NewEvent(event) => {
                        if let Err(e) = dag.write().await.add_event(event) {
                            error!("Failed to add event to DAG: {}", e);
                        }
                    }
                    NodeMessage::PeerConnected(addr, node_info) => {
                        let peer = ConnectedPeer {
                            address: addr,
                            node_info,
                            connected_at: chrono::Utc::now(),
                            last_seen: chrono::Utc::now(),
                        };
                        peers.write().await.insert(addr, peer);
                        info!("Peer connected: {}", addr);
                    }
                    NodeMessage::PeerDisconnected(addr) => {
                        peers.write().await.remove(&addr);
                        info!("Peer disconnected: {}", addr);
                    }
                    NodeMessage::Shutdown => {
                        info!("Message processor shutting down");
                        break;
                    }
                }
            }
        });
        self.tasks.push(message_processor);

        // Start accepting incoming connections
        let node_info = self.get_node_info();
        let message_tx = self.message_tx.clone();
        let accept_task = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, peer_addr)) => {
                        info!("Incoming connection from: {}", peer_addr);

                        let node_info_clone = node_info.clone();
                        let message_tx_clone = message_tx.clone();

                        tokio::spawn(async move {
                            match accept_connection(stream, peer_addr, &node_info_clone).await {
                                Ok(peer_info) => {
                                    info!("Accepted connection from {}: {}", peer_addr, peer_info.name);
                                    let _ = message_tx_clone.send(NodeMessage::PeerConnected(peer_addr, peer_info));
                                    // TODO: Handle ongoing communication with this peer
                                }
                                Err(e) => {
                                    error!("Failed to accept connection from {}: {}", peer_addr, e);
                                }
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept connection: {}", e);
                    }
                }
            }
        });
        self.tasks.push(accept_task);

        // Connect to initial peers
        for peer_addr in self.config.initial_peers.clone() {
            let node_info = self.get_node_info();
            let message_tx = self.message_tx.clone();

            tokio::spawn(async move {
                match connect_to_peer(peer_addr, &node_info).await {
                    Ok((_stream, peer_info)) => {
                        info!("Connected to peer {}: {}", peer_addr, peer_info.name);
                        let _ = message_tx.send(NodeMessage::PeerConnected(peer_addr, peer_info));
                        // TODO: Handle ongoing communication with this peer
                    }
                    Err(e) => {
                        warn!("Failed to connect to initial peer {}: {}", peer_addr, e);
                    }
                }
            });
        }

        // Start gossip protocol
        let gossip_clone = gossip.clone();
        let gossip_task = tokio::spawn(async move {
            if let Err(e) = gossip_clone.write().await.start().await {
                error!("Gossip protocol error: {}", e);
            }
        });
        self.tasks.push(gossip_task);

        info!("Node started successfully");
        Ok(())
    }

    /// Get node information
    fn get_node_info(&self) -> NodeInfo {
        NodeInfo {
            public_key: hex::encode(self.signing_key.verifying_key().as_bytes()),
            address: self.config.listen_addr.to_string(),
            name: self.config.name.clone(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Announce this node to the network
    async fn announce_self(&mut self) -> Result<()> {
        let node_info = self.get_node_info();

        // Get current tips as parents for this announcement
        let tips = self.dag.read().await.get_tips()?;
        let parents: Vec<String> = tips.into_iter().map(|e| e.id).collect();

        let mut sequence = self.sequence.write().await;
        let event = Event::new_node_announce(
            node_info,
            vec!["receipt".to_string(), "federation".to_string()], // capabilities
            parents,
            *sequence,
            hex::encode(self.signing_key.verifying_key().as_bytes()),
            &self.signing_key,
        )?;

        *sequence += 1;
        drop(sequence);

        self.dag.write().await.add_event(event)?;

        info!("Announced node to network");
        Ok(())
    }

    /// Get connected peers
    pub async fn get_connected_peers(&self) -> Vec<ConnectedPeer> {
        self.peers.read().await.values().cloned().collect()
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
    pub async fn get_dag_stats(&self) -> Result<tenzik_protocol::DAGStats> {
        Ok(self.dag.read().await.get_stats()?)
    }

    /// Add an event to the local DAG (e.g., from execution)
    pub async fn add_event(&mut self, event: Event) -> Result<()> {
        self.message_tx.send(NodeMessage::NewEvent(event))?;
        Ok(())
    }

    /// Shutdown the node gracefully
    pub async fn shutdown(mut self) -> Result<()> {
        info!("Shutting down Tenzik node");

        // Send leave announcement
        let tips = self.dag.read().await.get_tips()?;
        let parents: Vec<String> = tips.into_iter().map(|e| e.id).collect();

        let mut sequence = self.sequence.write().await;
        let timestamp = chrono::Utc::now().to_rfc3339();
        let node_id = hex::encode(self.signing_key.verifying_key().as_bytes());

        let leave_event = Event::new_node_leave(
            "Graceful shutdown".to_string(),
            parents,
            *sequence,
            node_id,
            &self.signing_key,
        )?;

        *sequence += 1;
        drop(sequence);

        self.dag.write().await.add_event(leave_event)?;

        // Signal shutdown to message processor
        let _ = self.message_tx.send(NodeMessage::Shutdown);

        // Abort all background tasks
        for task in self.tasks {
            task.abort();
        }

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
