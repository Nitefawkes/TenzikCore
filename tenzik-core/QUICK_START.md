# Tenzik Quick Start Guide

Welcome to Tenzik! This guide will help you get started with verifiable edge compute using WASM capsules.

## Table of Contents

1. [Installation](#installation)
2. [Your First Capsule](#your-first-capsule)
3. [Common User Journeys](#common-user-journeys)
4. [Advanced Features](#advanced-features)

## Installation

### Prerequisites

- Rust 1.70+ ([Install Rust](https://rustup.rs/))
- Node.js 16+ (for AssemblyScript)
- Git

### Build from Source

```bash
git clone https://github.com/yourusername/TenzikCore
cd TenzikCore/tenzik-core
cargo build --release
```

The `tenzik` binary will be at `target/release/tenzik`.

## Your First Capsule

### 1. Create a New Project

```bash
tenzik init my-first-capsule
cd my-first-capsule
```

This creates a hello-world capsule with:
- `capsule.ts` - Your WASM code
- `package.json` - Build configuration
- `README.md` - Project documentation

### 2. Install Dependencies

```bash
npm install
```

This installs AssemblyScript and required tools.

### 3. Build the Capsule

**Option A: Using tenzik build (recommended)**
```bash
tenzik build
```

**Option B: Using npm**
```bash
npm run build
```

Your WASM capsule will be compiled to `build/capsule.wasm`.

The `tenzik build` command:
- Auto-detects AssemblyScript or Rust projects
- Optimizes for size by default
- Validates WASM output
- Shows build statistics

### 4. Test Locally

```bash
tenzik test build/capsule.wasm "World"
```

Expected output:
```
🧪 Testing capsule: build/capsule.wasm
📥 Input: World
✅ Execution successful!
📤 Output: Hello, World!
```

### 5. View Execution Receipt

```bash
tenzik test build/capsule.wasm "World" --show-receipt
```

This shows the cryptographic receipt proving execution:
- Capsule ID (Blake3 hash)
- Input/Output commits
- Execution metrics
- Node signature
- Timestamp

## Common User Journeys

### Journey 1: Capsule Developer

**Goal**: Create and test a data transformation capsule

```bash
# Create a JSON transform capsule
tenzik init json-processor --template json-transform
cd json-processor

# Install and build
npm install
npm run build

# Test with sample data
tenzik test build/capsule.wasm '{
  "data": {"name": "Alice", "age": 30, "email": "alice@example.com"},
  "transform": {
    "select": ["name", "email"],
    "rename": {"name": "fullName"}
  }
}'

# Validate WASM
tenzik validate build/capsule.wasm
```

**Output**:
```json
{"fullName":"Alice","email":"alice@example.com"}
```

### Journey 2: Node Operator

**Goal**: Run a Tenzik node to execute capsules

```bash
# Start a node on port 9000
tenzik node --port 9000 --db ./node-data

# Start a second node and connect to the first
tenzik node --port 9001 --peer 127.0.0.1:9000 --db ./node-data-2

# Use custom node name
tenzik node --name "my-tenzik-node" --port 9000
```

**What you get**:
- Executes capsules from network
- Participates in gossip protocol
- Stores receipts and events
- Federation with other nodes

### Journey 3: Application Integrator

**Goal**: Integrate Tenzik into an existing application

#### Option A: Webhook Integration

```rust
use tenzik_adapters::{WebhookRouter, WebhookConfig};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let config = WebhookConfig {
        port: 8080,
        capsule_path: PathBuf::from("./capsules/transform.wasm"),
        enable_zk_proofs: true,
        store_receipts: true,
        receipt_path: Some(PathBuf::from("./receipts")),
        ..Default::default()
    };

    let router = WebhookRouter::new(config)?;
    router.start().await?;
    Ok(())
}
```

#### Option B: Direct Runtime Integration

```rust
use tenzik_runtime::{WasmRuntime, ResourceLimits};
use ed25519_dalek::SigningKey;

#[tokio::main]
async fn main() -> Result<()> {
    let signing_key = /* your signing key */;
    let mut runtime = WasmRuntime::new(signing_key)?;

    let capsule = std::fs::read("capsule.wasm")?;
    let input = b"Hello, Tenzik!";
    let limits = ResourceLimits::default();

    let receipt = runtime.execute(&capsule, input, limits).await?;

    println!("Output: {}", receipt.output);
    println!("Receipt: {}", serde_json::to_string_pretty(&receipt)?);
    Ok(())
}
```

### Journey 4: Auditor

**Goal**: Verify execution receipts

```bash
# Verify receipt signature and validity
tenzik receipt verify receipt.json

# Inspect receipt details
tenzik receipt inspect receipt.json

# Inspect with full JSON output
tenzik receipt inspect receipt.json --verbose

# Export summary for reporting
tenzik receipt export receipt.json --output summary.json
```

**Example output**:
```
🔍 Verifying receipt: receipt.json
  ✓ Checking signature... ✅ Valid
  ✓ Checking timestamp... ✅ Valid (within acceptable age)
  ✓ Checking ZK proof... ✅ Present
     Proof data: 384 bytes

✅ Receipt verification completed successfully!

📊 Receipt Details:
   Node ID: node-12345
   Capsule ID: a3f2e8d1c7b6...
   Timestamp: 2025-11-23T04:30:15Z
   Nonce: 67890
```

## Advanced Features

### Custom Resource Limits

Control execution resources:

```bash
tenzik test capsule.wasm "input" --limits '{
  "max_fuel": 10000000,
  "max_memory_bytes": 16777216,
  "max_execution_ms": 5000
}'
```

### Execution Metrics

View detailed performance data:

```bash
tenzik test capsule.wasm "input" --metrics
```

Output includes:
- Fuel consumed
- Memory usage
- Execution time
- Host function calls

### Available Templates

#### hello-world
Basic input/output handling

```bash
tenzik init demo --template hello-world
```

#### json-transform
Field selection, renaming, and transformation

```bash
tenzik init api-processor --template json-transform
```

## CLI Command Reference

### Project Management

| Command | Description | Example |
|---------|-------------|---------|
| `tenzik init <name>` | Create new capsule project | `tenzik init my-capsule` |
| `tenzik init <name> --template <tpl>` | Create from template | `tenzik init demo --template json-transform` |

### Building

| Command | Description | Example |
|---------|-------------|---------|
| `tenzik build` | Build capsule from source | `tenzik build` |
| `tenzik build -o <path>` | Build to custom output path | `tenzik build -o dist/capsule.wasm` |
| `tenzik build -O <level>` | Set optimization level | `tenzik build -O aggressive` |
| `tenzik build -v` | Verbose build output | `tenzik build --verbose` |

**Optimization Levels:**
- `0` or `none`: No optimization (fastest build)
- `s` or `size`: Optimize for size (default)
- `1-2` or `speed`: Optimize for speed
- `3` or `z` or `aggressive`: Maximum optimization

### Testing & Validation

| Command | Description | Example |
|---------|-------------|---------|
| `tenzik test <wasm> <input>` | Test capsule locally | `tenzik test capsule.wasm "test"` |
| `tenzik test ... --metrics` | Show execution metrics | `tenzik test capsule.wasm "test" --metrics` |
| `tenzik test ... --show-receipt` | Display full receipt | `tenzik test capsule.wasm "test" --show-receipt` |
| `tenzik validate <wasm>` | Validate WASM capsule | `tenzik validate capsule.wasm` |

### Node Operations

| Command | Description | Example |
|---------|-------------|---------|
| `tenzik node` | Start node (default port 9000) | `tenzik node` |
| `tenzik node --port <port>` | Use custom port | `tenzik node --port 8080` |
| `tenzik node --peer <addr>` | Connect to peer | `tenzik node --peer 192.168.1.100:9000` |
| `tenzik node --db <path>` | Set database path | `tenzik node --db ./my-node-data` |
| `tenzik node --name <name>` | Set node name | `tenzik node --name production-node-1` |

### Receipt Management

| Command | Description | Example |
|---------|-------------|---------|
| `tenzik receipt verify <file>` | Verify receipt | `tenzik receipt verify receipt.json` |
| `tenzik receipt inspect <file>` | View receipt details | `tenzik receipt inspect receipt.json` |
| `tenzik receipt inspect <file> -v` | Verbose output | `tenzik receipt inspect receipt.json --verbose` |
| `tenzik receipt export <file>` | Export summary | `tenzik receipt export receipt.json -o summary.json` |

## Troubleshooting

### Build Failures

**Error**: `asc: command not found`
```bash
npm install -g assemblyscript
```

**Error**: `Failed to create Wasmtime engine`
```bash
# Check WASM runtime compatibility
rustc --version  # Ensure 1.70+
```

### Validation Errors

**Error**: `Capsule exceeds maximum size`
```bash
# Optimize your build
asc capsule.ts -o capsule.wasm --optimize
```

**Error**: `Invalid import detected`
```bash
# Only use allowed imports:
# - env::memory
# - env::abort
```

### Node Issues

**Error**: `Cannot write to database path`
```bash
# Ensure directory is writable
mkdir -p ./node-data
chmod 755 ./node-data
```

**Error**: `Peer connection failed`
```bash
# Check peer is reachable
nc -zv 127.0.0.1 9000
```

## Next Steps

- 📖 Read the [Architecture Documentation](docs/architecture/runtime-design.md)
- 🔬 Explore [Sprint Progress](docs/progress/)
- 🚀 Check out [Advanced Examples](examples/)
- 🔐 Learn about [ZK Proof Integration](docs/zk-proofs.md)

## Getting Help

- GitHub Issues: [Report bugs or request features](https://github.com/yourusername/TenzikCore/issues)
- Documentation: See the `docs/` directory
- Examples: See the `examples/` directory

---

**Happy building with Tenzik! 🚀**
