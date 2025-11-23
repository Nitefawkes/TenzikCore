//! Mock proof backend for testing and development
//!
//! This module provides a simple mock implementation of the ProofBackend
//! trait that simulates proof generation without actual ZK computation.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{debug, info};

use crate::backend::{BackendStats, ProofBackend, ProofError, ProofRequest, ProofResponse};
use tenzik_runtime::ExecutionReceipt;

/// Mock proof backend for testing
///
/// This backend simulates proof generation by:
/// - Adding a configurable delay
/// - Generating a simple "proof" (hash of receipt)
/// - Always returning successful verification
pub struct MockProofBackend {
    /// Whether the backend is initialized
    initialized: bool,
    /// Statistics
    stats: Arc<RwLock<BackendStats>>,
    /// Simulated delay for proof generation (milliseconds)
    generation_delay_ms: u64,
    /// Simulated delay for verification (milliseconds)
    verification_delay_ms: u64,
    /// Whether to simulate failures
    simulate_failure: bool,
}

impl MockProofBackend {
    /// Create a new mock backend with default settings
    pub fn new() -> Self {
        Self {
            initialized: false,
            stats: Arc::new(RwLock::new(BackendStats::default())),
            generation_delay_ms: 100, // 100ms simulated delay
            verification_delay_ms: 10, // 10ms simulated delay
            simulate_failure: false,
        }
    }

    /// Create a new mock backend with custom delays
    pub fn with_delays(generation_delay_ms: u64, verification_delay_ms: u64) -> Self {
        Self {
            initialized: false,
            stats: Arc::new(RwLock::new(BackendStats::default())),
            generation_delay_ms,
            verification_delay_ms,
            simulate_failure: false,
        }
    }

    /// Enable failure simulation for testing error handling
    pub fn with_failure_simulation(mut self) -> Self {
        self.simulate_failure = true;
        self
    }

    /// Generate a mock proof (just a hash of the receipt)
    fn generate_mock_proof(&self, receipt: &ExecutionReceipt) -> Vec<u8> {
        use serde_json;

        // Serialize receipt and create a simple "proof"
        let receipt_json = serde_json::to_string(receipt).unwrap_or_default();
        let mut proof = blake3::hash(receipt_json.as_bytes()).as_bytes().to_vec();

        // Add some metadata to make it look like a real proof
        proof.extend_from_slice(b"MOCK_PROOF_v1");
        proof.extend_from_slice(&self.generation_delay_ms.to_le_bytes());

        proof
    }

    /// Verify a mock proof
    fn verify_mock_proof(&self, receipt: &ExecutionReceipt, proof: &[u8]) -> bool {
        // Check proof format
        if proof.len() < 13 + 8 {
            // 32 bytes hash + "MOCK_PROOF_v1" + 8 bytes delay
            return false;
        }

        // Verify it ends with our marker
        let marker = &proof[32..45];
        if marker != b"MOCK_PROOF_v1" {
            return false;
        }

        // Regenerate expected proof and compare
        let expected_proof = self.generate_mock_proof(receipt);
        proof[..32] == expected_proof[..32]
    }
}

impl Default for MockProofBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProofBackend for MockProofBackend {
    async fn initialize(&mut self) -> Result<(), ProofError> {
        info!("Initializing mock proof backend");

        // Simulate initialization delay
        sleep(Duration::from_millis(50)).await;

        self.initialized = true;
        info!("Mock proof backend initialized");
        Ok(())
    }

    async fn generate_proof(&self, request: ProofRequest) -> Result<ProofResponse, ProofError> {
        if !self.initialized {
            return Err(ProofError::NotInitialized {
                reason: "Backend not initialized".to_string(),
            });
        }

        if self.simulate_failure {
            let mut stats = self.stats.write().await;
            stats.record_failure();
            return Err(ProofError::GenerationFailed {
                reason: "Simulated failure for testing".to_string(),
            });
        }

        debug!("Generating mock proof for receipt: {}", request.receipt.capsule_id);

        // Simulate proof generation time
        let start = std::time::Instant::now();
        sleep(Duration::from_millis(self.generation_delay_ms)).await;

        // Generate mock proof
        let proof = self.generate_mock_proof(&request.receipt);

        let generation_time_ms = start.elapsed().as_millis() as u64;

        // Update stats
        let mut stats = self.stats.write().await;
        stats.record_generation(generation_time_ms);

        Ok(ProofResponse {
            receipt: request.receipt,
            proof,
            generation_time_ms,
            proof_system: "mock".to_string(),
        })
    }

