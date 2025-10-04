//! Simple DAG-based federation and gossip
//!
//! This crate implements a minimal federated event system using a DAG structure
//! for receipt exchange between Tenzik nodes.

pub mod storage;
pub mod node;
pub mod gossip;

// Re-export key types
pub use storage::{Event, EventDAG, EventType, EventContent, NodeInfo, StorageError, DAGStats};
pub use node::{TenzikNode, NodeConfig};
pub use gossip::{GossipProtocol, PeerInfo};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
