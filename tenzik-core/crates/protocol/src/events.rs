//! Shared federation event definitions.
//!
//! These types are consumed by both the runtime (which produces execution
//! receipts) and the federation crate (which persists and gossips events).

use blake3;
use chrono::Utc;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use hex;
use serde::{Deserialize, Serialize};
use tenzik_runtime::ExecutionReceipt;

use crate::errors::ProtocolError;

/// Types of events in the federation DAG.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventType {
    /// ExecutionReceipt from capsule execution
    Receipt,
    /// Node announcing itself to the network
    NodeAnnounce,
    /// Node leaving the network gracefully
    NodeLeave,
    /// Heartbeat/keepalive from node
    Heartbeat,
}

/// Content of different event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventContent {
    /// Receipt content
    Receipt(ExecutionReceipt),
    /// Node announcement content
    NodeAnnounce {
        /// Information about the announcing node
        node_info: NodeInfo,
        /// Capabilities advertised by the node
        capabilities: Vec<String>,
    },
    /// Node leave content
    NodeLeave {
        /// Optional human-readable reason for leaving
        reason: String,
    },
    /// Heartbeat content
    Heartbeat {
        /// Current load reported by the node (0.0 - 1.0)
        load: f64,
        /// Uptime in seconds
        uptime_seconds: u64,
    },
}

/// Information about a network node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeInfo {
    /// Node's public key (Ed25519)
    pub public_key: String,
    /// Network address (IP:port)
    pub address: String,
    /// Human-readable node name
    pub name: String,
    /// Software version
    pub version: String,
}

/// A single event in the federation DAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Unique event ID (Blake3 hash of content)
    pub id: String,
    /// Type of event
    pub event_type: EventType,
    /// Event content
    pub content: EventContent,
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// Parent event IDs (for DAG structure)
    pub parents: Vec<String>,
    /// Local sequence number from creator
    pub sequence: u64,
    /// ID of the node that created this event
    pub node_id: String,
    /// Ed25519 signature of the event
    pub signature: String,
}

impl Event {
    /// Create a new receipt event.
    pub fn new_receipt(
        receipt: ExecutionReceipt,
        parents: Vec<String>,
        sequence: u64,
        node_id: String,
        signing_key: &SigningKey,
    ) -> Result<Self, ProtocolError> {
        let content = EventContent::Receipt(receipt);
        let timestamp = Utc::now().to_rfc3339();

        Self::new_event(
            EventType::Receipt,
            content,
            parents,
            sequence,
            node_id,
            signing_key,
            timestamp,
        )
    }

    /// Create a new node announcement event.
    pub fn new_node_announce(
        node_info: NodeInfo,
        capabilities: Vec<String>,
        parents: Vec<String>,
        sequence: u64,
        node_id: String,
        signing_key: &SigningKey,
    ) -> Result<Self, ProtocolError> {
        let content = EventContent::NodeAnnounce {
            node_info,
            capabilities,
        };
        let timestamp = Utc::now().to_rfc3339();

        Self::new_event(
            EventType::NodeAnnounce,
            content,
            parents,
            sequence,
            node_id,
            signing_key,
            timestamp,
        )
    }

    /// Create a new heartbeat event.
    pub fn new_heartbeat(
        load: f64,
        uptime_seconds: u64,
        parents: Vec<String>,
        sequence: u64,
        node_id: String,
        signing_key: &SigningKey,
    ) -> Result<Self, ProtocolError> {
        let content = EventContent::Heartbeat {
            load,
            uptime_seconds,
        };
        let timestamp = Utc::now().to_rfc3339();

        Self::new_event(
            EventType::Heartbeat,
            content,
            parents,
            sequence,
            node_id,
            signing_key,
            timestamp,
        )
    }

    /// Create a new node leave event.
    pub fn new_node_leave(
        reason: String,
        parents: Vec<String>,
        sequence: u64,
        node_id: String,
        signing_key: &SigningKey,
    ) -> Result<Self, ProtocolError> {
        let content = EventContent::NodeLeave { reason };
        let timestamp = Utc::now().to_rfc3339();

        Self::new_event(
            EventType::NodeLeave,
            content,
            parents,
            sequence,
            node_id,
            signing_key,
            timestamp,
        )
    }

    /// Generic event creation (public method).
    pub fn new_event(
        event_type: EventType,
        content: EventContent,
        parents: Vec<String>,
        sequence: u64,
        node_id: String,
        signing_key: &SigningKey,
        timestamp: String,
    ) -> Result<Self, ProtocolError> {
        let payload = Self::create_signing_payload(
            &event_type,
            &content,
            &parents,
            sequence,
            &node_id,
            &timestamp,
        )?;

        let signature_bytes = signing_key.sign(payload.as_bytes());
        let signature = hex::encode(signature_bytes.to_bytes());

        let id = blake3::hash(payload.as_bytes()).to_hex().to_string();

        Ok(Event {
            id,
            event_type,
            content,
            timestamp,
            parents,
            sequence,
            node_id,
            signature,
        })
    }

    /// Create the payload that gets signed.
    fn create_signing_payload(
        event_type: &EventType,
        content: &EventContent,
        parents: &[String],
        sequence: u64,
        node_id: &str,
        timestamp: &str,
    ) -> Result<String, ProtocolError> {
        let content_json = serde_json::to_string(content)?;
        let parents_json = serde_json::to_string(parents)?;

        Ok(format!(
            "TENZIK_EVENT_V1\n\
             type:{:?}\n\
             content:{}\n\
             parents:{}\n\
             sequence:{}\n\
             node_id:{}\n\
             timestamp:{}",
            event_type, content_json, parents_json, sequence, node_id, timestamp
        ))
    }

    /// Verify the event signature.
    pub fn verify_signature(&self, verifying_key: &VerifyingKey) -> Result<bool, ProtocolError> {
        let payload = Self::create_signing_payload(
            &self.event_type,
            &self.content,
            &self.parents,
            self.sequence,
            &self.node_id,
            &self.timestamp,
        )?;

        let signature_bytes =
            hex::decode(&self.signature).map_err(|_| ProtocolError::InvalidFormat {
                reason: "Invalid signature hex".to_string(),
            })?;

        let signature =
            Signature::from_bytes(&signature_bytes).map_err(|_| ProtocolError::InvalidFormat {
                reason: "Invalid signature format".to_string(),
            })?;

        match verifying_key.verify(payload.as_bytes(), &signature) {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Check if this event is a receipt event.
    pub fn is_receipt(&self) -> bool {
        matches!(self.event_type, EventType::Receipt)
    }

    /// Get the receipt if this is a receipt event.
    pub fn get_receipt(&self) -> Option<&ExecutionReceipt> {
        match &self.content {
            EventContent::Receipt(receipt) => Some(receipt),
            _ => None,
        }
    }
}
