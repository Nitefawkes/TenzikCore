# Tenzik Migration Notes

## Migration Summary

**Date**: December 2024  
**Approach**: Fresh start with selective pattern reuse (70% fresh, 30% patterns)  
**Previous**: Complex Tent-based federated platform  
**Current**: Focused edge compute with verifiable receipts

## Key Architectural Changes

### Scope Simplification
- **Dropped**: Tent protocol dependency, marketplace, dashboards, complex permissions
- **Focused**: 3-5KB capsules, simple receipts, optional ZK, great DevEx

### Language Consolidation  
- **From**: Mixed TypeScript/JavaScript runtime + Rust protocol
- **To**: Rust-first workspace with focused scope

### Federation Model
- **From**: Complex Tent federation with full event system
- **To**: Simple DAG-based gossip for receipt exchange only

### ZK Strategy
- **From**: ZK-first with mandatory proofs
- **To**: Optional/deferred with pluggable backends (Risc0/SP1/TEE)

## Reusable Patterns Extracted

### 1. Security Sandbox Concepts
**Source**: `packages/wasm-runtime/src/security-sandbox.ts`

```rust
// Pattern: Capability-based access control
pub enum Capability {
    Time,      // time-related functions
    Random,    // random number generation  
    Hash,      // blake3 hashing (host-provided)
    Json,      // JSON path operations (host-provided)
    Base64,    // base64 encoding (host-provided)
    // Removed: FILE_ACCESS, NETWORK_ACCESS (not in MVP)
}

pub struct ResourceLimits {
    pub memory_limit_mb: u32,    // Default: 32MB for 3-5KB capsules
    pub execution_time_ms: u64,  // Default: 1000ms
    pub capabilities: Vec<Capability>,
}
```

**Implementation Target**: `crates/runtime/src/sandbox.rs`

### 2. Function Metadata Structure
**Source**: `packages/protocol/src/function.rs`

```rust
// Pattern: Function metadata with hash verification
// Simplified for new roadmap:

pub struct CapsuleMetadata {
    pub id: String,           // blake3 hash of WASM bytes
    pub name: String,
    pub description: String,
    pub wasm_binary: Vec<u8>, // Direct bytes, not base64
    pub hash: String,         // blake3 verification
    pub resources: ResourceLimits,
    // Removed: complex interface, triggers, permissions
}
```

**Implementation Target**: `crates/protocol/src/receipts.rs`

### 3. CLI Command Patterns
**Source**: `packages/cli/src/index.ts`

```rust
// Pattern: Command structure with subcommands
// Adapted to new roadmap scope:

Commands:
- init [name] --template <template>     // Project scaffolding
- test <capsule.wasm> <input> --metrics // Local execution  
- node --peer <addr> --db <path>        // Federation node
- receipt verify <receipt_id>           // Verification

// Removed: build, deploy, sandbox:init (complex workflows)
```

**Implementation Target**: `crates/cli/src/main.rs` ✅ Done

### 4. Execution Metrics
**Source**: `packages/wasm-runtime/src/index.ts`

```rust
// Pattern: Resource usage tracking
pub struct ExecMetrics {
    pub gas_used: u64,
    pub memory_mb: f64,  
    pub duration_ms: u64,
    pub host_function_calls: u32,
}

// Applied to ExecutionReceipt for transparency
```

**Implementation Target**: `crates/runtime/src/execution.rs`

## What Was Deliberately Dropped

### Enterprise Complexity
- **Marketplace**: Function sharing/discovery (future phase)
- **Dashboard**: Complex UI (CLI-first MVP)  
- **Audit/Compliance**: Enterprise features (future phase)
- **UI Kit**: Component library (no UI in MVP)

### Protocol Complexity
- **Tent Protocol**: Full federated event system
- **Complex Permissions**: Entity-specific access control
- **Advanced Triggers**: Event pattern matching
- **Identity Management**: Enterprise identity integration

### ZK Complexity  
- **Mandatory ZK**: Required proof generation
- **ZK-VM**: Custom circuit compilation
- **Complex Verification**: Multi-backend proof validation

## Implementation Strategy

### Phase 1: Core Runtime (Current Sprint 1)
**Files to implement**:
- `crates/runtime/src/validation.rs` - WASM validation rules
- `crates/runtime/src/sandbox.rs` - Capability system  
- `crates/runtime/src/execution.rs` - Main runtime
- `crates/runtime/src/receipts.rs` - ExecutionReceipt

**Success Criteria**: Execute capsule → get signed receipt

### Phase 2: Minimal Federation (Sprint 2)  
**Files to implement**:
- `crates/federation/src/storage.rs` - Event DAG with sled
- `crates/federation/src/node.rs` - Node announcement
- `crates/federation/src/gossip.rs` - Simple peer-to-peer

**Success Criteria**: Two nodes exchange receipts

### Phase 3: Demo Implementation (Sprints 3-4)
**Files to implement**:
- `crates/adapters/src/webhook_router.rs` - HTTP adapter
- `capsules/templates/json-transform/` - Demo capsule
- Basic proof backend with mock implementation

**Success Criteria**: Working Verifiable Webhook Router demo

## Lessons Learned

### What Worked Well (Preserve)
1. **Modular Architecture**: Clear separation of runtime/protocol/federation
2. **Security-First Design**: Capability-based sandbox model
3. **Resource Limiting**: Memory/CPU/time constraints  
4. **Hash Verification**: Integrity checking patterns

### What Added Complexity (Avoid)
1. **Protocol Maximalism**: Trying to replace existing protocols
2. **Premature Enterprise Features**: Complex permissions/audit from day 1
3. **ZK-First Approach**: Mandatory proofs blocking simple use cases
4. **Mixed Language Stack**: TypeScript + Rust coordination overhead

### Key Success Factors for New Implementation
1. **Start Simple**: MVP scope with clear gates
2. **Rust-First**: Unified language for performance and safety
3. **Optional Complexity**: ZK/enterprise features as addons
4. **Developer Experience**: Focus on `tenzik test/deploy` workflow

## Migration Timeline Estimate

**Week 1-2**: Runtime + receipts (reusing security patterns) ✅ Started  
**Week 3-4**: Federation (much simpler than Tent protocol)  
**Week 5-6**: ZK backend integration (pluggable design)  
**Week 7-8**: Demo implementation (webhook router)  
**Week 9-12**: Polish, testing, documentation

**Total**: 12 weeks to MVP (matching roadmap)

## Reference Archive

The complete previous codebase has been preserved at:
`C:\Users\17577\Desktop\tenzik\` (original location)

Key files for future reference:
- `packages/wasm-runtime/src/security-sandbox.ts` - Security patterns
- `packages/protocol/src/function.rs` - Metadata structures  
- `packages/cli/src/index.ts` - CLI command patterns
- `docs/architecture/overview.md` - Original architecture insights

## Success Metrics

**Sprint 1 Complete**: `cargo run -p tenzik-cli -- test hello.wasm '{"name":"world"}'` works  
**Sprint 2 Complete**: Two local nodes exchange receipts via gossip  
**MVP Complete**: Webhook router demo with receipt verification working

This migration preserves the valuable architectural insights while dramatically simplifying scope for faster delivery and better developer experience.
