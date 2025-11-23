//! Network transport layer for federation
//!
//! This module implements a simple TCP-based transport for sending and
//! receiving gossip messages between Tenzik nodes.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info, warn};

use crate::gossip::GossipMessage;
use tenzik_protocol::NodeInfo;

/// Maximum message size (10 MB)
const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024;

/// Protocol messages exchanged between nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProtocolMessage {
    /// Handshake request with node information
    Handshake { node_info: NodeInfo },
    /// Handshake acknowledgment
    HandshakeAck { node_info: NodeInfo },
    /// Gossip protocol message
    Gossip(GossipMessage),
    /// Error message
    Error { message: String },
}

/// Send a protocol message over a TCP stream
pub async fn send_message(stream: &mut TcpStream, message: &ProtocolMessage) -> Result<()> {
    // Serialize message to JSON
    let json = serde_json::to_vec(message)
        .context("Failed to serialize message")?;

    // Check message size
    if json.len() > MAX_MESSAGE_SIZE {
        anyhow::bail!("Message too large: {} bytes", json.len());
    }

    // Send length prefix (4 bytes, big-endian)
    let len = json.len() as u32;
    stream.write_u32(len).await
        .context("Failed to write message length")?;

    // Send message body
    stream.write_all(&json).await
        .context("Failed to write message body")?;

    stream.flush().await
        .context("Failed to flush stream")?;

    debug!("Sent message: {} bytes", json.len());
    Ok(())
}

/// Receive a protocol message from a TCP stream
pub async fn receive_message(stream: &mut TcpStream) -> Result<ProtocolMessage> {
    // Read length prefix (4 bytes, big-endian)
    let len = stream.read_u32().await
        .context("Failed to read message length")?;

    // Check message size
    if len as usize > MAX_MESSAGE_SIZE {
        anyhow::bail!("Message too large: {} bytes", len);
    }

    // Read message body
    let mut buffer = vec![0u8; len as usize];
    stream.read_exact(&mut buffer).await
        .context("Failed to read message body")?;

    // Deserialize message
    let message: ProtocolMessage = serde_json::from_slice(&buffer)
        .context("Failed to deserialize message")?;

    debug!("Received message: {} bytes", len);
    Ok(message)
}

/// Establish a connection to a peer and perform handshake
pub async fn connect_to_peer(
    peer_addr: SocketAddr,
    local_node_info: &NodeInfo,
) -> Result<(TcpStream, NodeInfo)> {
    debug!("Connecting to peer: {}", peer_addr);

    // Connect to peer
    let mut stream = TcpStream::connect(peer_addr).await
        .with_context(|| format!("Failed to connect to peer {}", peer_addr))?;

    // Send handshake
    let handshake = ProtocolMessage::Handshake {
        node_info: local_node_info.clone(),
    };
    send_message(&mut stream, &handshake).await
        .context("Failed to send handshake")?;

    // Receive handshake acknowledgment
    let response = receive_message(&mut stream).await
        .context("Failed to receive handshake response")?;

    match response {
        ProtocolMessage::HandshakeAck { node_info } => {
            info!("Handshake completed with peer: {}", peer_addr);
            Ok((stream, node_info))
        }
        ProtocolMessage::Error { message } => {
            anyhow::bail!("Peer rejected handshake: {}", message);
        }
        _ => {
            anyhow::bail!("Unexpected handshake response");
        }
    }
}

/// Accept an incoming connection and perform handshake
pub async fn accept_connection(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    local_node_info: &NodeInfo,
) -> Result<(TcpStream, NodeInfo)> {
    debug!("Accepting connection from: {}", peer_addr);

    // Receive handshake
    let message = receive_message(&mut stream).await
        .context("Failed to receive handshake")?;

    match message {
        ProtocolMessage::Handshake { node_info } => {
            // Send handshake acknowledgment
            let ack = ProtocolMessage::HandshakeAck {
                node_info: local_node_info.clone(),
            };
            send_message(&mut stream, &ack).await
                .context("Failed to send handshake ack")?;

            info!("Accepted connection from peer: {}", peer_addr);
            Ok((stream, node_info))
        }
        _ => {
            // Send error response
            let error = ProtocolMessage::Error {
                message: "Expected handshake message".to_string(),
            };
            let _ = send_message(&mut stream, &error).await;
            anyhow::bail!("Invalid handshake from peer");
        }
    }
}

/// Connection handler that manages a peer connection
pub struct PeerConnection {
    /// TCP stream
    stream: TcpStream,
    /// Peer's address
    peer_addr: SocketAddr,
    /// Peer's node information
    peer_info: NodeInfo,
}

impl PeerConnection {
    /// Create a new peer connection
    pub fn new(stream: TcpStream, peer_addr: SocketAddr, peer_info: NodeInfo) -> Self {
        Self {
            stream,
            peer_addr,
            peer_info,
        }
    }

    /// Get peer's address
    pub fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }

    /// Get peer's node information
    pub fn peer_info(&self) -> &NodeInfo {
        &self.peer_info
    }

    /// Send a gossip message to the peer
    pub async fn send_gossip(&mut self, message: GossipMessage) -> Result<()> {
        let protocol_msg = ProtocolMessage::Gossip(message);
        send_message(&mut self.stream, &protocol_msg).await
    }

    /// Receive a gossip message from the peer
    pub async fn receive_gossip(&mut self) -> Result<GossipMessage> {
        let message = receive_message(&mut self.stream).await?;

        match message {
            ProtocolMessage::Gossip(gossip_msg) => Ok(gossip_msg),
            ProtocolMessage::Error { message } => {
                anyhow::bail!("Peer sent error: {}", message);
            }
            _ => {
                anyhow::bail!("Unexpected protocol message");
            }
        }
    }

    /// Close the connection gracefully
    pub async fn close(mut self) -> Result<()> {
        self.stream.shutdown().await
            .context("Failed to shutdown connection")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    fn create_test_node_info(name: &str) -> NodeInfo {
        NodeInfo {
            public_key: "test_key".to_string(),
            address: "127.0.0.1:9000".to_string(),
            name: name.to_string(),
            version: "0.1.0".to_string(),
        }
    }

    #[tokio::test]
    async fn test_message_serialization() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let node_info = create_test_node_info("test");
        let message = ProtocolMessage::Handshake {
            node_info: node_info.clone(),
        };

        // Spawn server
        let server_handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            receive_message(&mut stream).await.unwrap()
        });

        // Connect and send
        let mut client = TcpStream::connect(addr).await.unwrap();
        send_message(&mut client, &message).await.unwrap();

        // Verify
        let received = server_handle.await.unwrap();
        match received {
            ProtocolMessage::Handshake { node_info: info } => {
                assert_eq!(info.name, "test");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[tokio::test]
    async fn test_handshake() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_info = create_test_node_info("server");
        let client_info = create_test_node_info("client");

        // Spawn server
        let server_info_clone = server_info.clone();
        let server_handle = tokio::spawn(async move {
            let (stream, peer_addr) = listener.accept().await.unwrap();
            accept_connection(stream, peer_addr, &server_info_clone).await.unwrap()
        });

        // Connect as client
        let (_, peer_info) = connect_to_peer(addr, &client_info).await.unwrap();

        // Verify server received client info
        let (_stream, client_info_at_server) = server_handle.await.unwrap();
        assert_eq!(client_info_at_server.name, "client");
        assert_eq!(peer_info.name, "server");
    }
}
