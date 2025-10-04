//! Tenzik protocol types and DAG federation
//!
//! This crate defines the core protocol types for Tenzik federation,
//! including ExecutionReceipts, Events, and DAG structures.

pub mod errors;

// Simple re-exports for now
pub use errors::ProtocolError;

// Type aliases for federation types (defined in federation crate)
pub type ExecutionReceipt = String; // Placeholder
pub type ExecMetrics = String; // Placeholder
pub type Event = String; // Placeholder
pub type EventType = String; // Placeholder
pub type EventDAG = String; // Placeholder
pub type NodeInfo = String; // Placeholder

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
