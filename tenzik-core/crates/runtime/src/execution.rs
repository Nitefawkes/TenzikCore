//! WASM Execution Engine
//!
//! This module provides the main execution engine for Tenzik WASM capsules.
//! It integrates validation, sandboxing, resource limits, and receipt generation.

use crate::receipts::{ExecMetrics, ExecutionReceipt, ReceiptError};
use crate::sandbox::{ResourceLimits, SecuritySandbox, SandboxError};
use crate::validation::{WasmValidator, ValidationError, ValidationResult};

use anyhow::{Context, Result};
use blake3;
use ed25519_dalek::SigningKey;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::time::timeout;
use wasmtime::{
    Config, Engine, Func, Instance, Linker, Memory, MemoryType, Module, Store, TypedFunc, Val,
};

/// Maximum input/output size in bytes (1MB)
const MAX_IO_SIZE: usize = 1024 * 1024;

/// Execution errors
#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("Validation failed: {source}")]
    ValidationFailed { source: ValidationError },

    #[error("Sandbox error: {source}")]
    SandboxError { source: SandboxError },

    #[error("WASM execution failed: {reason}")]
    ExecutionFailed { reason: String },

    #[error("Timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("Resource limit exceeded: {limit_type}")]
    ResourceLimitExceeded { limit_type: String },

    #[error("Input/output error: {reason}")]
    IOError { reason: String },

    #[error("Receipt generation failed: {source}")]
    ReceiptError { source: ReceiptError },

    #[error("Host function error: {function} - {reason}")]
    HostFunctionError { function: String, reason: String },
}

/// Execution result containing output and metrics
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Output from the capsule execution
    pub output: Vec<u8>,
    /// Execution metrics
    pub metrics: ExecMetrics,
    /// Generated execution receipt
    pub receipt: ExecutionReceipt,
}

/// Runtime configuration
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Whether to enable fuel metering
    pub enable_fuel: bool,
    /// Whether to enable compilation caching
    pub enable_cache: bool,
    /// Maximum input/output size
    pub max_io_size: usize,
    /// Whether to collect detailed metrics
    pub detailed_metrics: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            enable_fuel: true,
            enable_cache: true,
            max_io_size: MAX_IO_SIZE,
            detailed_metrics: true,
        }
    }
}

/// Host function implementation
struct HostFunctions {
    sandbox: Arc<SecuritySandbox>,
}

impl HostFunctions {
    fn new(sandbox: Arc<SecuritySandbox>) -> Self {
        Self { sandbox }
    }

    /// Blake3 hash commit function
    fn hash_commit(&self, mut caller: wasmtime::Caller<'_, ()>, ptr: i32, len: i32) -> i32 {
        // Implementation would read from WASM memory, compute hash, write back
        // For now, return success (0)
        0
    }

    /// JSON path extraction function
    fn json_path(
        &self,
        mut caller: wasmtime::Caller<'_, ()>,
        data_ptr: i32,
        data_len: i32,
        path_ptr: i32,
        path_len: i32,
    ) -> i32 {
        // Implementation would extract JSON path and return result
        // For now, return success (0)
        0
    }

    /// Base64 encoding function
    fn base64_encode(&self, mut caller: wasmtime::Caller<'_, ()>, ptr: i32, len: i32) -> i32 {
        // Implementation would base64 encode and return result
        // For now, return success (0)
        0
    }

    /// Get current timestamp in milliseconds
    fn time_now_ms(&self, mut caller: wasmtime::Caller<'_, ()>) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }

    /// Generate random bytes
    fn random_bytes(&self, mut caller: wasmtime::Caller<'_, ()>, ptr: i32, len: i32) -> i32 {
        // Implementation would generate deterministic random bytes
        // For now, return success (0)
        0
    }
}

/// Main WASM runtime for executing capsules
pub struct WasmRuntime {
    /// Wasmtime engine
    engine: Engine,
    /// Runtime configuration
    config: RuntimeConfig,
    /// WASM validator
    validator: WasmValidator,
    /// Signing key for receipts
    signing_key: SigningKey,
    /// Nonce counter for receipts
    nonce_counter: u64,
}

impl WasmRuntime {
    /// Create a new WASM runtime
    pub fn new(signing_key: SigningKey) -> Result<Self> {
        Self::with_config(signing_key, RuntimeConfig::default())
    }

    /// Create a new runtime with custom configuration
    pub fn with_config(signing_key: SigningKey, config: RuntimeConfig) -> Result<Self> {
        // Configure Wasmtime engine
        let mut wasmtime_config = Config::new();
        wasmtime_config.wasm_simd(false); // Disable SIMD for smaller capsules
        wasmtime_config.wasm_multi_value(false); // Disable multi-value
        wasmtime_config.wasm_bulk_memory(false); // Disable bulk memory
        wasmtime_config.consume_fuel(config.enable_fuel);

        let engine = Engine::new(&wasmtime_config).context("Failed to create Wasmtime engine")?;

        let validator = WasmValidator::new().context("Failed to create WASM validator")?;

        Ok(Self {
            engine,
            config,
            validator,
            signing_key,
            nonce_counter: 1,
        })
    }

