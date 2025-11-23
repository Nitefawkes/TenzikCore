//! ZK Proof Backend Module
//!
//! This module provides pluggable zero-knowledge proof backends for Tenzik receipts.
//! Proofs are optional and can be generated asynchronously to enhance receipt verification
//! without blocking execution.

use crate::receipts::ExecutionReceipt;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Proof backend errors
#[derive(Error, Debug)]
pub enum ProofError {
    #[error("Proof generation failed: {reason}")]
    GenerationFailed { reason: String },

    #[error("Proof verification failed: {reason}")]
    VerificationFailed { reason: String },

    #[error("Invalid proof format: {reason}")]
    InvalidFormat { reason: String },

    #[error("Backend not available: {backend}")]
    BackendUnavailable { backend: String },

    #[error("Serialization error: {source}")]
    SerializationError { source: serde_json::Error },
}

/// Types of proof backends available
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProofBackendType {
    /// Mock backend for testing (always succeeds)
    Mock,
    /// Risc Zero zkVM backend
    Risc0,
    /// Succinct SP1 backend
    SP1,
    /// Trusted Execution Environment (Intel SGX, etc.)
    TEE,
}

impl std::fmt::Display for ProofBackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProofBackendType::Mock => write!(f, "Mock"),
            ProofBackendType::Risc0 => write!(f, "Risc0"),
            ProofBackendType::SP1 => write!(f, "SP1"),
            ProofBackendType::TEE => write!(f, "TEE"),
        }
    }
}

/// ZK Proof structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZkProof {
    /// Type of proof backend used
    pub backend_type: ProofBackendType,
    /// The actual proof bytes (backend-specific format)
    pub proof_data: Vec<u8>,
    /// Public inputs used in proof generation
    pub public_inputs: Vec<u8>,
    /// Metadata about proof generation
    pub metadata: ProofMetadata,
}

/// Metadata about proof generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofMetadata {
    /// Backend version string
    pub backend_version: String,
    /// Time taken to generate proof (milliseconds)
    pub generation_time_ms: u64,
    /// Proof size in bytes
    pub proof_size_bytes: usize,
    /// Timestamp of proof generation (ISO 8601)
    pub timestamp: String,
}

/// Proof backend trait
///
/// This trait defines the interface for different ZK proof backends.
/// Implementations can use different proving systems (Risc0, SP1, TEE, etc.)
#[async_trait]
pub trait ProofBackend: Send + Sync {
    /// Get the type of this backend
    fn backend_type(&self) -> ProofBackendType;

    /// Generate a ZK proof for an execution receipt
    ///
    /// This may be computationally expensive and should run asynchronously.
    async fn generate_proof(&self, receipt: &ExecutionReceipt) -> Result<ZkProof, ProofError>;

    /// Verify a ZK proof
    ///
    /// This should be relatively fast (< 100ms for most backends).
    async fn verify_proof(&self, proof: &ZkProof, receipt: &ExecutionReceipt) -> Result<bool, ProofError>;

    /// Check if this backend is available and configured
    fn is_available(&self) -> bool;

    /// Get backend configuration info
    fn get_info(&self) -> BackendInfo;
}

/// Information about a proof backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendInfo {
    pub backend_type: ProofBackendType,
    pub version: String,
    pub available: bool,
    pub description: String,
}

/// Mock proof backend for testing and development
///
/// This backend generates fake proofs that always verify successfully.
/// It's useful for testing the proof pipeline without expensive ZK computation.
pub struct MockProofBackend {
    /// Simulated proof generation delay (milliseconds)
    pub simulated_delay_ms: u64,
}

impl MockProofBackend {
    /// Create a new mock backend
    pub fn new() -> Self {
        Self {
            simulated_delay_ms: 10, // Simulate 10ms proof generation
        }
    }

    /// Create a mock backend with custom delay
    pub fn with_delay(delay_ms: u64) -> Self {
        Self {
            simulated_delay_ms: delay_ms,
        }
    }
}

impl Default for MockProofBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProofBackend for MockProofBackend {
    fn backend_type(&self) -> ProofBackendType {
        ProofBackendType::Mock
    }

