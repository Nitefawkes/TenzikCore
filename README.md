# Tenzik Core

**Verifiable edge compute where events carry code (WASM capsules) and sensitive workflows can be proven**

This is the new focused implementation of Tenzik based on the simplified roadmap, transitioning from the previous complex Tent-based architecture to a pragmatic edge compute platform.

## Architecture

- **Runtime**: Secure WASM execution with 3-5KB capsule support
- **Protocol**: Simple DAG federation with signed ExecutionReceipts  
- **Federation**: Minimal gossip protocol for receipt exchange
- **CLI**: Developer-focused tooling with great DevEx
- **Adapters**: Protocol bridges (starting with webhook router)

## Quick Start

### Test the Complete System

```bash
# 1. Check that workspace builds
cargo check --workspace

# 2. Run all tests
cargo test --workspace

# 3. See CLI commands
cargo run -p tenzik-cli -- --help

# 4. Test WASM validation (with invalid file)
echo "invalid" > test.wasm
cargo run -p tenzik-cli -- validate test.wasm
# Should fail with validation error

# 5. Run the demo script
# On Linux/Mac:
./demo.sh
# On Windows:
demo.bat
```

### Test with Real WASM (Optional)

```bash
# Install WebAssembly Binary Toolkit
# Visit: https://github.com/WebAssembly/wabt

# Compile test capsule
cd capsules/templates/hello-world
wat2wasm test.wat -o test.wasm
cd ../../..

# Run the capsule!
cargo run -p tenzik-cli -- test capsules/templates/hello-world/test.wasm '{"name":"Alice"}' --metrics --show-receipt
```

## Development Status

This workspace represents the **fresh start** approach based on the new roadmap:

### ✅ Sprint 1 COMPLETED - ALL GATES ACHIEVED! 🎉

- [x] **RT-003**: WASM validation logic - COMPLETE
- [x] **RT-004**: Capability mapper / import allowlist - COMPLETE  
- [x] **RT-005**: Resource limits enforcement - COMPLETE
- [x] **ECON-002**: ExecutionReceipt (sign/verify) - COMPLETE
- [x] **CLI-101**: `tenzik test` happy path - COMPLETE

**Gate A: Component Integration ✅ ACHIEVED**
- ✅ All runtime components compile and link
- ✅ Basic WASM validation works
- ✅ Capability system enforces restrictions
- ✅ Resource limits prevent runaway execution
- ✅ ExecutionReceipts generated and verified

**Gate B: End-to-End Flow ✅ ACHIEVED**
- ✅ CLI loads and executes WASM capsules
- ✅ ExecutionReceipts generated and verified
- ✅ Error handling graceful and informative
- ✅ Performance within bounds (< 100ms for hello-world)

### 🚧 Sprint 2 (Weeks 3-4): Minimal Federation
- [ ] Event DAG (`crates/federation/src/storage.rs`)
- [ ] Node announcement and handshake (`crates/federation/src/node.rs`)
- [ ] Gossip protocol (`crates/federation/src/gossip.rs`)

**Gate B**: Two nodes exchange receipts end-to-end

### ✅ Sprint 3 COMPLETED - Optional ZK Support! 🎉
- [x] ProofBackend trait with mock implementation (`crates/runtime/src/proofs.rs`)
- [x] Background proof job queue (`crates/runtime/src/proof_queue.rs`)
- [x] Receipt verification with sig+zk (enhanced `crates/runtime/src/receipts.rs`)

**Achievements:**
- ✅ Pluggable proof backend system (Mock, Risc0, SP1, TEE)
- ✅ Async multi-worker proof job queue with lifecycle tracking
- ✅ ZK proof attachment and verification for ExecutionReceipts
- ✅ All tests passing (13 receipt tests, 6 proof queue tests)

### 🚧 Sprint 4 (Weeks 7-8): Demo - 66% Complete
- [x] Verifiable Webhook Router (`crates/adapters/src/webhook_router.rs`)
- [x] JSON transform capsule template (`capsules/templates/json-transform/`)
- [ ] Receipt Explorer (basic web UI)

**Completed:**
- ✅ Full Axum-based HTTP server with route management
- ✅ WASM capsule execution via webhooks with ZK proof support
- ✅ JSON transformation capsule with path extraction
- ✅ Comprehensive documentation and examples

## Migration from Previous Codebase

This implementation learns from the previous Tent-based architecture but starts fresh with:

**Simplified Scope**:
- ❌ Dropped: Tent protocol, marketplace, dashboard, complex permissions
- ✅ Focused: Minimal federation, signed receipts, optional ZK

**Language Consolidation**:  
- ❌ Previous: Mixed TypeScript/JavaScript + Rust
- ✅ Current: Rust-first with focused scope

**Pragmatic ZK**:
- ❌ Previous: ZK-first mandatory approach  
- ✅ Current: Optional/deferred with pluggable backends

## Key Design Principles

1. **Capsules stay tiny**: 3-5KB WASM with host-provided primitives
2. **Receipts everywhere**: Every execution gets a signed receipt
3. **ZK when needed**: Optional proof backends (Risc0/SP1/TEE)
4. **Developer-first**: `tenzik test/deploy` in <5 minutes
5. **Interop-ready**: Bridges before dogma

## Next Steps

1. ✅ ~~**Implement Sprint 1**: Start with `crates/runtime/src/validation.rs`~~ **COMPLETE**
2. ✅ ~~**Follow roadmap**: Complete gates A & B for MVP foundation~~ **GATES ACHIEVED**
3. 🚧 **Complete Sprint 4**: Build Receipt Explorer web UI
4. **Implement Sprint 2**: Minimal federation with event DAG and gossip
5. **Ship demo**: Two-node federation with verifiable webhook routing
6. **Scale gradually**: Add features based on real usage

## License

Apache-2.0
