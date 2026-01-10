use anyhow::{Context, Result};
use serde_json;
use std::fs;
use tenzik_runtime::{ExecutionReceipt, ReceiptVerifier};

pub fn verify_receipt_file(receipt_path: &str) -> Result<()> {
    println!("🔍 Verifying receipt: {}", receipt_path);

    // Read receipt file
    let receipt_json = fs::read_to_string(receipt_path)
        .with_context(|| format!("Failed to read receipt file: {}", receipt_path))?;

    // Parse receipt
    let receipt: ExecutionReceipt = serde_json::from_str(&receipt_json)
        .context("Failed to parse receipt JSON")?;

    // Verify signature
    let verifier = ReceiptVerifier::default();

    print!("  ✓ Checking signature... ");
    let signature_valid = receipt.verify_node_signature()
        .context("Signature verification failed")?;

    if signature_valid {
        println!("✅ Valid");
    } else {
        println!("❌ Invalid");
        anyhow::bail!("Receipt signature is invalid");
    }

    // Verify age
    print!("  ✓ Checking timestamp... ");
    match verifier.verify_receipt(&receipt) {
        Ok(_) => println!("✅ Valid (within acceptable age)"),
        Err(e) => {
            println!("⚠️  {}", e);
            println!("     Receipt may be too old, but signature is valid");
        }
    }

    // Check for ZK proof
    print!("  ✓ Checking ZK proof... ");
    if receipt.has_proof() {
        println!("✅ Present");
        let proof = receipt.zk_proof.as_ref().unwrap();
        println!("     Backend: {}", proof.backend_type);
        println!("     Proof data: {} bytes", proof.proof_data.len());
    } else {
        println!("⚠️  None");
        println!("     No ZK proof attached to this receipt");
    }

    println!("\n✅ Receipt verification completed successfully!");
    println!("\n📊 Receipt Details:");
    println!("   Node ID: {}", receipt.node_id);
    println!("   Capsule ID: {}...", &receipt.capsule_id[..16]);
    println!("   Timestamp: {}", receipt.timestamp);
    println!("   Nonce: {}", receipt.nonce);

    Ok(())
}

pub fn inspect_receipt_file(receipt_path: &str, verbose: bool) -> Result<()> {
    println!("🔍 Inspecting receipt: {}", receipt_path);

    // Read and parse receipt
    let receipt_json = fs::read_to_string(receipt_path)
        .with_context(|| format!("Failed to read receipt file: {}", receipt_path))?;

    let receipt: ExecutionReceipt = serde_json::from_str(&receipt_json)
        .context("Failed to parse receipt JSON")?;

    // Display receipt information
    println!("\n📦 Capsule Information:");
    println!("   Capsule ID: {}", receipt.capsule_id);
    println!("   Version: {}", receipt.version);

    println!("\n📥 Input/Output:");
    println!("   Input Commit: {}", receipt.input_commit);
    println!("   Output Commit: {}", receipt.output_commit);

    println!("\n⚙️  Execution Metrics:");
    println!("   Fuel Used: {} units", receipt.exec_metrics.fuel_used);
    println!("   Memory: {:.2} MB", receipt.exec_metrics.memory_mb);
    println!("   Duration: {} ms", receipt.exec_metrics.duration_ms);
    println!("   Host Function Calls: {}", receipt.exec_metrics.host_function_calls);

    println!("\n🔐 Cryptographic Data:");
    println!("   Node ID: {}", receipt.node_id);
    println!("   Nonce: {}", receipt.nonce);
    println!("   Signature: {}...", &receipt.signature[..32]);
    println!("   Timestamp: {}", receipt.timestamp);

    if receipt.has_proof() {
        println!("\n🔬 Zero-Knowledge Proof:");
        println!("   Status: ✅ Present");
        let proof = receipt.zk_proof.as_ref().unwrap();
        println!("   Backend: {}", proof.backend_type);
        println!("   Proof Size: {} bytes", proof.proof_data.len());
        println!("   Public Inputs: {} bytes", proof.public_inputs.len());
        println!("   Generation Time: {}ms", proof.metadata.generation_time_ms);
        if verbose {
            let proof_hex = hex::encode(&proof.proof_data);
            println!("   Proof Data (hex): {}...", &proof_hex[..64.min(proof_hex.len())]);
        }
    } else {
        println!("\n🔬 Zero-Knowledge Proof:");
        println!("   Status: ⚠️  None");
    }

    if verbose {
        println!("\n📄 Full JSON:");
        println!("{}", serde_json::to_string_pretty(&receipt)?);
    }

    Ok(())
}

pub fn export_receipt_summary(receipt_path: &str, output_path: &str) -> Result<()> {
    println!("📤 Exporting receipt summary...");

    // Read and parse receipt
    let receipt_json = fs::read_to_string(receipt_path)
        .with_context(|| format!("Failed to read receipt file: {}", receipt_path))?;

    let receipt: ExecutionReceipt = serde_json::from_str(&receipt_json)
        .context("Failed to parse receipt JSON")?;

    // Create summary
    let summary = serde_json::json!({
        "receipt_summary": {
            "version": receipt.version,
            "timestamp": receipt.timestamp,
            "node_id": receipt.node_id,
            "capsule_id": receipt.capsule_id,
            "input_commit": receipt.input_commit,
            "output_commit": receipt.output_commit,
            "metrics": {
                "fuel_used": receipt.exec_metrics.fuel_used,
                "memory_mb": receipt.exec_metrics.memory_mb,
                "duration_ms": receipt.exec_metrics.duration_ms,
                "host_calls": receipt.exec_metrics.host_function_calls
            },
            "has_zk_proof": receipt.has_proof(),
            "signature_preview": &receipt.signature[..32]
        }
    });

    // Write summary
    fs::write(output_path, serde_json::to_string_pretty(&summary)?)
        .with_context(|| format!("Failed to write summary to: {}", output_path))?;

    println!("✅ Summary exported to: {}", output_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use tenzik_runtime::ExecMetrics;
    use ed25519_dalek::SigningKey;
    use rand::RngCore;

    fn create_test_receipt_file() -> NamedTempFile {
        let mut csprng = rand::rngs::OsRng;
        let mut secret_bytes = [0u8; 32];
        csprng.fill_bytes(&mut secret_bytes);
        let signing_key = SigningKey::from_bytes(&secret_bytes);

        let receipt = ExecutionReceipt::new(
            b"test capsule",
            b"test input",
            b"test output",
            ExecMetrics {
                fuel_used: 1000,
                memory_mb: 1.0,
                duration_ms: 50,
                host_function_calls: 0,
            },
            &signing_key,
            12345,
        )
        .unwrap();

        let temp_file = NamedTempFile::new().unwrap();
        fs::write(temp_file.path(), serde_json::to_string(&receipt).unwrap()).unwrap();
        temp_file
    }

    #[test]
    fn test_verify_receipt_file() {
        let temp_file = create_test_receipt_file();
        let result = verify_receipt_file(temp_file.path().to_str().unwrap());
        assert!(result.is_ok());
    }

    #[test]
    fn test_inspect_receipt_file() {
        let temp_file = create_test_receipt_file();
        let result = inspect_receipt_file(temp_file.path().to_str().unwrap(), false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_export_receipt_summary() {
        let temp_file = create_test_receipt_file();
        let output_file = NamedTempFile::new().unwrap();

        let result = export_receipt_summary(
            temp_file.path().to_str().unwrap(),
            output_file.path().to_str().unwrap(),
        );

        assert!(result.is_ok());
        assert!(output_file.path().exists());
    }
}
