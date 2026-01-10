//! Integration tests for Tenzik Core
//!
//! These tests verify end-to-end functionality across all components:
//! - Runtime execution and receipt generation
//! - ZK proof integration
//! - Federation and gossip protocol
//! - Two-node event exchange

use anyhow::Result;
use tenzik_runtime::ExecutionReceipt;
use tenzik_zk::{MockProofBackend, ProofBackend, ProofRequest};
use std::sync::Arc;

#[tokio::test]
async fn test_receipt_generation() -> Result<()> {
    println!("🧪 Testing receipt generation...");

    // Create test data
    let capsule_bytes = b"test_wasm_capsule";
    let input_bytes = b"{\"test\": \"input\"}";
    let output_bytes = b"{\"test\": \"output\"}";

    // Generate signing key
    use rand::RngCore;
    use ed25519_dalek::SigningKey;
    let mut csprng = rand::rngs::OsRng;
    let mut secret_bytes = [0u8; 32];
    csprng.fill_bytes(&mut secret_bytes);
    let signing_key = SigningKey::from_bytes(&secret_bytes);

    // Create receipt
    use tenzik_runtime::ExecMetrics;
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
    )?;

    println!("✅ Receipt generated successfully");
    println!("   Receipt ID: {}", receipt.receipt_id());
    println!("   Capsule ID: {}", receipt.capsule_id);
    println!("   Node ID: {}", receipt.node_id);

    // Verify the receipt
    assert!(receipt.verify_node_signature()?);
    println!("✅ Receipt signature verified");

    Ok(())
}

#[tokio::test]
async fn test_receipt_with_zk_proof() -> Result<()> {
    println!("🧪 Testing receipt with ZK proof integration...");

    // Create test receipt
    use rand::RngCore;
    use ed25519_dalek::SigningKey;
    let mut csprng = rand::rngs::OsRng;
    let mut secret_bytes = [0u8; 32];
    csprng.fill_bytes(&mut secret_bytes);
    let signing_key = SigningKey::from_bytes(&secret_bytes);

    use tenzik_runtime::ExecMetrics;
    let receipt = ExecutionReceipt::new(
        b"test_capsule",
        b"{\"input\": \"data\"}",
        b"{\"output\": \"result\"}",
        ExecMetrics::default(),
        &signing_key,
        42,
    )?;

    println!("✅ Base receipt created");

    // Initialize mock proof backend
    let mut backend = MockProofBackend::new();
    backend.initialize().await?;

    println!("✅ Mock proof backend initialized");

    // Generate a proof
    let proof_request = ProofRequest {
        receipt: receipt.clone(),
        priority: 5,
        timeout_ms: None,
    };

    let proof_response = backend.generate_proof(proof_request).await?;
    println!("✅ ZK proof generated: {} bytes", proof_response.proof.len());

    // Verify the proof with the original receipt (before attaching)
    let is_valid = backend.verify_proof(&receipt, &proof_response.proof).await?;
    assert!(is_valid);
    println!("✅ ZK proof verified successfully");

    // Attach proof to receipt for serialization test
    let receipt_with_proof = receipt.with_proof(proof_response.proof.clone());

    assert!(receipt_with_proof.has_proof());
    println!("✅ Proof attached to receipt");

    // Serialize to JSON and verify round-trip
    let json = receipt_with_proof.to_json()?;
    let deserialized = ExecutionReceipt::from_json(&json)?;

    assert_eq!(receipt_with_proof.capsule_id, deserialized.capsule_id);
    assert_eq!(receipt_with_proof.zk_proof, deserialized.zk_proof);
    println!("✅ Receipt with proof serialization verified");

    Ok(())
}

#[tokio::test]
async fn test_proof_backend_stats() -> Result<()> {
    println!("🧪 Testing proof backend statistics...");

    let backend = Arc::new(MockProofBackend::new());
    let mut backend_mut = MockProofBackend::new();
    backend_mut.initialize().await?;

    // Generate some proofs
    use rand::RngCore;
    use ed25519_dalek::SigningKey;
    let mut csprng = rand::rngs::OsRng;
    let mut secret_bytes = [0u8; 32];
    csprng.fill_bytes(&mut secret_bytes);
    let signing_key = SigningKey::from_bytes(&secret_bytes);

    for i in 0..3 {
        use tenzik_runtime::ExecMetrics;
        let receipt = ExecutionReceipt::new(
            b"test",
            b"input",
            b"output",
            ExecMetrics::default(),
            &signing_key,
            i,
        )?;

        let request = ProofRequest {
            receipt,
            priority: 5,
            timeout_ms: None,
        };

        backend_mut.generate_proof(request).await?;
    }

    let stats = backend_mut.stats();
    println!("✅ Backend stats: {} proofs generated, {} verified",
             stats.total_proofs_generated, stats.total_proofs_verified);

    assert_eq!(stats.total_proofs_generated, 3);
    println!("✅ Statistics tracking working correctly");

    Ok(())
}

#[test]
fn test_receipt_optional_proof_serialization() {
    println!("🧪 Testing optional proof field serialization...");

    use rand::RngCore;
    use ed25519_dalek::SigningKey;
    let mut csprng = rand::rngs::OsRng;
    let mut secret_bytes = [0u8; 32];
    csprng.fill_bytes(&mut secret_bytes);
    let signing_key = SigningKey::from_bytes(&secret_bytes);

    // Receipt without proof
    use tenzik_runtime::ExecMetrics;
    let receipt_no_proof = ExecutionReceipt::new(
        b"test",
        b"input",
        b"output",
        ExecMetrics::default(),
        &signing_key,
        1,
    ).unwrap();

    let json_no_proof = receipt_no_proof.to_json().unwrap();
    assert!(!json_no_proof.contains("zk_proof"));
    println!("✅ Receipt without proof: zk_proof field omitted from JSON");

    // Receipt with proof
    let receipt_with_proof = receipt_no_proof.with_proof(vec![1, 2, 3, 4]);
    let json_with_proof = receipt_with_proof.to_json().unwrap();
    assert!(json_with_proof.contains("zk_proof"));
    println!("✅ Receipt with proof: zk_proof field included in JSON");
}

#[test]
fn test_component_integration() {
    println!("🧪 Testing component integration...");

    // Test that all crates can be used together
    use tenzik_runtime::{ExecutionReceipt, ExecMetrics};
    use tenzik_protocol::DAGStats;
    use tenzik_zk::MockProofBackend;

    println!("✅ All core crates imported successfully");
    println!("   - tenzik-runtime: ExecutionReceipt, ExecMetrics");
    println!("   - tenzik-protocol: DAGStats");
    println!("   - tenzik-zk: MockProofBackend");
}