    async fn generate_proof(&self, receipt: &ExecutionReceipt) -> Result<ZkProof, ProofError> {
        use std::time::Instant;

        // Simulate proof generation delay
        if self.simulated_delay_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(self.simulated_delay_ms)).await;
        }

        let start = Instant::now();

        // Create fake proof data from receipt ID
        let receipt_id = receipt.receipt_id();
        let proof_data = format!("MOCK_PROOF:{}", receipt_id).into_bytes();

        // Create public inputs (commitments)
        let public_inputs = format!(
            "{}:{}:{}",
            receipt.capsule_id,
            receipt.input_commit,
            receipt.output_commit
        ).into_bytes();

        let generation_time_ms = start.elapsed().as_millis() as u64;

        Ok(ZkProof {
            backend_type: ProofBackendType::Mock,
            proof_data: proof_data.clone(),
            public_inputs,
            metadata: ProofMetadata {
                backend_version: "mock-0.1.0".to_string(),
                generation_time_ms,
                proof_size_bytes: proof_data.len(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            },
        })
    }

    async fn verify_proof(&self, proof: &ZkProof, receipt: &ExecutionReceipt) -> Result<bool, ProofError> {
        // Check backend type matches
        if proof.backend_type != ProofBackendType::Mock {
            return Err(ProofError::InvalidFormat {
                reason: format!("Expected Mock proof, got {:?}", proof.backend_type),
            });
        }

        // Verify proof data format
        let proof_str = String::from_utf8(proof.proof_data.clone())
            .map_err(|e| ProofError::InvalidFormat {
                reason: format!("Invalid proof data UTF-8: {}", e),
            })?;

        if !proof_str.starts_with("MOCK_PROOF:") {
            return Ok(false);
        }

        // Verify public inputs match receipt commitments
        let expected_inputs = format!(
            "{}:{}:{}",
            receipt.capsule_id,
            receipt.input_commit,
            receipt.output_commit
        ).into_bytes();

        Ok(proof.public_inputs == expected_inputs)
    }

    fn is_available(&self) -> bool {
        true // Mock backend is always available
    }

    fn get_info(&self) -> BackendInfo {
        BackendInfo {
            backend_type: ProofBackendType::Mock,
            version: "0.1.0".to_string(),
            available: true,
            description: "Mock proof backend for testing (generates fake proofs)".to_string(),
        }
    }
}

/// Proof backend factory
pub struct ProofBackendFactory;

impl ProofBackendFactory {
    /// Create a proof backend by type
    pub fn create(backend_type: ProofBackendType) -> Result<Box<dyn ProofBackend>, ProofError> {
        match backend_type {
            ProofBackendType::Mock => Ok(Box::new(MockProofBackend::new())),
            ProofBackendType::Risc0 => Err(ProofError::BackendUnavailable {
                backend: "Risc0 backend not yet implemented".to_string(),
            }),
            ProofBackendType::SP1 => Err(ProofError::BackendUnavailable {
                backend: "SP1 backend not yet implemented".to_string(),
            }),
            ProofBackendType::TEE => Err(ProofError::BackendUnavailable {
                backend: "TEE backend not yet implemented".to_string(),
            }),
        }
    }

