//! Node command implementation
//!
//! This module implements the `tenzik node` command for starting federation nodes
//! and managing peer connections.

use anyhow::{Context, Result};
use std::net::SocketAddr;
use std::path::Path;
use tenzik_federation::{TenzikNode, NodeConfig};
use tokio::signal;

/// Arguments for the node command
pub struct NodeArgs {
    /// Port to listen on
    pub port: u16,
    /// Peer address to connect to
    pub peer: Option<String>,
    /// Local database path
    pub db: String,
    /// Node name
    pub name: Option<String>,
}

/// Execute the node command
pub async fn execute_node_command(args: NodeArgs) -> Result<()> {
    println!("🌐 Starting Tenzik federation node...");
    println!("📁 Database: {}", args.db);
    println!("🔌 Port: {}", args.port);
    
    if let Some(peer) = &args.peer {
        println!("🤝 Initial peer: {}", peer);
    }
    println!();

    // Initialize tracing for the node
    tracing_subscriber::fmt().init();

    // Parse listen address
    let listen_addr: SocketAddr = format!("127.0.0.1:{}", args.port)
        .parse()
        .context("Invalid port number")?;

    // Parse initial peers
    let mut initial_peers = Vec::new();
    if let Some(peer_str) = args.peer {
        let peer_addr: SocketAddr = peer_str
            .parse()
            .context("Invalid peer address format")?;
        initial_peers.push(peer_addr);
    }

    // Create node configuration
    let config = NodeConfig {
        listen_addr,
        db_path: args.db.clone(),
        name: args.name.unwrap_or_else(|| format!("tenzik-node-{}", args.port)),
        initial_peers,
        signing_key: None, // Generate new key
    };

    // Create and start the node
    let mut node = TenzikNode::new(config)
        .context("Failed to create Tenzik node")?;

    println!("🔑 Node public key: {}", hex::encode(node.public_key().as_bytes()));
    println!("📡 Node listening on: {}", node.listen_address());
    println!();

    // Start the node
    node.start().await
        .context("Failed to start Tenzik node")?;

    println!("✅ Node started successfully!");
    println!("📊 Initial DAG stats: {:?}", node.get_dag_stats().await?);
    println!();

    // Print status information
    print_node_status(&node).await;

    // Wait for shutdown signal
    println!("🔄 Node running... Press Ctrl+C to shutdown");
    wait_for_shutdown().await;

    // Graceful shutdown
    println!("\n🛑 Shutting down node...");
    node.shutdown().await
        .context("Failed to shutdown node gracefully")?;

    println!("✅ Node shutdown complete");
    Ok(())
}

/// Print current node status
async fn print_node_status(node: &TenzikNode) {
    println!("📈 Node Status:");
    println!("   Connected peers: {}", node.get_connected_peers().await.len());

    if let Ok(stats) = node.get_dag_stats().await {
        println!("   DAG events: {}", stats.total_events);
        println!("   DAG tips: {}", stats.tip_count);
        println!("   Receipt count: {}", stats.receipt_count);
        println!("   Node count: {}", stats.node_count);
    }

    println!();
}

/// Wait for shutdown signal (Ctrl+C)
async fn wait_for_shutdown() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

/// Validate database path
pub fn validate_db_path(db_path: &str) -> Result<()> {
    let path = Path::new(db_path);

    // Get the directory where the database will be stored
    let db_dir = if path.extension().is_some() || !path.to_string_lossy().ends_with('/') {
        // It's a file path, use parent directory
        path.parent().unwrap_or(Path::new("."))
    } else {
        // It's a directory path
        path
    };

    // Create directory if it doesn't exist
    if !db_dir.exists() {
        std::fs::create_dir_all(db_dir)
            .with_context(|| format!("Failed to create database directory: {}", db_dir.display()))?;
    }

    // Check write permissions by trying to create a test file in the directory
    let test_file = db_dir.join(".tenzik_write_test");
    match std::fs::write(&test_file, b"test") {
        Ok(_) => {
            let _ = std::fs::remove_file(&test_file);
            Ok(())
        }
        Err(e) => {
            anyhow::bail!("Cannot write to database directory {}: {}", db_dir.display(), e);
        }
    }
}

/// Parse peer address with helpful error messages
pub fn parse_peer_address(peer_str: &str) -> Result<SocketAddr> {
    peer_str.parse()
        .with_context(|| {
            format!(
                "Invalid peer address '{}'. Expected format: IP:PORT (e.g., 127.0.0.1:9001)",
                peer_str
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_validate_db_path() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_db").to_string_lossy().to_string();
        
        // Should succeed for valid path
        assert!(validate_db_path(&db_path).is_ok());
    }
    
    #[test]
    fn test_parse_peer_address() {
        // Valid addresses
        assert!(parse_peer_address("127.0.0.1:9000").is_ok());
        assert!(parse_peer_address("192.168.1.1:8080").is_ok());
        
        // Invalid addresses
        assert!(parse_peer_address("invalid").is_err());
        assert!(parse_peer_address("127.0.0.1").is_err());
        assert!(parse_peer_address("127.0.0.1:99999").is_err());
    }
    
    #[test]
    fn test_node_args() {
        let args = NodeArgs {
            port: 9000,
            peer: Some("127.0.0.1:9001".to_string()),
            db: ".tenzik".to_string(),
            name: Some("test-node".to_string()),
        };
        
        assert_eq!(args.port, 9000);
        assert_eq!(args.peer.as_ref().unwrap(), "127.0.0.1:9001");
        assert_eq!(args.name.as_ref().unwrap(), "test-node");
    }
}
