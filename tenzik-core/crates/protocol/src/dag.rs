//! Shared DAG-related structures.
//!
//! These helper types describe metadata that multiple crates need to
//! understand, such as statistics produced by DAG storage implementations.

/// DAG statistics for monitoring and observability.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DAGStats {
    /// Total number of events in the DAG
    pub total_events: usize,
    /// Number of tips (events with no children)
    pub tip_count: usize,
    /// Number of receipt events
    pub receipt_count: usize,
    /// Number of unique nodes seen
    pub node_count: usize,
    /// Earliest event timestamp
    pub earliest_timestamp: Option<String>,
    /// Latest event timestamp
    pub latest_timestamp: Option<String>,
}