    /// Get list of available backends
    pub fn available_backends() -> Vec<ProofBackendType> {
        vec![ProofBackendType::Mock]
        // Future: Add Risc0, SP1, TEE when implemented
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::receipts::{ExecMetrics, ExecutionReceipt};
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    fn generate_test_signing_key() -> SigningKey {
        SigningKey::generate(&mut OsRng)
    }

    #[tokio::test]
    async fn test_mock_backend_proof_generation() {
        let backend = MockProofBackend::new();
        let signing_key = generate_test_signing_key();

        let receipt = ExecutionReceipt::new(
            b"test capsule",
            b"input",
            b"output",
            ExecMetrics::default(),
            &signing_key,
            42,
        ).unwrap();

        let proof = backend.generate_proof(&receipt).await.unwrap();

        assert_eq!(proof.backend_type, ProofBackendType::Mock);
        assert!(!proof.proof_data.is_empty());
        assert!(!proof.public_inputs.is_empty());
    }

    #[tokio::test]
    async fn test_mock_backend_proof_verification() {
        let backend = MockProofBackend::new();
        let signing_key = generate_test_signing_key();

        let receipt = ExecutionReceipt::new(
            b"test capsule",
            b"input",
            b"output",
            ExecMetrics::default(),
            &signing_key,
            42,
        ).unwrap();

        let proof = backend.generate_proof(&receipt).await.unwrap();
        let is_valid = backend.verify_proof(&proof, &receipt).await.unwrap();

        assert!(is_valid);
    }

    #[tokio::test]
    async fn test_mock_backend_invalid_proof() {
        let backend = MockProofBackend::new();
        let signing_key = generate_test_signing_key();

        let receipt1 = ExecutionReceipt::new(
            b"capsule1",
            b"input1",
            b"output1",
            ExecMetrics::default(),
            &signing_key,
            42,
        ).unwrap();

        let receipt2 = ExecutionReceipt::new(
            b"capsule2",
            b"input2",
            b"output2",
            ExecMetrics::default(),
            &signing_key,
            43,
        ).unwrap();

        // Generate proof for receipt1
        let proof = backend.generate_proof(&receipt1).await.unwrap();

        // Try to verify against receipt2 (should fail)
        let is_valid = backend.verify_proof(&proof, &receipt2).await.unwrap();
        assert!(!is_valid);
    }

    #[tokio::test]
    async fn test_mock_backend_info() {
        let backend = MockProofBackend::new();
        let info = backend.get_info();

        assert_eq!(info.backend_type, ProofBackendType::Mock);
        assert!(info.available);
        assert!(!info.version.is_empty());
    }

    #[test]
    fn test_proof_backend_factory() {
        let backend = ProofBackendFactory::create(ProofBackendType::Mock).unwrap();
        assert_eq!(backend.backend_type(), ProofBackendType::Mock);
        assert!(backend.is_available());
    }

    #[test]
    fn test_proof_backend_factory_unavailable() {
        let result = ProofBackendFactory::create(ProofBackendType::Risc0);
        assert!(result.is_err());

        let result = ProofBackendFactory::create(ProofBackendType::SP1);
        assert!(result.is_err());

        let result = ProofBackendFactory::create(ProofBackendType::TEE);
        assert!(result.is_err());
    }

    #[test]
    fn test_available_backends() {
        let backends = ProofBackendFactory::available_backends();
        assert!(backends.contains(&ProofBackendType::Mock));
        assert_eq!(backends.len(), 1); // Only Mock is currently available
    }

    #[tokio::test]
    async fn test_mock_backend_with_delay() {
        use std::time::Instant;

        let backend = MockProofBackend::with_delay(50);
        let signing_key = generate_test_signing_key();

        let receipt = ExecutionReceipt::new(
            b"test",
            b"input",
            b"output",
            ExecMetrics::default(),
            &signing_key,
            42,
        ).unwrap();

        let start = Instant::now();
        backend.generate_proof(&receipt).await.unwrap();
        let elapsed = start.elapsed();

        // Should take at least the simulated delay
        assert!(elapsed.as_millis() >= 50);
    }

    #[test]
    fn test_zk_proof_serialization() {
        let proof = ZkProof {
            backend_type: ProofBackendType::Mock,
            proof_data: b"test proof".to_vec(),
            public_inputs: b"test inputs".to_vec(),
            metadata: ProofMetadata {
                backend_version: "test-1.0".to_string(),
                generation_time_ms: 100,
                proof_size_bytes: 10,
                timestamp: chrono::Utc::now().to_rfc3339(),
            },
        };

        let json = serde_json::to_string(&proof).unwrap();
        let deserialized: ZkProof = serde_json::from_str(&json).unwrap();

        assert_eq!(proof.backend_type, deserialized.backend_type);
        assert_eq!(proof.proof_data, deserialized.proof_data);
    }
}
