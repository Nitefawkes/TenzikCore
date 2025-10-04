//! Event DAG and Storage Module
//!
//! This module implements a simple Directed Acyclic Graph (DAG) for storing
//! and organizing federation events, with persistent storage using sled.

use anyhow::{Context, Result};
use sled::{Db, Tree};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tenzik_protocol::{DAGStats, Event, ExecutionReceipt, NodeInfo};
use thiserror::Error;

/// Storage-related errors
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {source}")]
    DatabaseError { source: sled::Error },

    #[error("Event not found: {event_id}")]
    EventNotFound { event_id: String },

    #[error("Invalid event: {reason}")]
    InvalidEvent { reason: String },

    #[error("Serialization error: {source}")]
    SerializationError { source: serde_json::Error },

    #[error("Validation error: {reason}")]
    ValidationError { reason: String },

    #[error("DAG constraint violation: {reason}")]
    DAGViolation { reason: String },
}

/// Event DAG with persistent storage
pub struct EventDAG {
    /// Main database
    db: Db,
    /// Events tree (event_id -> Event)
    events: Tree,
    /// Parents tree (event_id -> Vec<parent_ids>)
    parents: Tree,
    /// Children tree (event_id -> Vec<child_ids>)
    children: Tree,
    /// Tips tree (event_id -> timestamp)
    tips: Tree,
    /// Sequence tree (node_id -> latest_sequence)
    sequences: Tree,
}

impl EventDAG {
    /// Create or open an EventDAG with the given database path
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self, StorageError> {
        let db = sled::open(db_path).map_err(|e| StorageError::DatabaseError { source: e })?;

        let events = db
            .open_tree("events")
            .map_err(|e| StorageError::DatabaseError { source: e })?;

        let parents = db
            .open_tree("parents")
            .map_err(|e| StorageError::DatabaseError { source: e })?;

        let children = db
            .open_tree("children")
            .map_err(|e| StorageError::DatabaseError { source: e })?;

        let tips = db
            .open_tree("tips")
            .map_err(|e| StorageError::DatabaseError { source: e })?;

        let sequences = db
            .open_tree("sequences")
            .map_err(|e| StorageError::DatabaseError { source: e })?;

        Ok(EventDAG {
            db,
            events,
            parents,
            children,
            tips,
            sequences,
        })
    }

    /// Add an event to the DAG
    pub fn add_event(&mut self, event: Event) -> Result<(), StorageError> {
        // Validate event
        self.validate_event(&event)?;

        // Check if event already exists
        if self.has_event(&event.id)? {
            return Ok(()); // Already exists, ignore
        }

        // Validate parents exist
        for parent_id in &event.parents {
            if !self.has_event(parent_id)? {
                return Err(StorageError::ValidationError {
                    reason: format!("Parent event {} not found", parent_id),
                });
            }
        }

        // Update sequence tracking
        self.update_sequence(&event.node_id, event.sequence)?;

        // Store the event
        let event_json = serde_json::to_string(&event)
            .map_err(|e| StorageError::SerializationError { source: e })?;

        self.events
            .insert(&event.id, event_json.as_bytes())
            .map_err(|e| StorageError::DatabaseError { source: e })?;

        // Update parent-child relationships
        self.update_relationships(&event)?;

        // Update tips
        self.update_tips(&event)?;

        // Flush changes
        self.db
            .flush()
            .map_err(|e| StorageError::DatabaseError { source: e })?;

        Ok(())
    }

    /// Get an event by ID
    pub fn get_event(&self, event_id: &str) -> Result<Option<Event>, StorageError> {
        let event_bytes = match self
            .events
            .get(event_id)
            .map_err(|e| StorageError::DatabaseError { source: e })?
        {
            Some(bytes) => bytes,
            None => return Ok(None),
        };

        let event_json =
            String::from_utf8(event_bytes.to_vec()).map_err(|_| StorageError::InvalidEvent {
                reason: "Invalid UTF-8 in event data".to_string(),
            })?;

        let event: Event = serde_json::from_str(&event_json)
            .map_err(|e| StorageError::SerializationError { source: e })?;

        Ok(Some(event))
    }

