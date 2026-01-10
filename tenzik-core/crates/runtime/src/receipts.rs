//! Execution Receipts Module
//!
//! This module provides cryptographic receipts for WASM capsule executions.
//! Receipts enable verification that an execution occurred with specific inputs/outputs
//! without needing to re-execute the capsule.

use blake3;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use crate::proofs::{ZkProof, ProofBackend, ProofError};

/// Execution metrics collected during capsule execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecMetrics {
    /// Fuel units consumed during execution
    pub fuel_used: u64,
    /// Peak memory usage in MB
    pub memory_mb: f64,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Number of host function calls made
    pub host_function_calls: u32,
}

impl Default for ExecMetrics {
    fn default() -> Self {
        Self {
            fuel_used: 0,
            memory_mb: 0.0,
            duration_ms: 0,
            host_function_calls: 0,
        }
    }
}

/// Receipt errors
#[derive(Error, Debug)]
pub enum ReceiptError {
    #[error("Invalid signature")]
    InvalidSignature,
    
    #[error("Signature verification failed")]
    SignatureVerificationFailed,
    
    #[error("Invalid receipt format: {reason}")]
    InvalidFormat { reason: String },
    
    #[error("Cryptographic error: {source}")]
    CryptographicError { source: Box<dyn std::error::Error + Send + Sync> },
    
    #[error("Serialization error: {source}")]
    SerializationError { source: serde_json::Error },
}

/// Cryptographic execution receipt
///
/// This structure provides cryptographic proof that a specific WASM capsule
/// was executed with specific inputs and produced specific outputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    /// Blake3 hash of the WASM capsule bytes
    pub capsule_id: String,
    /// Blake3 hash of the input JSON
    pub input_commit: String,
    /// Blake3 hash of the output JSON
    pub output_commit: String,
    /// Execution metrics
    pub exec_metrics: ExecMetrics,
    /// Ed25519 public key of the executing node
    pub node_id: String,
    /// Nonce for replay protection
    pub nonce: u64,
    /// Ed25519 signature of the receipt content
    pub signature: String,
    /// ISO 8601 timestamp of execution
    pub timestamp: String,
    /// Version of the receipt format
    pub version: String,
    /// Optional zero-knowledge proof
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zk_proof: Option<ZkProof>,
}

impl ExecutionReceipt {
    /// Create a new execution receipt
    pub fn new(
        capsule_bytes: &[u8],
        input_bytes: &[u8],
        output_bytes: &[u8],
        metrics: ExecMetrics,
        signing_key: &SigningKey,
        nonce: u64,
    ) -> Result<Self, ReceiptError> {
        // Generate content commitments
        let capsule_id = blake3::hash(capsule_bytes).to_hex().to_string();
        let input_commit = blake3::hash(input_bytes).to_hex().to_string();
        let output_commit = blake3::hash(output_bytes).to_hex().to_string();
        
        // Get node ID from signing key
        let node_id = hex::encode(signing_key.verifying_key().as_bytes());
        
        // Generate timestamp
        let timestamp = Self::current_timestamp_iso8601();
        
        // Create the payload to sign
        let payload = Self::create_signature_payload(
            &capsule_id,
            &input_commit,
            &output_commit,
            &metrics,
            &node_id,
            nonce,
            &timestamp,
        );
        
        // Sign the payload
        let signature_bytes = signing_key.sign(payload.as_bytes());
        let signature = hex::encode(signature_bytes.to_bytes());
        
        Ok(ExecutionReceipt {
            capsule_id,
            input_commit,
            output_commit,
            exec_metrics: metrics,
            node_id,
            nonce,
            signature,
            timestamp,
            version: "1.0.0".to_string(),
            zk_proof: None,
        })
    }

    /// Attach a zero-knowledge proof to this receipt
    pub fn with_proof(mut self, proof: ZkProof) -> Self {
        self.zk_proof = Some(proof);
        self
    }

    /// Check if this receipt has a ZK proof attached
    pub fn has_proof(&self) -> bool {
        self.zk_proof.is_some()
    }
    