    async fn verify_proof(
        &self,
        receipt: &ExecutionReceipt,
        proof: &[u8],
    ) -> Result<bool, ProofError> {
        if !self.initialized {
            return Err(ProofError::NotInitialized {
                reason: "Backend not initialized".to_string(),
            });
        }

        debug!("Verifying mock proof for receipt: {}", receipt.capsule_id);

        // Simulate verification time
        let start = std::time::Instant::now();
        sleep(Duration::from_millis(self.verification_delay_ms)).await;

        // Verify mock proof
        let is_valid = self.verify_mock_proof(receipt, proof);

        let verification_time_ms = start.elapsed().as_millis() as u64;

        // Update stats
        let mut stats = self.stats.write().await;
        stats.record_verification(verification_time_ms);

        Ok(is_valid)
    }

    fn proof_system(&self) -> &str {
        "mock"
    }

    fn is_initialized(&self) -> bool {
        self.initialized
    }

    fn stats(&self) -> BackendStats {
        // This is a sync method but stats is behind RwLock
        // For now, return default if we can't get the lock
        // In production, this should be redesigned
        futures::executor::block_on(async { self.stats.read().await.clone() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tenzik_runtime::ExecutionReceipt;

    fn create_test_receipt() -> ExecutionReceipt {
        use tenzik_runtime::ExecMetrics;

        ExecutionReceipt {
            capsule_id: "test_capsule_hash".to_string(),
            input_commit: "input_hash".to_string(),
            output_commit: "output_hash".to_string(),
            exec_metrics: ExecMetrics {
                fuel_used: 1000,
                memory_mb: 1.0,
                duration_ms: 100,
                host_function_calls: 0,
            },
            node_id: "test_node".to_string(),
            nonce: 1,
            signature: "test_sig".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            version: "1.0".to_string(),
            zk_proof: None,
        }
    }

    #[tokio::test]
    async fn test_mock_backend_initialization() {
        let mut backend = MockProofBackend::new();
        assert!(!backend.is_initialized());

        backend.initialize().await.unwrap();
        assert!(backend.is_initialized());
    }

    #[tokio::test]
    async fn test_mock_proof_generation() {
        let mut backend = MockProofBackend::new();
        backend.initialize().await.unwrap();

        let receipt = create_test_receipt();
        let request = ProofRequest {
            receipt: receipt.clone(),
            timeout_ms: None,
            priority: 1,
        };

        let response = backend.generate_proof(request).await.unwrap();
        assert_eq!(response.receipt.capsule_id, receipt.capsule_id);
        assert!(!response.proof.is_empty());
        assert_eq!(response.proof_system, "mock");
    }

    #[tokio::test]
    async fn test_mock_proof_verification() {
        let mut backend = MockProofBackend::new();
        backend.initialize().await.unwrap();

        let receipt = create_test_receipt();
        let request = ProofRequest {
            receipt: receipt.clone(),
            timeout_ms: None,
            priority: 1,
        };

        // Generate proof
        let response = backend.generate_proof(request).await.unwrap();

        // Verify the proof
        let is_valid = backend
            .verify_proof(&receipt, &response.proof)
            .await
            .unwrap();
        assert!(is_valid);

        // Verify with wrong proof
        let wrong_proof = vec![0u8; 64];
        let is_valid = backend.verify_proof(&receipt, &wrong_proof).await.unwrap();
        assert!(!is_valid);
    }

    #[tokio::test]
    async fn test_mock_backend_stats() {
        let mut backend = MockProofBackend::new();
        backend.initialize().await.unwrap();

        let receipt = create_test_receipt();
        let request = ProofRequest {
            receipt: receipt.clone(),
            timeout_ms: None,
            priority: 1,
        };

        // Generate proof
        let response = backend.generate_proof(request).await.unwrap();

        // Verify proof
        backend
            .verify_proof(&receipt, &response.proof)
            .await
            .unwrap();

        let stats = backend.stats();
        assert_eq!(stats.total_proofs_generated, 1);
        assert_eq!(stats.total_proofs_verified, 1);
        assert!(stats.avg_generation_time_ms > 0.0);
    }

    #[tokio::test]
    async fn test_failure_simulation() {
        let mut backend = MockProofBackend::new().with_failure_simulation();
        backend.initialize().await.unwrap();

        let receipt = create_test_receipt();
        let request = ProofRequest {
            receipt,
            timeout_ms: None,
            priority: 1,
        };

        let result = backend.generate_proof(request).await;
        assert!(result.is_err());

        let stats = backend.stats();
        assert_eq!(stats.total_proofs_failed, 1);
    }
}
