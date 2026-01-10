# TenzikCore - Claude AI Assistant Guide

## Project Overview

TenzikCore is a verifiable edge compute platform where events carry code (WASM capsules) and sensitive workflows can be cryptographically proven. This is a fresh implementation transitioning from a complex Tent-based architecture to a pragmatic, focused edge compute platform.

**Core Concept**: Execute tiny (3-5KB) WASM capsules with signed ExecutionReceipts, optional ZK proofs, and minimal federation protocol.

## Tech Stack

- **Language**: Rust (Edition 2021)
- **Runtime**: Wasmtime (async WASM execution)
- **Cryptography**: ed25519-dalek, blake3
- **CLI**: clap v4
- **Async**: tokio (full features)
- **Federation**: libp2p, sled (embedded database)
- **Serialization**: serde, serde_json
- **Logging**: tracing, tracing-subscriber

## Project Structure

```
tenzik-core/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── runtime/            # WASM execution, validation, sandboxing
│   │   ├── validation.rs   # WASM capsule validation (RT-003)
│   │   ├── sandbox.rs      # Capability controls & resource limits (RT-004, RT-005)
│   │   ├── execution.rs    # WASM runtime execution
│   │   └── receipts.rs     # ExecutionReceipt sign/verify (ECON-002)
│   ├── protocol/           # Core protocol types and DAG
│   │   ├── events.rs       # Event types
│   │   ├── dag.rs          # DAG storage
│   │   └── errors.rs       # Error types
│   ├── federation/         # P2P networking and gossip
│   │   ├── node.rs         # Node announcement/handshake
│   │   ├── gossip.rs       # Receipt exchange
│   │   └── storage.rs      # Event DAG storage
│   ├── cli/                # Developer CLI tool
│   │   ├── main.rs
│   │   └── commands/       # test, validate, node subcommands
│   └── adapters/           # Protocol bridges
│       ├── webhook_router.rs  # Verifiable webhook router
│       └── http_server.rs     # HTTP adapter
├── capsules/               # WASM capsule templates
│   └── templates/
│       └── hello-world/    # Example capsule
├── docs/                   # Documentation
│   ├── architecture/       # Design docs
│   └── progress/          # Sprint tracking
└── examples/              # Demo applications
    └── two-node-demo/     # Federation demo
```

## Development Status

### ✅ Sprint 1 - COMPLETED
- RT-003: WASM validation
- RT-004: Capability mapper
- RT-005: Resource limits
- ECON-002: ExecutionReceipt (sign/verify)
- CLI-101: `tenzik test` command

### 🚧 Sprint 2 - Minimal Federation (In Progress)
- Event DAG storage
- Node announcement/handshake
- Gossip protocol for receipt exchange

### 🔜 Sprint 3 - Optional ZK
- ProofBackend trait with mock implementation
- Background proof job queue
- Receipt verification with signatures + ZK

### 🔜 Sprint 4 - Demo
- Verifiable Webhook Router
- JSON transform capsule template
- Receipt Explorer UI

## Key Design Principles

1. **Capsules stay tiny**: 3-5KB WASM with host-provided primitives
2. **Receipts everywhere**: Every execution gets a signed receipt
3. **ZK when needed**: Optional proof backends (Risc0/SP1/TEE)
4. **Developer-first**: `tenzik test/deploy` in <5 minutes
5. **Interop-ready**: Bridges before dogma
6. **Simplicity**: Dropped marketplace, complex permissions, dashboard from previous version

## Common Development Tasks

### Building and Testing

```bash
# Check workspace builds
cargo check --workspace

# Run all tests
cargo test --workspace

# Build release version
cargo build --release --workspace

# Run specific crate tests
cargo test -p tenzik-runtime
cargo test -p tenzik-cli
```

### CLI Development

```bash
# Run CLI in development
cargo run -p tenzik-cli -- --help

# Test capsule validation
cargo run -p tenzik-cli -- validate <path-to-wasm>

# Test capsule execution
cargo run -p tenzik-cli -- test <path-to-wasm> '<json-input>' --metrics --show-receipt

# Run node
cargo run -p tenzik-cli -- node start --port 8080
```

### Working with Capsules

```bash
# Navigate to hello-world template
cd capsules/templates/hello-world

# Compile WAT to WASM (requires wabt toolkit)
wat2wasm test.wat -o test.wasm

# Test the capsule
cargo run -p tenzik-cli -- test test.wasm '{"name":"Alice"}'
```

### Demo Scripts

```bash
# Linux/Mac
./demo.sh

# Windows
demo.bat
```

## Code Style and Conventions

### Rust Conventions
- Follow standard Rust formatting (rustfmt)
- Use `anyhow::Result` for functions that may error
- Use `thiserror` for custom error types
- Prefer async/await with tokio runtime
- Use tracing for logging, not println!