    /// Check if an event exists
    pub fn has_event(&self, event_id: &str) -> Result<bool, StorageError> {
        Ok(self
            .events
            .contains_key(event_id)
            .map_err(|e| StorageError::DatabaseError { source: e })?)
    }

    /// Get current tips (events with no children)
    pub fn get_tips(&self) -> Result<Vec<Event>, StorageError> {
        let mut tips = Vec::new();

        for result in self.tips.iter() {
            let (event_id_bytes, _timestamp_bytes) =
                result.map_err(|e| StorageError::DatabaseError { source: e })?;

            let event_id = String::from_utf8(event_id_bytes.to_vec()).map_err(|_| {
                StorageError::InvalidEvent {
                    reason: "Invalid UTF-8 in tip ID".to_string(),
                }
            })?;

            if let Some(event) = self.get_event(&event_id)? {
                tips.push(event);
            }
        }

        // Sort by timestamp (latest first)
        tips.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(tips)
    }

    /// Get events since a specific event ID
    pub fn get_events_since(
        &self,
        since_event_id: Option<&str>,
    ) -> Result<Vec<Event>, StorageError> {
        let mut events = Vec::new();
        let mut seen = HashSet::new();

        // If no since_event_id, return all events
        if since_event_id.is_none() {
            for result in self.events.iter() {
                let (_, event_bytes) =
                    result.map_err(|e| StorageError::DatabaseError { source: e })?;

                let event_json = String::from_utf8(event_bytes.to_vec()).map_err(|_| {
                    StorageError::InvalidEvent {
                        reason: "Invalid UTF-8 in event data".to_string(),
                    }
                })?;

                let event: Event = serde_json::from_str(&event_json)
                    .map_err(|e| StorageError::SerializationError { source: e })?;

                events.push(event);
            }
        } else {
            // TODO: Implement efficient since-based retrieval
            // For now, return all events (simple implementation)
            return self.get_events_since(None);
        }

        // Sort by timestamp
        events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        Ok(events)
    }

    /// Get DAG statistics
    pub fn get_stats(&self) -> Result<DAGStats, StorageError> {
        let mut receipt_count = 0;
        let mut nodes = HashSet::new();
        let mut earliest_timestamp: Option<String> = None;
        let mut latest_timestamp: Option<String> = None;

        let total_events = self.events.len();
        let tip_count = self.tips.len();

        for result in self.events.iter() {
            let (_, event_bytes) = result.map_err(|e| StorageError::DatabaseError { source: e })?;

            let event_json = String::from_utf8(event_bytes.to_vec()).map_err(|_| {
                StorageError::InvalidEvent {
                    reason: "Invalid UTF-8 in event data".to_string(),
                }
            })?;

            let event: Event = serde_json::from_str(&event_json)
                .map_err(|e| StorageError::SerializationError { source: e })?;

            if event.is_receipt() {
                receipt_count += 1;
            }

            nodes.insert(event.node_id);

            if earliest_timestamp.is_none()
                || event.timestamp < earliest_timestamp.as_ref().unwrap().clone()
            {
                earliest_timestamp = Some(event.timestamp.clone());
            }

            if latest_timestamp.is_none()
                || event.timestamp > latest_timestamp.as_ref().unwrap().clone()
            {
                latest_timestamp = Some(event.timestamp.clone());
            }
        }

        Ok(DAGStats {
            total_events,
            tip_count,
            receipt_count,
            node_count: nodes.len(),
            earliest_timestamp,
            latest_timestamp,
        })
    }

