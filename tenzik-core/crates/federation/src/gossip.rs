//! Gossip Protocol Module
//!
//! This module implements a simple push-based gossip protocol for
//! propagating events between Tenzik nodes.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::{interval, Instant};
use tracing::{debug, info, warn, error};

use crate::storage::{Event, EventDAG};

/// Information about a peer for gossip
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Peer's network address
    pub address: SocketAddr,
    /// Peer's public key
    pub public_key: String,
    /// Last time we successfully synced with this peer
    pub last_sync: Option<Instant>,
    /// Number of events we've sent to this peer
    pub events_sent: u64,
    /// Number of events we've received from this peer
    pub events_received: u64,
    /// Whether this peer is currently reachable
    pub is_reachable: bool,
}

/// Gossip protocol messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GossipMessage {
    /// Request events since a specific event ID
    Sync {
        /// Event ID to sync since (None = all events)
        since: Option<String>,
        /// Maximum number of events to return
        limit: usize,
    },
    /// Push events to peer
    Events {
        /// List of events to send
        events: Vec<Event>,
        /// Whether more events are available
        has_more: bool,
    },
    /// Acknowledge receipt of events
    Ack {
        /// Number of events received
        count: usize,
        /// Event IDs that were rejected (e.g., duplicates)
        rejected: Vec<String>,
    },
    /// Ping to check connectivity
    Ping {
        /// Timestamp when ping was sent
        timestamp: u64,
    },
    /// Pong response to ping
    Pong {
        /// Original ping timestamp
        ping_timestamp: u64,
        /// Pong timestamp
        pong_timestamp: u64,
    },
}

/// Configuration for the gossip protocol
#[derive(Debug, Clone)]
pub struct GossipConfig {
    /// How often to sync with peers (milliseconds)
    pub sync_interval_ms: u64,
    /// Maximum number of events to send per sync
    pub max_events_per_sync: usize,
    /// Timeout for peer connections (milliseconds)
    pub peer_timeout_ms: u64,
    /// Maximum number of concurrent syncs
    pub max_concurrent_syncs: usize,
    /// How often to ping peers (milliseconds)
    pub ping_interval_ms: u64,
}

impl Default for GossipConfig {
    fn default() -> Self {
        Self {
            sync_interval_ms: 5000,     // 5 seconds
            max_events_per_sync: 100,   // 100 events
            peer_timeout_ms: 30000,     // 30 seconds
            max_concurrent_syncs: 5,    // 5 concurrent syncs
            ping_interval_ms: 10000,    // 10 seconds
        }
    }
}

/// Statistics for monitoring gossip performance
#[derive(Debug, Clone, Default)]
pub struct GossipStats {
    /// Total sync attempts
    pub sync_attempts: u64,
    /// Successful syncs
    pub sync_successes: u64,
    /// Failed syncs
    pub sync_failures: u64,
    /// Total events sent
    pub events_sent: u64,
    /// Total events received
    pub events_received: u64,
    /// Duplicate events received
    pub duplicate_events: u64,
    /// Average sync latency (milliseconds)
    pub avg_sync_latency_ms: f64,
}

/// Gossip protocol implementation
pub struct GossipProtocol {
    /// Configuration
    config: GossipConfig,
    /// Known peers
    peers: HashMap<SocketAddr, PeerInfo>,
    /// Local event DAG
    dag: EventDAG,
    /// Protocol statistics
    stats: GossipStats,
    /// Active sync operations
    active_syncs: HashSet<SocketAddr>,
}

impl GossipProtocol {
    /// Create a new gossip protocol instance
    pub fn new(config: GossipConfig, dag: EventDAG) -> Self {
        Self {
            config,
            peers: HashMap::new(),
            dag,
            stats: GossipStats::default(),
            active_syncs: HashSet::new(),
        }
    }
    
    /// Add a peer to the gossip network
    pub fn add_peer(&mut self, address: SocketAddr, public_key: String) {
        let peer_info = PeerInfo {
            address,
            public_key,
            last_sync: None,
            events_sent: 0,
            events_received: 0,
            is_reachable: true,
        };
        
        self.peers.insert(address, peer_info);
        info!("Added peer to gossip network: {}", address);
    }
    
    /// Remove a peer from the gossip network
    pub fn remove_peer(&mut self, address: &SocketAddr) {
        self.peers.remove(address);
        self.active_syncs.remove(address);
        info!("Removed peer from gossip network: {}", address);
    }
    
    /// Start the gossip protocol (background task)
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting gossip protocol");
        
        // Create sync interval
        let mut sync_interval = interval(Duration::from_millis(self.config.sync_interval_ms));
        let mut ping_interval = interval(Duration::from_millis(self.config.ping_interval_ms));
        