### Module Organization
- Each crate has a clear single responsibility
- Re-export key types in lib.rs for convenience
- Group related functionality in submodules
- Keep public API surface minimal

### Error Handling
- Use `anyhow::Result` for application errors
- Use custom error types (`ValidationError`, `SandboxError`, etc.) for domain errors
- Always provide context with `.context()` or `.with_context()`
- Errors should be actionable and informative

### Testing
- Unit tests in same file as implementation
- Integration tests in `tests/` directory
- Use `#[cfg(test)]` for test-only utilities
- Test error cases, not just happy paths
- Aim for meaningful test names: `test_validation_rejects_oversized_capsule`

### Documentation
- Public APIs must have doc comments
- Use `//!` for module-level documentation
- Include examples in doc comments where helpful
- Reference issue/feature IDs in comments (e.g., "RT-003", "ECON-002")

## Important Constraints

### WASM Capsule Requirements
- Maximum size: 3-5KB
- Must pass validation (proper imports, no prohibited instructions)
- Must respect capability allowlist
- Must operate within resource limits (memory, execution time)

### Security Considerations
- All WASM execution is sandboxed
- Capabilities must be explicitly allowed
- Resource limits are enforced (memory, CPU time, stack depth)
- ExecutionReceipts are signed with ed25519
- All inputs/outputs are logged in receipts

### Performance Targets
- Hello-world capsule execution: <100ms
- Receipt verification: <10ms
- Federation gossip latency: <500ms (Sprint 2+)

## Architecture Notes

### ExecutionReceipt
Every WASM capsule execution produces a signed receipt containing:
- Input hash
- Output hash
- Execution metrics (gas, memory, duration)
- Node signature
- Optional ZK proof (future)

### Capability System
WASM capsules can only import/call functions explicitly allowed:
- `tenzik_log`: Logging function
- `tenzik_emit`: Emit events
- Host may provide additional primitives

### Resource Limits
Enforced limits prevent runaway execution:
- Max memory: 1MB default
- Max execution time: 5 seconds default
- Stack depth limits
- No file I/O, network access (unless explicitly granted)

## Working with This Codebase

### Adding New Features
1. Check roadmap in README.md for sprint goals
2. Create/update types in `protocol/` if needed
3. Implement core logic in appropriate crate
4. Add CLI command if user-facing
5. Write tests (unit + integration)
6. Update documentation

### Debugging
- Use `RUST_LOG=debug cargo run` for verbose logging
- Use `cargo test -- --nocapture` to see test output
- Check `docs/progress/` for implementation notes
- Review `docs/architecture/` for design decisions

### Common Gotchas
- WASM validation requires proper module structure
- Capability checks must match import names exactly
- Resource limits apply to total execution (including host calls)
- Receipt signatures require matching keypairs (use test keys for dev)

## Git Workflow

- Main branch: `main` (protected)
- Feature branches: `feature/description` or `claude/description-SESSION_ID`
- Always run `cargo test --workspace` before committing
- Use conventional commit messages

## Dependencies

### Adding Dependencies
- Add to `[workspace.dependencies]` in root Cargo.toml
- Reference in crate Cargo.toml: `dependency = { workspace = true }`
- Prefer mature, well-maintained crates
- Consider binary size impact (capsules must stay small)

### Key Dependencies
- `wasmtime`: WASM runtime (v26.0)
- `ed25519-dalek`: Digital signatures (v2.1)
- `blake3`: Cryptographic hashing
- `tokio`: Async runtime
- `clap`: CLI parsing
- `libp2p`: P2P networking (Sprint 2+)

## Helpful Commands

```bash
# Check for common issues
cargo clippy --workspace

# Format code
cargo fmt --all

# Update dependencies (carefully!)
cargo update

# Generate documentation
cargo doc --workspace --open

# Check dependency tree
cargo tree -p tenzik-runtime

# Run specific test
cargo test test_capsule_validation

# Benchmark (when implemented)
cargo bench
```

## Resources

- **License**: Apache-2.0
- **Project**: Edge compute with verifiable execution
- **Documentation**: `/docs` directory
- **Examples**: `/examples` directory
- **Templates**: `/capsules/templates`

## Tips for Claude

- This is a security-sensitive project - always validate WASM before execution
- ExecutionReceipts are critical for verifiability - never skip signature verification
- Resource limits exist for a reason - don't bypass them
- When modifying runtime code, run full test suite
- Sprint goals guide priorities - check README.md for current focus
- Keep capsules tiny - if adding host functions, update capability system
- Federation code (Sprint 2+) may not be complete yet
- ZK proof system (Sprint 3) is optional/pluggable by design