    /// Verify the receipt signature
    pub fn verify(&self, verifying_key: &VerifyingKey) -> Result<bool, ReceiptError> {
        // Recreate the signature payload
        let payload = Self::create_signature_payload(
            &self.capsule_id,
            &self.input_commit,
            &self.output_commit,
            &self.exec_metrics,
            &self.node_id,
            self.nonce,
            &self.timestamp,
        );
        
        // Decode the signature
        let signature_bytes = hex::decode(&self.signature)
            .map_err(|e| ReceiptError::InvalidFormat {
                reason: format!("Invalid signature hex: {}", e)
            })?;

        // Convert Vec<u8> to [u8; 64]
        let signature_array: [u8; 64] = signature_bytes
            .try_into()
            .map_err(|_| ReceiptError::InvalidFormat {
                reason: "Signature must be exactly 64 bytes".to_string()
            })?;

        let signature = Signature::from_bytes(&signature_array);
        
        // Verify the signature
        match verifying_key.verify(payload.as_bytes(), &signature) {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }
    
    /// Verify that the receipt was signed by the claimed node
    pub fn verify_node_signature(&self) -> Result<bool, ReceiptError> {
        // Decode the node public key
        let public_key_bytes = hex::decode(&self.node_id)
            .map_err(|e| ReceiptError::InvalidFormat { 
                reason: format!("Invalid node_id hex: {}", e) 
            })?;
        
        let verifying_key = VerifyingKey::from_bytes(&public_key_bytes
            .try_into()
            .map_err(|_| ReceiptError::InvalidFormat { 
                reason: "Invalid public key length".to_string() 
            })?)
            .map_err(|e| ReceiptError::CryptographicError { 
                source: Box::new(e) 
            })?;
        
        self.verify(&verifying_key)
    }
    
    /// Get the receipt ID (hash of the receipt content)
    pub fn receipt_id(&self) -> String {
        let content = format!(
            "{}:{}:{}:{}:{}",
            self.capsule_id,
            self.input_commit,
            self.output_commit,
            self.node_id,
            self.nonce
        );
        blake3::hash(content.as_bytes()).to_hex().to_string()
    }
    
    /// Serialize to JSON
    pub fn to_json(&self) -> Result<String, ReceiptError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| ReceiptError::SerializationError { source: e })
    }
    
    /// Deserialize from JSON
    pub fn from_json(json: &str) -> Result<Self, ReceiptError> {
        serde_json::from_str(json)
            .map_err(|e| ReceiptError::SerializationError { source: e })
    }
    
    /// Check if the receipt is recent (within last hour by default)
    pub fn is_recent(&self, max_age_seconds: u64) -> bool {
        if let Ok(receipt_time) = chrono::DateTime::parse_from_rfc3339(&self.timestamp) {
            let now = chrono::Utc::now();
            let age = now.signed_duration_since(receipt_time.with_timezone(&chrono::Utc));
            age.num_seconds() < max_age_seconds as i64
        } else {
            false
        }
    }
    
    /// Create the payload that gets signed
    fn create_signature_payload(
        capsule_id: &str,
        input_commit: &str,
        output_commit: &str,
        metrics: &ExecMetrics,
        node_id: &str,
        nonce: u64,
        timestamp: &str,
    ) -> String {
        // Create a deterministic representation for signing
        format!(
            "TENZIK_RECEIPT_V1\n\
             capsule_id:{}\n\
             input_commit:{}\n\
             output_commit:{}\n\
             fuel_used:{}\n\
             memory_mb:{:.3}\n\
             duration_ms:{}\n\
             host_calls:{}\n\
             node_id:{}\n\
             nonce:{}\n\
             timestamp:{}",
            capsule_id,
            input_commit,
            output_commit,
            metrics.fuel_used,
            metrics.memory_mb,
            metrics.duration_ms,
            metrics.host_function_calls,
            node_id,
            nonce,
            timestamp
        )
    }
    
    /// Get current timestamp as ISO 8601 string
    fn current_timestamp_iso8601() -> String {
        chrono::Utc::now().to_rfc3339()
    }
}

/// Receipt verification utilities
///
/// This verifies signatures and age. ZK proof verification should be done
/// at a higher level using the tenzik-zk crate's ProofBackend.
pub struct ReceiptVerifier {
    /// Maximum age for receipts to be considered valid (in seconds)
    pub max_receipt_age_seconds: u64,
}

impl Default for ReceiptVerifier {
    fn default() -> Self {
        Self {
            max_receipt_age_seconds: 3600, // 1 hour
        }
    }
}

impl ReceiptVerifier {
    /// Create a new verifier with custom settings
    pub fn new(max_receipt_age_seconds: u64) -> Self {
        Self {
            max_receipt_age_seconds,
        }
    }

    /// Verify a receipt's signature and age
    ///
    /// Note: This does NOT verify ZK proofs. For ZK proof verification,
    /// use the tenzik-zk crate's ProofBackend::verify_proof method.
    pub fn verify_receipt(&self, receipt: &ExecutionReceipt) -> Result<bool, ReceiptError> {
        // Check signature
        if !receipt.verify_node_signature()? {
            return Ok(false);
        }

        // Check age
        if !receipt.is_recent(self.max_receipt_age_seconds) {
            return Ok(false);
        }

        Ok(true)
    }

