//! Tenzik WASM runtime with security sandbox
//!
//! This crate provides a secure WebAssembly runtime for executing small
//! capsules (3-5KB WASM modules) with strict resource limits and capability controls.

pub mod validation;
pub mod sandbox;
pub mod execution;
pub mod receipts;

// Re-export key types for easy access
pub use validation::{WasmValidator, ValidationResult, ValidationError, ValidatorConfig};
pub use sandbox::{Capability, ResourceLimits, SecuritySandbox, SandboxError};
pub use execution::{WasmRuntime, ExecutionResult, ExecutionError, RuntimeConfig};
pub use receipts::{ExecutionReceipt, ExecMetrics, ReceiptError, ReceiptVerifier};

// Re-export crypto types for convenience
pub use ed25519_dalek::{SigningKey, VerifyingKey};

/// Convenience function to validate a WASM capsule
pub fn validate_capsule(wasm_bytes: &[u8]) -> anyhow::Result<ValidationResult> {
    validation::validate_capsule(wasm_bytes)
}

/// Generate a new signing key for development/testing
#[cfg(test)]
pub fn generate_test_signing_key() -> SigningKey {
    receipts::generate_test_signing_key()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