    /// Execute a WASM capsule with the given input
    pub async fn execute(
        &mut self,
        capsule_bytes: &[u8],
        input: &[u8],
        resource_limits: ResourceLimits,
    ) -> Result<ExecutionResult, ExecutionError> {
        let start_time = Instant::now();

        // Validate input size
        if input.len() > self.config.max_io_size {
            return Err(ExecutionError::IOError {
                reason: format!(
                    "Input too large: {} bytes (max: {})",
                    input.len(),
                    self.config.max_io_size
                ),
            });
        }

        // Step 1: Validate WASM capsule
        let validation_result = self
            .validator
            .validate(capsule_bytes)
            .map_err(|e| ExecutionError::ValidationFailed {
                source: ValidationError::InvalidModule {
                    reason: e.to_string(),
                },
            })?;

        if !validation_result.is_valid {
            return Err(ExecutionError::ValidationFailed {
                source: validation_result.errors[0].clone(),
            });
        }

        // Step 2: Set up security sandbox
        let sandbox = Arc::new(SecuritySandbox::new(resource_limits.clone()));

        // Step 3: Compile WASM module
        let module = Module::from_binary(&self.engine, capsule_bytes)
            .map_err(|e| ExecutionError::ExecutionFailed {
                reason: format!("Module compilation failed: {}", e),
            })?;

        // Step 4: Execute with timeout
        let execution_timeout = Duration::from_millis(resource_limits.execution_time_ms);

        let execution_future = self.execute_module(module, input, sandbox.clone());

        let (output, exec_metrics) = match timeout(execution_timeout, execution_future).await {
            Ok(result) => result?,
            Err(_) => {
                return Err(ExecutionError::Timeout {
                    timeout_ms: resource_limits.execution_time_ms,
                })
            }
        };

        // Step 5: Generate execution receipt
        let receipt = ExecutionReceipt::new(
            capsule_bytes,
            input,
            &output,
            exec_metrics.clone(),
            &self.signing_key,
            self.nonce_counter,
        )
        .map_err(|e| ExecutionError::ReceiptError { source: e })?;

        self.nonce_counter += 1;

        Ok(ExecutionResult {
            output,
            metrics: exec_metrics,
            receipt,
        })
    }

    /// Execute a compiled WASM module
    async fn execute_module(
        &self,
        module: Module,
        input: &[u8],
        sandbox: Arc<SecuritySandbox>,
    ) -> Result<(Vec<u8>, ExecMetrics), ExecutionError> {
        let start_time = Instant::now();

        // Create store with fuel if enabled
        let mut store = Store::new(&self.engine, ());
        if self.config.enable_fuel {
            store
                .add_fuel(sandbox.resource_limits().fuel_limit)
                .map_err(|e| ExecutionError::ExecutionFailed {
                    reason: format!("Failed to add fuel: {}", e),
                })?;
        }

        // Set memory limits
        store.limiter(|_| {
            wasmtime::ResourceLimiterAsync::new(
                sandbox.resource_limits().memory_limit_mb as usize * 1024 * 1024, // Convert MB to bytes
                1000, // Max table elements
                10,   // Max instances
                1000, // Max tables
                1000, // Max memories
            )
        });

        // Create linker with host functions
        let mut linker = Linker::new(&self.engine);

        // Add host functions based on capabilities
        let host_functions = HostFunctions::new(sandbox.clone());

        if sandbox.has_capability(crate::sandbox::Capability::Hash) {
            linker
                .func_wrap(
                    "env",
                    "hash_commit",
                    |caller: wasmtime::Caller<'_, ()>, ptr: i32, len: i32| -> i32 { 0 },
                )
                .map_err(|e| ExecutionError::ExecutionFailed {
                    reason: format!("Failed to link hash_commit: {}", e),
                })?;
        }

        if sandbox.has_capability(crate::sandbox::Capability::Json) {
            linker
                .func_wrap(
                    "env",
                    "json_path",
                    |caller: wasmtime::Caller<'_, ()>,
                     data_ptr: i32,
                     data_len: i32,
                     path_ptr: i32,
                     path_len: i32|
                     -> i32 { 0 },
                )
                .map_err(|e| ExecutionError::ExecutionFailed {
                    reason: format!("Failed to link json_path: {}", e),
                })?;
        }

        if sandbox.has_capability(crate::sandbox::Capability::Time) {
            linker
                .func_wrap("env", "time_now_ms", |caller: wasmtime::Caller<'_, ()>| -> i64 {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64
                })
                .map_err(|e| ExecutionError::ExecutionFailed {
                    reason: format!("Failed to link time_now_ms: {}", e),
                })?;
        }

        // Instantiate the module
        let instance = linker
            .instantiate_async(&mut store, &module)
            .await
            .map_err(|e| ExecutionError::ExecutionFailed {
                reason: format!("Module instantiation failed: {}", e),
            })?;

