//! Protocol errors module

use thiserror::Error;

/// Protocol-level errors
#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("Serialization error: {source}")]
    SerializationError { source: serde_json::Error },
    
    #[error("Invalid format: {reason}")]
    InvalidFormat { reason: String },
    
    #[error("Validation failed: {reason}")]
    ValidationFailed { reason: String },
    
    #[error("Cryptographic error: {reason}")]
    CryptographicError { reason: String },
    
    #[error("Network error: {reason}")]
    NetworkError { reason: String },
    
    #[error("Storage error: {reason}")]
    StorageError { reason: String },
}

impl From<serde_json::Error> for ProtocolError {
    fn from(err: serde_json::Error) -> Self {
        ProtocolError::SerializationError { source: err }
    }
}
