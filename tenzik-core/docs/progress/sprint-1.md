# Sprint 1: Runtime Hardening & Receipts

**Duration**: Weeks 1-2  
**Objective**: Execute capsules safely and emit signed ExecutionReceipts  
**Success Criteria**: `tenzik test capsule.wasm '{"input":"data"}'` works end-to-end

## Progress Dashboard

### 🎯 Sprint 1 Gates - ✅ ALL COMPLETED!
- [x] **RT-003**: WASM validation logic
- [x] **RT-004**: Capability mapper / import allowlist
- [x] **RT-005**: Resource limits enforcement
- [x] **ECON-002**: ExecutionReceipt (sign/verify)
- [x] **CLI-101**: `tenzik test` happy path

### 📊 Implementation Status

| Component | File | Status | Implementation | Tests | Docs |
|-----------|------|--------|----------------|-------|------|
| WASM Validation | `runtime/src/validation.rs` | ✅ Complete | 100% | 100% | 100% |
| Capability System | `runtime/src/sandbox.rs` | ✅ Complete | 100% | 100% | 100% |
| Resource Limits | `runtime/src/execution.rs` | ✅ Complete | 100% | 90% | 90% |
| ExecutionReceipt | `runtime/src/receipts.rs` | ✅ Complete | 100% | 100% | 100% |
| CLI Test Command | `cli/src/commands/test.rs` | ✅ Complete | 100% | 80% | 90% |

### 📁 Documentation Structure

```
docs/
├── progress/
│   ├── sprint-1.md          (this file)
│   ├── sprint-2.md          (federation planning)
│   └── completion-log.md    (daily progress)
├── architecture/
│   ├── runtime-design.md    (WASM execution architecture)
│   ├── security-model.md    (capability system design)
│   └── receipt-protocol.md  (ExecutionReceipt specification)
├── api/
│   ├── runtime-api.md       (runtime crate API)
│   ├── protocol-api.md      (protocol crate API)
│   └── cli-api.md           (CLI commands)
└── tutorials/
    ├── capsule-development.md
    ├── testing-guide.md
    └── deployment-guide.md
```

## Sprint 1 Implementation Plan

### Week 1: Core Runtime Foundation

#### Day 1-2: WASM Validation (RT-003)
**File**: `crates/runtime/src/validation.rs`

**Requirements**:
- Validate required exports (`run`, `memory`)
- Check WASM binary size limits (target: 5KB max)  
- Verify import allowlist compliance
- Reject invalid/malicious modules

**Success Criteria**:
```rust
let validator = WasmValidator::new();
let result = validator.validate(&wasm_bytes)?;
assert!(result.is_valid);
assert!(result.size_kb <= 5);
```

#### Day 3-4: Capability System (RT-004)  
**File**: `crates/runtime/src/sandbox.rs`

**Requirements**:
- Implement capability enum from migration patterns
- Create capability-to-import mapper
- Host function allowlist enforcement
- Security isolation

**Success Criteria**:
```rust
let sandbox = SecuritySandbox::new(&[Capability::Hash, Capability::Json]);
assert!(sandbox.allows_import("env::hash_commit"));
assert!(!sandbox.allows_import("env::network_fetch"));
```

#### Day 5: Resource Limits (RT-005)
**File**: `crates/runtime/src/execution.rs`

**Requirements**:
- Memory limits (default: 32MB)
- Execution time limits (default: 1000ms)
- Fuel metering integration with wasmtime
- Graceful timeout handling

### Week 2: Receipts & Integration

#### Day 6-7: ExecutionReceipt (ECON-002)
**File**: `crates/runtime/src/receipts.rs`

**Requirements**:
- Blake3 input/output commitments
- Ed25519 signature generation/verification  
- Deterministic receipt format
- JSON serialization

**Success Criteria**:
```rust
let receipt = ExecutionReceipt::new(capsule_bytes, input, output, metrics, &signing_key, nonce);
assert!(receipt.verify(&verifying_key));
```

#### Day 8-9: CLI Integration (CLI-101)
**File**: `crates/cli/src/commands/test.rs`

**Requirements**:
- Load WASM from file
- Parse JSON input
- Execute via runtime
- Display receipt + metrics
- Error handling

**Success Criteria**:
```bash
cargo run -p tenzik-cli -- test hello.wasm '{"name":"Alice"}' --metrics
# Output: Receipt ID + execution metrics + result
```

#### Day 10: End-to-End Testing
- Integration tests across all components
- Error handling verification
- Performance baseline measurement
- Documentation updates

## Documentation Links

### Architecture Documents
- [Runtime Design](../architecture/runtime-design.md) - Core WASM execution architecture
- [Security Model](../architecture/security-model.md) - Capability-based sandbox design
- [Receipt Protocol](../architecture/receipt-protocol.md) - ExecutionReceipt specification

### API Documentation  
- [Runtime API](../api/runtime-api.md) - WasmRuntime, SecuritySandbox APIs
- [Protocol API](../api/protocol-api.md) - ExecutionReceipt, Event types
- [CLI API](../api/cli-api.md) - Command-line interface specification

### Implementation Guides
- [Capsule Development](../tutorials/capsule-development.md) - Writing 3-5KB WASM capsules
- [Testing Guide](../tutorials/testing-guide.md) - Local testing workflow
- [Security Guidelines](../tutorials/security-guidelines.md) - Safe capsule patterns

## Success Metrics

### Gate A: Component Integration - ✅ ACHIEVED
**Target**: End of Week 1
- [x] All runtime components compile and link
- [x] Basic WASM validation works
- [x] Capability system enforces restrictions
- [x] Resource limits prevent runaway execution

### Gate B: End-to-End Flow - ✅ ACHIEVED
**Target**: End of Week 2
- [x] CLI loads and executes WASM capsules
- [x] ExecutionReceipts generated and verified
- [x] Error handling graceful and informative
- [x] Performance within acceptable bounds (< 100ms for hello-world)

### Quality Gates - ✅ ACHIEVED
- [x] All public APIs documented with rustdoc
- [x] Integration tests cover happy path + error cases
- [x] Security review of capability system completed
- [x] Performance baseline established

## Risk Mitigation

### Technical Risks
- **WASM size constraints**: Provide size optimization guidance, host-provided primitives
- **Wasmtime integration**: Start with simple cases, add complexity gradually
- **Cryptographic correctness**: Use well-tested libraries (blake3, ed25519-dalek)

### Schedule Risks  
- **Scope creep**: Defer ZK integration to Sprint 3
- **Integration complexity**: Build incrementally with frequent testing
- **Performance issues**: Establish baseline early, optimize if needed

## Next Sprint Preview

**Sprint 2: Minimal Federation (Weeks 3-4)**
- Event DAG implementation
- Simple gossip protocol  
- Two-node receipt exchange
- Basic peer discovery

The foundation built in Sprint 1 will enable rapid federation development in Sprint 2.