    /// Verify receipt with ZK proof if present
    ///
    /// This performs standard verification (signature + age) and additionally
    /// verifies the ZK proof if one is attached to the receipt.
    pub async fn verify_with_zk(
        &self,
        receipt: &ExecutionReceipt,
        backend: &dyn ProofBackend,
    ) -> Result<bool, ReceiptError> {
        // First do standard verification
        if !self.verify_receipt(receipt)? {
            return Ok(false);
        }

        // If there's a ZK proof, verify it
        if let Some(ref proof) = receipt.zk_proof {
            match backend.verify_proof(proof, receipt).await {
                Ok(valid) => Ok(valid),
                Err(e) => Err(ReceiptError::InvalidFormat {
                    reason: format!("ZK proof verification failed: {}", e),
                }),
            }
        } else {
            // No ZK proof, just return result of standard verification
            Ok(true)
        }
    }

    /// Verify multiple receipts
    pub fn verify_receipts(&self, receipts: &[ExecutionReceipt]) -> Vec<Result<bool, ReceiptError>> {
        receipts.iter().map(|r| self.verify_receipt(r)).collect()
    }

    /// Verify multiple receipts with ZK proofs
    pub async fn verify_receipts_with_zk(
        &self,
        receipts: &[ExecutionReceipt],
        backend: &dyn ProofBackend,
    ) -> Vec<Result<bool, ReceiptError>> {
        let mut results = Vec::new();
        for receipt in receipts {
            results.push(self.verify_with_zk(receipt, backend).await);
        }
        results
    }
}

