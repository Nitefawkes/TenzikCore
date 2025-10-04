//! Test command implementation
//!
//! This module implements the `tenzik test` command for locally executing
//! WASM capsules and displaying execution results with receipts.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use tenzik_runtime::{
    Capability, ResourceLimits, WasmRuntime, 
};

/// Arguments for the test command
pub struct TestArgs {
    /// Path to the WASM capsule file
    pub capsule: String,
    /// Input JSON string
    pub input: String,
    /// Whether to show detailed execution metrics
    pub metrics: bool,
    /// Whether to show the full receipt
    pub show_receipt: bool,
    /// Custom resource limits (JSON format)
    pub limits: Option<String>,
}

/// Execute the test command
pub async fn execute_test_command(args: TestArgs) -> Result<()> {
    println!("🧪 Testing Tenzik capsule...");
    println!("📁 Capsule: {}", args.capsule);
    println!("📝 Input: {}", args.input);
    println!();

    // Load WASM capsule from file
    let capsule_path = Path::new(&args.capsule);
    if !capsule_path.exists() {
        anyhow::bail!("Capsule file not found: {}", args.capsule);
    }

    let capsule_bytes = fs::read(capsule_path)
        .with_context(|| format!("Failed to read capsule file: {}", args.capsule))?;

    println!("📦 Loaded capsule: {} bytes ({:.2} KB)", 
             capsule_bytes.len(), 
             capsule_bytes.len() as f64 / 1024.0);

    // Validate input JSON
    let input_bytes = args.input.as_bytes();
    if let Err(e) = serde_json::from_str::<serde_json::Value>(&args.input) {
        println!("⚠️  Warning: Input doesn't appear to be valid JSON: {}", e);
    }

    // Parse custom resource limits if provided
    let resource_limits = if let Some(limits_json) = args.limits {
        serde_json::from_str(&limits_json)
            .with_context(|| "Failed to parse resource limits JSON")?    
    } else {
        // Use development-friendly defaults for testing
        ResourceLimits {
            memory_limit_mb: 64,
            execution_time_ms: 5000,
            fuel_limit: 10_000_000,
            capabilities: vec![
                Capability::Hash,
                Capability::Json,
                Capability::Base64,
                Capability::Time,
            ],
        }
    };

    println!("⚙️  Resource limits:");
    println!("   Memory: {} MB", resource_limits.memory_limit_mb);
    println!("   Time: {} ms", resource_limits.execution_time_ms);
    println!("   Fuel: {}", resource_limits.fuel_limit);
    println!("   Capabilities: {:?}", resource_limits.capabilities);
    println!();

    // Create runtime with test signing key
    let signing_key = generate_test_signing_key();
    let mut runtime = WasmRuntime::new(signing_key)?;

    println!("🚀 Executing capsule...");
    let start_time = std::time::Instant::now();

    // Execute the capsule
    let result = match runtime.execute(&capsule_bytes, input_bytes, resource_limits).await {
        Ok(result) => result,
        Err(e) => {
            println!("❌ Execution failed: {}", e);
            return Err(e.into());
        }
    };

    let total_time = start_time.elapsed();
    println!("✅ Execution completed in {:.3}ms", total_time.as_millis());
    println!();

    // Display output
    println!("📤 Output ({} bytes):", result.output.len());
    match String::from_utf8(result.output.clone()) {
        Ok(output_str) => {
            // Try to pretty-print JSON
            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&output_str) {
                println!("{}", serde_json::to_string_pretty(&json_value)?);
            } else {
                println!("{}", output_str);
            }
        }
        Err(_) => {
            println!("[Binary output - {} bytes]", result.output.len());
            if result.output.len() <= 64 {
                println!("Hex: {}", hex::encode(&result.output));
            }
        }
    }
    println!();

    // Display execution metrics
    if args.metrics {
        println!("📊 Execution Metrics:");
        println!("   Fuel used: {}", result.metrics.fuel_used);
        println!("   Memory: {:.3} MB", result.metrics.memory_mb);
        println!("   Duration: {} ms", result.metrics.duration_ms);
        println!("   Host calls: {}", result.metrics.host_function_calls);
        println!();
    }

    // Display receipt information
    println!("🧾 Execution Receipt:");
    println!("   Receipt ID: {}", result.receipt.receipt_id());
    println!("   Capsule ID: {}", result.receipt.capsule_id);
    println!("   Input commit: {}", result.receipt.input_commit);
    println!("   Output commit: {}", result.receipt.output_commit);
    println!("   Node ID: {}", result.receipt.node_id);
    println!("   Timestamp: {}", result.receipt.timestamp);
    println!("   Signature: {}...", &result.receipt.signature[..16]);

    // Verify the receipt
    match result.receipt.verify_node_signature() {
        Ok(true) => println!("   ✅ Receipt signature valid"),
        Ok(false) => println!("   ❌ Receipt signature invalid"),
        Err(e) => println!("   ⚠️  Receipt verification error: {}", e),
    }
    println!();

    // Show full receipt if requested
    if args.show_receipt {
        println!("📋 Full Receipt (JSON):");
        println!("{}", result.receipt.to_json()?);
        println!();
    }

    // Summary
    println!("🎉 Test completed successfully!");
    println!("   Total time: {:.3}ms", total_time.as_millis());
    println!("   Output size: {} bytes", result.output.len());
    println!("   Receipt ID: {}", result.receipt.receipt_id());

    Ok(())
}

/// Validate a capsule file without executing it
pub fn validate_capsule_file(capsule_path: &str) -> Result<()> {
    println!("🔍 Validating capsule: {}", capsule_path);
    
    let capsule_bytes = fs::read(capsule_path)
        .with_context(|| format!("Failed to read capsule file: {}", capsule_path))?;
    
    let validation_result = tenzik_runtime::validate_capsule(&capsule_bytes)?;
    
    if validation_result.is_valid {
        println!("✅ Capsule validation passed");
        println!("   Size: {:.2} KB", validation_result.size_kb);
        println!("   Exports: {:?}", validation_result.exports);
        println!("   Imports: {:?}", validation_result.imports);
        
        if !validation_result.warnings.is_empty() {
            println!("⚠️  Warnings:");
            for warning in &validation_result.warnings {
                println!("   - {}", warning);
            }
        }
    } else {
        println!("❌ Capsule validation failed");
        for error in &validation_result.errors {
            println!("   Error: {}", error);
        }
        anyhow::bail!("Capsule validation failed");
    }
    
    Ok(())
}

/// Generate a test signing key for development
fn generate_test_signing_key() -> ed25519_dalek::SigningKey {
    use rand::rngs::OsRng;
    ed25519_dalek::SigningKey::generate(&mut OsRng)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_validate_nonexistent_file() {
        let result = validate_capsule_file("nonexistent.wasm");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_test_args() {
        let args = TestArgs {
            capsule: "test.wasm".to_string(),
            input: "{\"test\": \"value\"}".to_string(),
            metrics: true,
            show_receipt: false,
            limits: None,
        };
        
        assert_eq!(args.capsule, "test.wasm");
        assert!(args.metrics);
        assert!(!args.show_receipt);
    }
}
