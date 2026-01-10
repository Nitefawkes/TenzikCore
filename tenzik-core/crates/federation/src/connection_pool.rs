//! Connection pool for managing persistent peer connections

use anyhow::Result;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::gossip::GossipMessage;
use crate::transport::{self, PeerConnection};
use tenzik_protocol::NodeInfo;

/// Pool of active peer connections
pub struct ConnectionPool {
    /// Active connections indexed by peer address
    connections: Arc<RwLock<HashMap<SocketAddr, Arc<RwLock<PeerConnection>>>>>,
    /// Local node information for handshakes
    local_node_info: NodeInfo,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub fn new(local_node_info: NodeInfo) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            local_node_info,
        }
    }

    /// Get or create a connection to a peer
    pub async fn get_or_connect(&self, peer_addr: SocketAddr) -> Result<Arc<RwLock<PeerConnection>>> {
        // Check if we already have a connection
        {
            let connections = self.connections.read().await;
            if let Some(conn) = connections.get(&peer_addr) {
                debug!("Reusing existing connection to {}", peer_addr);
                return Ok(conn.clone());
            }
        }

        // Create new connection
        info!("Creating new connection to {}", peer_addr);
        let (stream, peer_info) = transport::connect_to_peer(peer_addr, &self.local_node_info).await?;
        let peer_conn = PeerConnection::new(stream, peer_addr, peer_info);
        let peer_conn = Arc::new(RwLock::new(peer_conn));

        // Store connection
        let mut connections = self.connections.write().await;
        connections.insert(peer_addr, peer_conn.clone());

        Ok(peer_conn)
    }

    /// Add an existing connection to the pool
    pub async fn add_connection(&self, stream: TcpStream, peer_addr: SocketAddr, peer_info: NodeInfo) {
        let peer_conn = PeerConnection::new(stream, peer_addr, peer_info);
        let peer_conn = Arc::new(RwLock::new(peer_conn));

        let mut connections = self.connections.write().await;
        connections.insert(peer_addr, peer_conn);
        info!("Added incoming connection from {}", peer_addr);
    }

    /// Send a gossip message to a peer
    pub async fn send_message(&self, peer_addr: SocketAddr, message: GossipMessage) -> Result<()> {
        let conn = self.get_or_connect(peer_addr).await?;
        let mut conn = conn.write().await;
        conn.send_gossip(message).await
    }

    /// Receive a gossip message from a peer
    pub async fn receive_message(&self, peer_addr: SocketAddr) -> Result<GossipMessage> {
        let conn = self.get_or_connect(peer_addr).await?;
        let mut conn = conn.write().await;
        conn.receive_gossip().await
    }

    /// Remove a connection from the pool
    pub async fn remove_connection(&self, peer_addr: SocketAddr) {
        let mut connections = self.connections.write().await;
        if connections.remove(&peer_addr).is_some() {
            info!("Removed connection to {}", peer_addr);
        }
    }

    /// Get all active peer addresses
    pub async fn get_active_peers(&self) -> Vec<SocketAddr> {
        let connections = self.connections.read().await;
        connections.keys().copied().collect()
    }

    /// Get connection count
    pub async fn connection_count(&self) -> usize {
        let connections = self.connections.read().await;
        connections.len()
    }

    /// Broadcast a message to all connected peers
    pub async fn broadcast(&self, message: GossipMessage) -> Vec<Result<()>> {
        let peer_addrs = self.get_active_peers().await;
        let mut results = Vec::new();

        for peer_addr in peer_addrs {
            let result = self.send_message(peer_addr, message.clone()).await;
            if let Err(e) = &result {
                warn!("Failed to send message to {}: {}", peer_addr, e);
            }
            results.push(result);
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_node_info() -> NodeInfo {
        NodeInfo {
            public_key: "test_key".to_string(),
            address: "127.0.0.1:9000".to_string(),
            name: "test_node".to_string(),
            version: "0.1.0".to_string(),
        }
    }

    #[tokio::test]
    async fn test_connection_pool_creation() {
        let node_info = create_test_node_info();
        let pool = ConnectionPool::new(node_info);
        assert_eq!(pool.connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_get_active_peers() {
        let node_info = create_test_node_info();
        let pool = ConnectionPool::new(node_info);
        let peers = pool.get_active_peers().await;
        assert_eq!(peers.len(), 0);
    }
}