/// Generate a new signing key for testing
#[cfg(test)]
pub fn generate_test_signing_key() -> SigningKey {
    use rand::RngCore;
    use rand::rngs::OsRng;
    let mut secret_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut secret_bytes);
    SigningKey::from_bytes(&secret_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_receipt_creation_and_verification() {
        let signing_key = generate_test_signing_key();
        let verifying_key = signing_key.verifying_key();
        
        let capsule_bytes = b"test capsule";
        let input_bytes = b"{\"test\": \"input\"}";
        let output_bytes = b"{\"test\": \"output\"}";
        let metrics = ExecMetrics {
            fuel_used: 1000,
            memory_mb: 2.5,
            duration_ms: 50,
            host_function_calls: 3,
        };
        
        let receipt = ExecutionReceipt::new(
            capsule_bytes,
            input_bytes,
            output_bytes,
            metrics,
            &signing_key,
            12345,
        ).unwrap();
        
        assert!(receipt.verify(&verifying_key).unwrap());
        assert!(receipt.verify_node_signature().unwrap());
    }
    
    #[test]
    fn test_receipt_json_serialization() {
        let signing_key = generate_test_signing_key();
        
        let receipt = ExecutionReceipt::new(
            b"test",
            b"input",
            b"output",
            ExecMetrics::default(),
            &signing_key,
            42,
        ).unwrap();
        
        let json = receipt.to_json().unwrap();
        let deserialized = ExecutionReceipt::from_json(&json).unwrap();
        
        assert_eq!(receipt.capsule_id, deserialized.capsule_id);
        assert_eq!(receipt.signature, deserialized.signature);
        assert_eq!(receipt.nonce, deserialized.nonce);
    }
    
    #[test]
    fn test_receipt_id_generation() {
        let signing_key = generate_test_signing_key();
        
        let receipt1 = ExecutionReceipt::new(
            b"test",
            b"input",
            b"output",
            ExecMetrics::default(),
            &signing_key,
            42,
        ).unwrap();
        
        let receipt2 = ExecutionReceipt::new(
            b"test",
            b"input",
            b"output",
            ExecMetrics::default(),
            &signing_key,
            42,
        ).unwrap();
        
        // Same inputs should produce same receipt ID
        assert_eq!(receipt1.receipt_id(), receipt2.receipt_id());
    }
    
    #[test]
    fn test_receipt_verification_failure() {
        let signing_key1 = generate_test_signing_key();
        let signing_key2 = generate_test_signing_key();
        let verifying_key2 = signing_key2.verifying_key();
        
        let receipt = ExecutionReceipt::new(
            b"test",
            b"input",
            b"output",
            ExecMetrics::default(),
            &signing_key1,
            42,
        ).unwrap();
        
        // Verification with wrong key should fail
        assert!(!receipt.verify(&verifying_key2).unwrap());
    }
    
    #[test]
    fn test_receipt_age_checking() {
        let signing_key = generate_test_signing_key();
        
        let receipt = ExecutionReceipt::new(
            b"test",
            b"input",
            b"output",
            ExecMetrics::default(),
            &signing_key,
            42,
        ).unwrap();
        
        // Fresh receipt should be recent
        assert!(receipt.is_recent(3600)); // 1 hour
        
        // Even very strict age limit should accept fresh receipt
        assert!(receipt.is_recent(60)); // 1 minute
    }
    
    #[test]
    fn test_receipt_verifier() {
        let verifier = ReceiptVerifier::new(3600);
        let signing_key = generate_test_signing_key();
        
        let receipt = ExecutionReceipt::new(
            b"test",
            b"input",
            b"output",
            ExecMetrics::default(),
            &signing_key,
            42,
        ).unwrap();
        
        assert!(verifier.verify_receipt(&receipt).unwrap());
    }
    
    #[test]
    fn test_exec_metrics() {
        let metrics = ExecMetrics {
            fuel_used: 5000,
            memory_mb: 16.75,
            duration_ms: 125,
            host_function_calls: 7,
        };

        // Test serialization
        let json = serde_json::to_string(&metrics).unwrap();
        let deserialized: ExecMetrics = serde_json::from_str(&json).unwrap();

        assert_eq!(metrics, deserialized);
    }

    #[test]
    fn test_receipt_zk_proof_attachment() {
        use crate::proofs::{ZkProof, ProofBackendType, ProofMetadata};

        let signing_key = generate_test_signing_key();

        let mut receipt = ExecutionReceipt::new(
            b"test",
            b"input",
            b"output",
            ExecMetrics::default(),
            &signing_key,
            42,
        ).unwrap();

        // Initially should not have a proof
        assert!(!receipt.has_zk_proof());

        // Attach a mock proof
        let proof = ZkProof {
            backend_type: ProofBackendType::Mock,
            proof_data: b"mock proof".to_vec(),
            public_inputs: b"inputs".to_vec(),
            metadata: ProofMetadata {
                backend_version: "test-1.0".to_string(),
                generation_time_ms: 100,
                proof_size_bytes: 10,
                timestamp: chrono::Utc::now().to_rfc3339(),
            },
        };

        receipt.attach_proof(proof);

        // Should now have a proof
        assert!(receipt.has_zk_proof());
    }

    #[tokio::test]
    async fn test_receipt_verification_with_zk() {
        use crate::proofs::MockProofBackend;
        use std::sync::Arc;

        let backend = MockProofBackend::new();
        let verifier = ReceiptVerifier::new(3600);
        let signing_key = generate_test_signing_key();

        // Create receipt
        let mut receipt = ExecutionReceipt::new(
            b"test capsule",
            b"input",
            b"output",
            ExecMetrics::default(),
            &signing_key,
            42,
        ).unwrap();

        // Verify without ZK proof
        let result = verifier.verify_with_zk(&receipt, &backend).await.unwrap();
        assert!(result);

        // Generate and attach ZK proof
        let proof = backend.generate_proof(&receipt).await.unwrap();
        receipt.attach_proof(proof);

        // Verify with ZK proof
        let result = verifier.verify_with_zk(&receipt, &backend).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_receipt_verification_with_invalid_zk() {
        use crate::proofs::{MockProofBackend, ZkProof, ProofBackendType, ProofMetadata};
        use std::sync::Arc;

        let backend = MockProofBackend::new();
        let verifier = ReceiptVerifier::new(3600);
        let signing_key1 = generate_test_signing_key();
        let signing_key2 = generate_test_signing_key();

        // Create receipt 1
        let mut receipt1 = ExecutionReceipt::new(
            b"capsule1",
            b"input1",
            b"output1",
            ExecMetrics::default(),
            &signing_key1,
            42,
        ).unwrap();

        // Create receipt 2
        let receipt2 = ExecutionReceipt::new(
            b"capsule2",
            b"input2",
            b"output2",
            ExecMetrics::default(),
            &signing_key2,
            43,
        ).unwrap();

        // Generate proof for receipt2
        let proof2 = backend.generate_proof(&receipt2).await.unwrap();

        // Attach proof2 to receipt1 (mismatch!)
        receipt1.attach_proof(proof2);

        // Verification should fail
        let result = verifier.verify_with_zk(&receipt1, &backend).await.unwrap();
        assert!(!result);
    }
}
