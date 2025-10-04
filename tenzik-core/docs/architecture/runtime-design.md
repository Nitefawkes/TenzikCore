# Runtime Design Architecture

## Overview

The Tenzik runtime is designed to execute small WASM capsules (3-5KB) with strict security and resource controls. This document outlines the core architecture and design decisions.

## Core Components

### 1. WASM Validation (`validation.rs`)

**Purpose**: Ensure WASM modules meet Tenzik safety and size requirements before execution.

**Key Responsibilities**:
- Size validation (≤ 5KB target, configurable maximum)
- Required export verification (`run` function, `memory` export)
- Import allowlist enforcement (only approved host functions)
- Basic WASM structure validation
- Malformed module rejection

**Design Principles**:
- Fail-fast validation before expensive compilation
- Clear error messages for developers
- Configurable limits for different deployment scenarios
- Zero-tolerance for security violations

### 2. Security Sandbox (`sandbox.rs`)

**Purpose**: Implement capability-based access control for WASM modules.

**Key Responsibilities**:
- Capability enum definition and checking
- Host function allowlist management
- Resource limit enforcement
- Security boundary maintenance

**Capability Model**:
```rust
pub enum Capability {
    Hash,      // Blake3 hashing via host
    Json,      // JSON path operations via host  
    Base64,    // Base64 encoding via host
    Time,      // Timestamp access (deterministic)
    Random,    // PRNG access (deterministic seed)
}
```

**Security Boundaries**:
- No network I/O in MVP
- No file system access in MVP
- No process execution capabilities
- Host-provided primitives only

### 3. Execution Engine (`execution.rs`)

**Purpose**: Execute validated WASM capsules with resource controls and metrics collection.

**Key Responsibilities**:
- Wasmtime integration and configuration
- Resource limit enforcement (memory, time, fuel)
- Execution metrics collection
- Error handling and timeout management
- Host function implementation

**Resource Model**:
```rust
pub struct ResourceLimits {
    pub memory_limit_mb: u32,      // Default: 32MB
    pub execution_time_ms: u64,    // Default: 1000ms  
    pub fuel_limit: u64,           // Wasmtime fuel units
    pub capabilities: Vec<Capability>,
}
```

### 4. Execution Receipts (`receipts.rs`)

**Purpose**: Generate cryptographic receipts for all executions to enable verification.

**Key Responsibilities**:
- Input/output commitment generation (Blake3)
- Ed25519 signature creation and verification
- Deterministic receipt format
- JSON serialization for federation
- Nonce management for replay protection

**Receipt Model**:
```rust
pub struct ExecutionReceipt {
    pub capsule_id: String,        // Blake3 of WASM bytes
    pub input_commit: String,      // Blake3 of input JSON
    pub output_commit: String,     // Blake3 of output JSON
    pub exec_metrics: ExecMetrics, // Resource usage
    pub node_id: String,          // Ed25519 public key
    pub nonce: u64,               // Replay protection
    pub signature: String,        // Ed25519 signature
    pub timestamp: String,        // ISO 8601 timestamp
}
```

## Data Flow

### Execution Pipeline

1. **Validation Phase**
   ```
   WASM bytes → WasmValidator → ValidationResult
   ```
   - Size check, export verification, import allowlist
   - Early rejection of invalid modules

2. **Preparation Phase**
   ```
   ValidationResult → SecuritySandbox → ExecutionContext
   ```
   - Capability checking, resource limit setup
   - Host function binding

3. **Execution Phase**
   ```
   ExecutionContext + Input → WasmRuntime → Output + Metrics
   ```
   - Wasmtime execution with timeouts
   - Resource monitoring and enforcement

4. **Receipt Generation**
   ```
   Input + Output + Metrics → ExecutionReceipt → Signed Receipt
   ```
   - Cryptographic commitments and signatures
   - JSON serialization for storage/federation

### Host Function Interface

Host functions are the primary mechanism for capsules to access external capabilities:

```rust
// Example: Blake3 hashing
fn host_hash_commit(input_ptr: i32, input_len: i32) -> i32 {
    // Read input from WASM memory
    // Compute Blake3 hash
    // Write result to WASM memory
    // Return result pointer/length
}
```

**Host Function Categories**:
- **Cryptographic**: `hash_commit`, `hash_verify`
- **Data Processing**: `json_path`, `base64_encode`, `base64_decode`
- **System**: `time_now_ms` (deterministic), `random_bytes` (seeded)

## Security Model

### Isolation Levels

1. **Process Isolation**: Each execution in separate wasmtime instance
2. **Memory Isolation**: WASM linear memory sandboxing  
3. **Capability Isolation**: Explicit capability grants required
4. **Resource Isolation**: Strict CPU/memory/time limits

### Threat Model

**Protected Against**:
- Resource exhaustion (memory bombs, infinite loops)
- Information disclosure (no file/network access)
- Privilege escalation (capability boundaries)
- Code injection (WASM validation)

**Assumptions**:
- Host environment is trusted
- Cryptographic primitives are secure
- Wasmtime provides correct isolation

### Audit Points

- All imports validated against allowlist
- Resource usage logged and limited
- All executions generate receipts
- Capability usage tracked and logged

## Performance Considerations

### Size Optimization

Target capsule size of 3-5KB achieved through:
- Host-provided primitives (reduce WASM code size)
- AssemblyScript compilation with aggressive optimization
- wasm-opt post-processing with `-Oz` flag
- Runtime compilation caching

### Execution Optimization

- Module compilation caching by content hash
- Fuel metering for fair resource allocation
- Timeout handling to prevent blocking
- Memory pre-allocation for predictable performance

### Scalability

- Stateless execution model (no persistent state in runtime)
- Parallel execution capability (multiple runtime instances)
- Receipt batching for federation efficiency
- Resource pooling for high-throughput scenarios

## Configuration

### Runtime Configuration

```rust
pub struct RuntimeConfig {
    pub max_capsule_size_kb: u32,     // Default: 5KB
    pub default_memory_limit_mb: u32, // Default: 32MB
    pub default_time_limit_ms: u64,   // Default: 1000ms
    pub fuel_per_instruction: u64,    // Wasmtime fuel config
    pub enable_compilation_cache: bool, // Default: true
    pub cache_size_limit: usize,      // Default: 100 modules
}
```

### Security Configuration

```rust
pub struct SecurityConfig {
    pub allowed_capabilities: Vec<Capability>,
    pub require_deterministic: bool,  // Default: true
    pub enable_debugging: bool,       // Default: false in production
    pub max_host_calls_per_execution: u32, // Rate limiting
}
```

## Error Handling

### Error Categories

1. **Validation Errors**: Size limits, missing exports, invalid imports
2. **Runtime Errors**: Resource exhaustion, timeouts, traps
3. **Security Errors**: Capability violations, unauthorized imports
4. **System Errors**: Host function failures, serialization errors

### Error Propagation

```rust
pub enum RuntimeError {
    Validation(ValidationError),
    Execution(ExecutionError),
    Security(SecurityError),
    System(SystemError),
}
```

All errors include:
- Clear error messages for developers
- Context information (capsule ID, input hash)
- Suggested remediation when possible
- Security-safe error details (no sensitive data leakage)

## Testing Strategy

### Unit Testing
- Individual component testing with mocks
- Property-based testing for validation logic
- Fuzzing for WASM parsing robustness

### Integration Testing  
- End-to-end execution flows
- Resource limit enforcement verification
- Security boundary testing

### Performance Testing
- Execution time benchmarks
- Memory usage profiling
- Throughput testing under load

This architecture provides a secure, performant foundation for executing small WASM capsules with full auditability through cryptographic receipts.