    /// Validate an event
    fn validate_event(&self, event: &Event) -> Result<(), StorageError> {
        // Check ID format
        if event.id.len() != 64 {
            return Err(StorageError::ValidationError {
                reason: "Event ID must be 64-character hex string".to_string(),
            });
        }

        // Check signature format
        if event.signature.len() != 128 {
            return Err(StorageError::ValidationError {
                reason: "Signature must be 128-character hex string".to_string(),
            });
        }

        // Check timestamp format
        if chrono::DateTime::parse_from_rfc3339(&event.timestamp).is_err() {
            return Err(StorageError::ValidationError {
                reason: "Invalid timestamp format".to_string(),
            });
        }

        Ok(())
    }

    /// Update sequence tracking for a node
    fn update_sequence(&mut self, node_id: &str, sequence: u64) -> Result<(), StorageError> {
        let current_sequence = self.get_node_sequence(node_id)?;

        if sequence <= current_sequence {
            return Err(StorageError::ValidationError {
                reason: format!(
                    "Sequence {} is not greater than current {} for node {}",
                    sequence, current_sequence, node_id
                ),
            });
        }

        self.sequences
            .insert(node_id, &sequence.to_be_bytes())
            .map_err(|e| StorageError::DatabaseError { source: e })?;

        Ok(())
    }

    /// Get the latest sequence number for a node
    fn get_node_sequence(&self, node_id: &str) -> Result<u64, StorageError> {
        match self
            .sequences
            .get(node_id)
            .map_err(|e| StorageError::DatabaseError { source: e })?
        {
            Some(bytes) => {
                let seq_bytes: [u8; 8] =
                    bytes
                        .as_ref()
                        .try_into()
                        .map_err(|_| StorageError::InvalidEvent {
                            reason: "Invalid sequence format".to_string(),
                        })?;
                Ok(u64::from_be_bytes(seq_bytes))
            }
            None => Ok(0),
        }
    }

    /// Update parent-child relationships
    fn update_relationships(&mut self, event: &Event) -> Result<(), StorageError> {
        // Store parents
        let parents_json = serde_json::to_string(&event.parents)
            .map_err(|e| StorageError::SerializationError { source: e })?;

        self.parents
            .insert(&event.id, parents_json.as_bytes())
            .map_err(|e| StorageError::DatabaseError { source: e })?;

        // Update children for each parent
        for parent_id in &event.parents {
            let mut children: Vec<String> = match self
                .children
                .get(parent_id)
                .map_err(|e| StorageError::DatabaseError { source: e })?
            {
                Some(bytes) => {
                    let json = String::from_utf8(bytes.to_vec()).map_err(|_| {
                        StorageError::InvalidEvent {
                            reason: "Invalid UTF-8 in children data".to_string(),
                        }
                    })?;
                    serde_json::from_str(&json)
                        .map_err(|e| StorageError::SerializationError { source: e })?
                }
                None => Vec::new(),
            };

            children.push(event.id.clone());

            let children_json = serde_json::to_string(&children)
                .map_err(|e| StorageError::SerializationError { source: e })?;

            self.children
                .insert(parent_id, children_json.as_bytes())
                .map_err(|e| StorageError::DatabaseError { source: e })?;
        }

        Ok(())
    }

