//! Tenzik protocol types and DAG federation
//!
//! This crate defines the core protocol types for Tenzik federation,
//! including execution receipts, events, and DAG structures shared across
//! the runtime and federation components.

pub mod dag;
pub mod errors;
pub mod events;

// Simple re-exports for now
pub use dag::DAGStats;
pub use errors::ProtocolError;
pub use events::{Event, EventContent, EventType, NodeInfo};
pub use tenzik_runtime::{ExecMetrics, ExecutionReceipt};

/// Result type for protocol operations
pub type Result<T> = std::result::Result<T, ProtocolError>;

/// Version of the Tenzik protocol
pub const PROTOCOL_VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_version() {
        assert!(!PROTOCOL_VERSION.is_empty());
    }
}