        loop {
            tokio::select! {
                _ = sync_interval.tick() => {
                    self.sync_with_peers().await;
                }
                _ = ping_interval.tick() => {
                    self.ping_peers().await;
                }
                // TODO: Handle incoming messages
            }
        }
    }
    
    /// Sync with all available peers
    async fn sync_with_peers(&mut self) {
        debug!("Starting sync round with {} peers", self.peers.len());
        
        // Collect peers that need syncing
        let peers_to_sync: Vec<SocketAddr> = self.peers
            .iter()
            .filter(|(addr, peer)| {
                // Skip if already syncing
                if self.active_syncs.contains(addr) {
                    return false;
                }
                
                // Skip unreachable peers
                if !peer.is_reachable {
                    return false;
                }
                
                // Sync if never synced or last sync was long ago
                peer.last_sync.map_or(true, |last| {
                    last.elapsed() > Duration::from_millis(self.config.sync_interval_ms)
                })
            })
            .map(|(addr, _)| *addr)
            .take(self.config.max_concurrent_syncs)
            .collect();
        
        // Start sync with selected peers
        for peer_addr in peers_to_sync {
            self.active_syncs.insert(peer_addr);
            let result = self.sync_with_peer(peer_addr).await;
            self.active_syncs.remove(&peer_addr);
            
            match result {
                Ok(_) => {
                    self.stats.sync_successes += 1;
                    if let Some(peer) = self.peers.get_mut(&peer_addr) {
                        peer.last_sync = Some(Instant::now());
                        peer.is_reachable = true;
                    }
                }
                Err(e) => {
                    self.stats.sync_failures += 1;
                    warn!("Sync failed with peer {}: {}", peer_addr, e);
                    if let Some(peer) = self.peers.get_mut(&peer_addr) {
                        peer.is_reachable = false;
                    }
                }
            }
        }
        
        self.stats.sync_attempts += self.active_syncs.len() as u64;
    }
    
    /// Sync with a specific peer
    async fn sync_with_peer(&mut self, peer_addr: SocketAddr) -> Result<()> {
        debug!("Syncing with peer: {}", peer_addr);
        
        let start_time = Instant::now();
        
        // Get events to send (simplified: send latest events)
        let events = self.dag.get_events_since(None)?;
        let events_to_send: Vec<Event> = events
            .into_iter()
            .rev() // Latest first
            .take(self.config.max_events_per_sync)
            .collect();
        
        if !events_to_send.is_empty() {
            // TODO: Send events to peer via network
            // For now, just simulate sending
            debug!("Sending {} events to peer {}", events_to_send.len(), peer_addr);
            
            // Update statistics
            self.stats.events_sent += events_to_send.len() as u64;
            if let Some(peer) = self.peers.get_mut(&peer_addr) {
                peer.events_sent += events_to_send.len() as u64;
            }
        }
        
        // TODO: Request events from peer
        // TODO: Handle peer's response
        
        // Update latency statistics
        let latency = start_time.elapsed().as_millis() as f64;
        self.stats.avg_sync_latency_ms = 
            (self.stats.avg_sync_latency_ms * self.stats.sync_successes as f64 + latency) 
            / (self.stats.sync_successes + 1) as f64;
        
        debug!("Sync completed with peer {} in {:.2}ms", peer_addr, latency);
        Ok(())
    }
    
    /// Send ping to all peers
    async fn ping_peers(&mut self) {
        debug!("Pinging {} peers", self.peers.len());
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        
        for (peer_addr, peer) in &mut self.peers {
            if peer.is_reachable {
                // TODO: Send ping message via network
                debug!("Pinging peer: {}", peer_addr);
            }
        }
    }
    
    /// Handle incoming gossip message
    pub async fn handle_message(&mut self, from: SocketAddr, message: GossipMessage) -> Result<Option<GossipMessage>> {
        match message {
            GossipMessage::Sync { since, limit } => {
                self.handle_sync_request(from, since, limit).await
            }
            GossipMessage::Events { events, has_more } => {
                self.handle_events(from, events, has_more).await
            }
            GossipMessage::Ack { count, rejected } => {
                self.handle_ack(from, count, rejected).await
            }
            GossipMessage::Ping { timestamp } => {
                self.handle_ping(from, timestamp).await
            }
            GossipMessage::Pong { ping_timestamp, pong_timestamp } => {
                self.handle_pong(from, ping_timestamp, pong_timestamp).await
            }
        }
    }
    
    /// Handle sync request from peer
    async fn handle_sync_request(&mut self, from: SocketAddr, since: Option<String>, limit: usize) -> Result<Option<GossipMessage>> {
        debug!("Handling sync request from {}, since: {:?}, limit: {}", from, since, limit);
        
        let events = self.dag.get_events_since(since.as_deref())?;
        let events_to_send: Vec<Event> = events
            .into_iter()
            .take(limit.min(self.config.max_events_per_sync))
            .collect();
        
        let has_more = events_to_send.len() >= limit;
        
        Ok(Some(GossipMessage::Events {
            events: events_to_send,
            has_more,
        }))
    }
    
    /// Handle events from peer
    async fn handle_events(&mut self, from: SocketAddr, events: Vec<Event>, _has_more: bool) -> Result<Option<GossipMessage>> {
        debug!("Handling {} events from {}", events.len(), from);
        
        let mut accepted = 0;
        let mut rejected = Vec::new();
        
        for event in events {
            match self.dag.add_event(event.clone()) {
                Ok(_) => {
                    accepted += 1;
                    self.stats.events_received += 1;
                }
                Err(_) => {
                    // Event already exists or invalid
                    rejected.push(event.id);
                    self.stats.duplicate_events += 1;
                }
            }
        }
        
        // Update peer statistics
        if let Some(peer) = self.peers.get_mut(&from) {
            peer.events_received += accepted;
        }
        
        debug!("Accepted {} events, rejected {} from {}", accepted, rejected.len(), from);
        
        Ok(Some(GossipMessage::Ack {
            count: accepted as usize,
            rejected,
        }))
    }
    
    /// Handle acknowledgment from peer
    async fn handle_ack(&mut self, from: SocketAddr, count: usize, rejected: Vec<String>) -> Result<Option<GossipMessage>> {
        debug!("Received ack from {}: {} accepted, {} rejected", from, count, rejected.len());
        
        // TODO: Update internal state based on ack
        // TODO: Handle rejected events (maybe retry or log)
        
        Ok(None)
    }
    
    /// Handle ping from peer
    async fn handle_ping(&mut self, from: SocketAddr, timestamp: u64) -> Result<Option<GossipMessage>> {
        debug!("Received ping from {}", from);
        
        let pong_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        
        Ok(Some(GossipMessage::Pong {
            ping_timestamp: timestamp,
            pong_timestamp,
        }))
    }
    
    /// Handle pong from peer
    async fn handle_pong(&mut self, from: SocketAddr, ping_timestamp: u64, pong_timestamp: u64) -> Result<Option<GossipMessage>> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        
        let rtt = now.saturating_sub(ping_timestamp);
        debug!("Received pong from {}, RTT: {}ms", from, rtt);
        
        // Mark peer as reachable
        if let Some(peer) = self.peers.get_mut(&from) {
            peer.is_reachable = true;
        }
        
        Ok(None)
    }
    
    /// Get gossip statistics
    pub fn get_stats(&self) -> &GossipStats {
        &self.stats
    }
    
    /// Get peer information
    pub fn get_peers(&self) -> &HashMap<SocketAddr, PeerInfo> {
        &self.peers
    }
    
    /// Get number of reachable peers
    pub fn reachable_peer_count(&self) -> usize {
        self.peers.values().filter(|p| p.is_reachable).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::storage::EventDAG;
    
    #[test]
    fn test_gossip_config_default() {
        let config = GossipConfig::default();
        assert_eq!(config.sync_interval_ms, 5000);
        assert_eq!(config.max_events_per_sync, 100);
    }
    
    #[test]
    fn test_peer_management() {
        let temp_dir = TempDir::new().unwrap();
        let dag = EventDAG::new(temp_dir.path()).unwrap();
        let mut gossip = GossipProtocol::new(GossipConfig::default(), dag);
        
        let peer_addr = "127.0.0.1:9001".parse().unwrap();
        let public_key = "test_key".to_string();
        
        gossip.add_peer(peer_addr, public_key.clone());
        assert_eq!(gossip.peers.len(), 1);
        assert_eq!(gossip.reachable_peer_count(), 1);
        
        gossip.remove_peer(&peer_addr);
        assert_eq!(gossip.peers.len(), 0);
        assert_eq!(gossip.reachable_peer_count(), 0);
    }
    
    #[tokio::test]
    async fn test_ping_pong() {
        let temp_dir = TempDir::new().unwrap();
        let dag = EventDAG::new(temp_dir.path()).unwrap();
        let mut gossip = GossipProtocol::new(GossipConfig::default(), dag);
        
        let peer_addr = "127.0.0.1:9001".parse().unwrap();
        let timestamp = 12345;
        
        // Handle ping
        let response = gossip.handle_ping(peer_addr, timestamp).await.unwrap();
        match response {
            Some(GossipMessage::Pong { ping_timestamp, .. }) => {
                assert_eq!(ping_timestamp, timestamp);
            }
            _ => panic!("Expected pong response"),
        }
    }
}