    /// Update tips
    fn update_tips(&mut self, event: &Event) -> Result<(), StorageError> {
        // Remove parents from tips (they now have children)
        for parent_id in &event.parents {
            self.tips
                .remove(parent_id)
                .map_err(|e| StorageError::DatabaseError { source: e })?;
        }

        // Add this event as a tip
        self.tips
            .insert(&event.id, event.timestamp.as_bytes())
            .map_err(|e| StorageError::DatabaseError { source: e })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_signing_key() -> ed25519_dalek::SigningKey {
        use rand::rngs::OsRng;
        ed25519_dalek::SigningKey::generate(&mut OsRng)
    }

    fn create_test_receipt() -> ExecutionReceipt {
        let signing_key = create_test_signing_key();
        ExecutionReceipt::new(
            b"test capsule",
            b"test input",
            b"test output",
            tenzik_protocol::ExecMetrics::default(),
            &signing_key,
            1,
        )
        .unwrap()
    }

    #[test]
    fn test_event_creation_and_verification() {
        let signing_key = create_test_signing_key();
        let verifying_key = signing_key.verifying_key();
        let receipt = create_test_receipt();

        let event =
            Event::new_receipt(receipt, vec![], 1, "test_node".to_string(), &signing_key).unwrap();

        assert!(event.verify_signature(&verifying_key).unwrap());
        assert!(event.is_receipt());
        assert!(event.get_receipt().is_some());
    }

    #[test]
    fn test_dag_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let mut dag = EventDAG::new(temp_dir.path()).unwrap();

        let signing_key = create_test_signing_key();
        let receipt = create_test_receipt();

        let event =
            Event::new_receipt(receipt, vec![], 1, "test_node".to_string(), &signing_key).unwrap();

        let event_id = event.id.clone();

        // Add event
        dag.add_event(event).unwrap();

        // Retrieve event
        let retrieved = dag.get_event(&event_id).unwrap().unwrap();
        assert_eq!(retrieved.id, event_id);

        // Check it exists
        assert!(dag.has_event(&event_id).unwrap());

        // Check tips
        let tips = dag.get_tips().unwrap();
        assert_eq!(tips.len(), 1);
        assert_eq!(tips[0].id, event_id);
    }

    #[test]
    fn test_dag_parent_child_relationships() {
        let temp_dir = TempDir::new().unwrap();
        let mut dag = EventDAG::new(temp_dir.path()).unwrap();

        let signing_key = create_test_signing_key();

        // Create first event
        let event1 = Event::new_receipt(
            create_test_receipt(),
            vec![],
            1,
            "test_node".to_string(),
            &signing_key,
        )
        .unwrap();
        let event1_id = event1.id.clone();

        dag.add_event(event1).unwrap();

        // Create second event with first as parent
        let event2 = Event::new_receipt(
            create_test_receipt(),
            vec![event1_id.clone()],
            2,
            "test_node".to_string(),
            &signing_key,
        )
        .unwrap();
        let event2_id = event2.id.clone();

        dag.add_event(event2).unwrap();

        // Check tips - should only be event2 now
        let tips = dag.get_tips().unwrap();
        assert_eq!(tips.len(), 1);
        assert_eq!(tips[0].id, event2_id);
    }

    #[test]
    fn test_dag_sequence_validation() {
        let temp_dir = TempDir::new().unwrap();
        let mut dag = EventDAG::new(temp_dir.path()).unwrap();

        let signing_key = create_test_signing_key();

        // Add event with sequence 1
        let event1 = Event::new_receipt(
            create_test_receipt(),
            vec![],
            1,
            "test_node".to_string(),
            &signing_key,
        )
        .unwrap();

        dag.add_event(event1).unwrap();

        // Try to add event with same sequence - should fail
        let event2 = Event::new_receipt(
            create_test_receipt(),
            vec![],
            1,
            "test_node".to_string(),
            &signing_key,
        )
        .unwrap();

        let result = dag.add_event(event2);
        assert!(result.is_err());
    }

    #[test]
    fn test_dag_stats() {
        let temp_dir = TempDir::new().unwrap();
        let mut dag = EventDAG::new(temp_dir.path()).unwrap();

        let signing_key = create_test_signing_key();

        // Add a few events
        for i in 1..=3 {
            let event = Event::new_receipt(
                create_test_receipt(),
                vec![],
                i,
                format!("node_{}", i),
                &signing_key,
            )
            .unwrap();

            dag.add_event(event).unwrap();
        }

        let stats = dag.get_stats().unwrap();
        assert_eq!(stats.total_events, 3);
        assert_eq!(stats.receipt_count, 3);
        assert_eq!(stats.node_count, 3);
        assert_eq!(stats.tip_count, 3); // All independent events
    }
}
