//! ProofBackend trait and related types
//!
//! This module defines the abstraction for zero-knowledge proof generation
//! and verification.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tenzik_runtime::ExecutionReceipt;

/// Errors that can occur during proof operations
#[derive(Error, Debug, Clone)]
pub enum ProofError {
    #[error("Proof generation failed: {reason}")]
    GenerationFailed { reason: String },

    #[error("Proof verification failed: {reason}")]
    VerificationFailed { reason: String },

    #[error("Invalid proof format: {reason}")]
    InvalidFormat { reason: String },

    #[error("Backend not initialized: {reason}")]
    NotInitialized { reason: String },

    #[error("Proof timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("Resource limit exceeded: {resource}")]
    ResourceLimit { resource: String },
}

/// Status of a proof operation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProofStatus {
    /// Proof generation pending
    Pending,
    /// Proof generation in progress
    InProgress,
    /// Proof completed successfully
    Completed,
    /// Proof generation failed
    Failed { reason: String },
}

/// Request to generate a proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofRequest {
    /// The execution receipt to prove
    pub receipt: ExecutionReceipt,
    /// Optional timeout in milliseconds
    pub timeout_ms: Option<u64>,
    /// Priority level (higher = more urgent)
    pub priority: u8,
}

/// Response containing a generated proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofResponse {
    /// The receipt that was proven
    pub receipt: ExecutionReceipt,
    /// The zero-knowledge proof (implementation-specific format)
    pub proof: Vec<u8>,
    /// Time taken to generate proof (milliseconds)
    pub generation_time_ms: u64,
    /// Proof system identifier (e.g., "groth16", "plonk", "mock")
    pub proof_system: String,
}

/// Trait for zero-knowledge proof backends
///
/// This trait abstracts the proof generation and verification process,
/// allowing different ZK systems (Groth16, PLONK, etc.) to be used
/// interchangeably.
#[async_trait]
pub trait ProofBackend: Send + Sync {
    /// Initialize the proof backend
    ///
    /// This may involve loading proving/verifying keys, setting up
    /// cryptographic parameters, etc.
    async fn initialize(&mut self) -> Result<(), ProofError>;

    /// Generate a zero-knowledge proof for an execution receipt
    ///
    /// # Arguments
    /// * `request` - The proof generation request
    ///
    /// # Returns
    /// A proof response containing the proof and metadata
    async fn generate_proof(&self, request: ProofRequest) -> Result<ProofResponse, ProofError>;

    /// Verify a zero-knowledge proof
    ///
    /// # Arguments
    /// * `receipt` - The execution receipt to verify
    /// * `proof` - The proof to verify
    ///
    /// # Returns
    /// `Ok(true)` if the proof is valid, `Ok(false)` if invalid,
    /// or `Err` if verification could not be performed
    async fn verify_proof(
        &self,
        receipt: &ExecutionReceipt,
        proof: &[u8],
    ) -> Result<bool, ProofError>;

    /// Get the proof system identifier
    ///
    /// # Returns
    /// A string identifying the proof system (e.g., "groth16", "plonk")
    fn proof_system(&self) -> &str;

    /// Check if the backend is initialized
    fn is_initialized(&self) -> bool;

    /// Get backend statistics
    fn stats(&self) -> BackendStats;
}

/// Statistics about proof backend performance
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackendStats {
    /// Total proofs generated
    pub total_proofs_generated: u64,
    /// Total proofs verified
    pub total_proofs_verified: u64,
    /// Total proofs failed
    pub total_proofs_failed: u64,
    /// Average proof generation time (milliseconds)
    pub avg_generation_time_ms: f64,
    /// Average proof verification time (milliseconds)
    pub avg_verification_time_ms: f64,
}

impl BackendStats {
    /// Record a successful proof generation
    pub fn record_generation(&mut self, time_ms: u64) {
        self.total_proofs_generated += 1;
        // Update running average
        let total = self.total_proofs_generated as f64;
        self.avg_generation_time_ms =
            (self.avg_generation_time_ms * (total - 1.0) + time_ms as f64) / total;
    }

    /// Record a successful proof verification
    pub fn record_verification(&mut self, time_ms: u64) {
        self.total_proofs_verified += 1;
        // Update running average
        let total = self.total_proofs_verified as f64;
        self.avg_verification_time_ms =
            (self.avg_verification_time_ms * (total - 1.0) + time_ms as f64) / total;
    }

    /// Record a failed proof
    pub fn record_failure(&mut self) {
        self.total_proofs_failed += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_stats() {
        let mut stats = BackendStats::default();
        assert_eq!(stats.total_proofs_generated, 0);

        stats.record_generation(100);
        assert_eq!(stats.total_proofs_generated, 1);
        assert_eq!(stats.avg_generation_time_ms, 100.0);

        stats.record_generation(200);
        assert_eq!(stats.total_proofs_generated, 2);
        assert_eq!(stats.avg_generation_time_ms, 150.0);
    }

    #[test]
    fn test_proof_status() {
        let status = ProofStatus::Pending;
        assert_eq!(status, ProofStatus::Pending);

        let failed = ProofStatus::Failed {
            reason: "test".to_string(),
        };
        assert!(matches!(failed, ProofStatus::Failed { .. }));
    }
}
