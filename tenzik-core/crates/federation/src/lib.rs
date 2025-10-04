//! Simple DAG-based federation and gossip
//!
//! This crate implements a minimal federated event system using a DAG structure
//! for receipt exchange between Tenzik nodes.

pub mod gossip;
pub mod node;
pub mod storage;

// Re-export key types
pub use gossip::{GossipProtocol, PeerInfo};
pub use node::{NodeConfig, TenzikNode};
pub use storage::{EventDAG, StorageError};
pub use tenzik_protocol::{DAGStats, Event, EventContent, EventType, NodeInfo};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