        // Get the main function and memory
        let run_func: TypedFunc<(i32, i32), i32> = instance
            .get_typed_func(&mut store, "run")
            .map_err(|e| ExecutionError::ExecutionFailed {
                reason: format!("Failed to get 'run' function: {}", e),
            })?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| ExecutionError::ExecutionFailed {
                reason: "Module missing 'memory' export".to_string(),
            })?;

        // Write input to WASM memory
        let input_ptr = 1024; // Start at 1KB offset
        if input_ptr + input.len() > memory.data_size(&store) {
            return Err(ExecutionError::ExecutionFailed {
                reason: "Input too large for WASM memory".to_string(),
            });
        }

        memory
            .write(&mut store, input_ptr, input)
            .map_err(|e| ExecutionError::ExecutionFailed {
                reason: format!("Failed to write input to memory: {}", e),
            })?;

        // Execute the function
        let result = run_func
            .call_async(&mut store, (input_ptr as i32, input.len() as i32))
            .await
            .map_err(|e| ExecutionError::ExecutionFailed {
                reason: format!("Function execution failed: {}", e),
            })?;

        // Extract output from result (encoded as length in high bits, ptr in low bits)
        let output_len = (result >> 16) as usize;
        let output_ptr = (result & 0xFFFF) as usize;

        if output_len > self.config.max_io_size {
            return Err(ExecutionError::IOError {
                reason: format!(
                    "Output too large: {} bytes (max: {})",
                    output_len, self.config.max_io_size
                ),
            });
        }

        // Read output from WASM memory
        let mut output = vec![0u8; output_len];
        memory
            .read(&store, output_ptr, &mut output)
            .map_err(|e| ExecutionError::ExecutionFailed {
                reason: format!("Failed to read output from memory: {}", e),
            })?;

        // Collect execution metrics
        let duration = start_time.elapsed();
        let fuel_used = if self.config.enable_fuel {
            sandbox.resource_limits().fuel_limit
                - store.fuel_remaining().unwrap_or(0)
        } else {
            0
        };

        let metrics = ExecMetrics {
            fuel_used,
            memory_mb: memory.data_size(&store) as f64 / (1024.0 * 1024.0),
            duration_ms: duration.as_millis() as u64,
            host_function_calls: 0, // TODO: Track from sandbox access log
        };

        Ok((output, metrics))
    }

    /// Get the next nonce value
    pub fn next_nonce(&self) -> u64 {
        self.nonce_counter
    }

    /// Get the runtime's public key
    pub fn public_key(&self) -> ed25519_dalek::VerifyingKey {
        self.signing_key.verifying_key()
    }
}

/// Execution metrics for monitoring and optimization
#[derive(Debug, Clone)]
pub struct ExecutionMetrics {
    /// Number of executions performed
    pub total_executions: u64,
    /// Average execution time in milliseconds
    pub avg_execution_time_ms: f64,
    /// Peak memory usage across all executions
    pub peak_memory_mb: f64,
    /// Total fuel consumed
    pub total_fuel_used: u64,
}

impl Default for ExecutionMetrics {
    fn default() -> Self {
        Self {
            total_executions: 0,
            avg_execution_time_ms: 0.0,
            peak_memory_mb: 0.0,
            total_fuel_used: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::receipts::generate_test_signing_key;
    use crate::sandbox::Capability;

    fn create_minimal_wasm() -> Vec<u8> {
        // A minimal WASM module that exports 'run' and 'memory'
        // This is a placeholder - in real tests we'd use a proper WASM module
        vec![
            0x00, 0x61, 0x73, 0x6d, // Magic number
            0x01, 0x00, 0x00, 0x00, // Version
        ]
    }

    #[tokio::test]
    async fn test_runtime_creation() {
        let signing_key = generate_test_signing_key();
        let runtime = WasmRuntime::new(signing_key);
        assert!(runtime.is_ok());
    }

    #[tokio::test]
    async fn test_input_size_validation() {
        let signing_key = generate_test_signing_key();
        let mut runtime = WasmRuntime::new(signing_key).unwrap();

        let large_input = vec![0u8; MAX_IO_SIZE + 1];
        let capsule = create_minimal_wasm();
        let limits = ResourceLimits::default();

        let result = runtime.execute(&capsule, &large_input, limits).await;
        assert!(matches!(result, Err(ExecutionError::IOError { .. })));
    }

    #[test]
    fn test_runtime_config() {
        let config = RuntimeConfig {
            enable_fuel: false,
            enable_cache: true,
            max_io_size: 512,
            detailed_metrics: false,
        };

        assert!(!config.enable_fuel);
        assert_eq!(config.max_io_size, 512);
    }

    #[test]
    fn test_execution_metrics() {
        let metrics = ExecutionMetrics::default();
        assert_eq!(metrics.total_executions, 0);
        assert_eq!(metrics.avg_execution_time_ms, 0.0);
    }
}
